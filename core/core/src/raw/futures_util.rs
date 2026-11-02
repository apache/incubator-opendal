// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use futures::FutureExt;

use crate::*;

/// BoxedFuture is the type alias of [`futures::future::BoxFuture`].
#[cfg(not(target_arch = "wasm32"))]
pub type BoxedFuture<'a, T> = futures::future::BoxFuture<'a, T>;
#[cfg(target_arch = "wasm32")]
/// BoxedFuture is the type alias of [`futures::future::LocalBoxFuture`].
pub type BoxedFuture<'a, T> = futures::future::LocalBoxFuture<'a, T>;

/// BoxedStaticFuture is the type alias of [`futures::future::BoxFuture`].
#[cfg(not(target_arch = "wasm32"))]
pub type BoxedStaticFuture<T> = futures::future::BoxFuture<'static, T>;
#[cfg(target_arch = "wasm32")]
/// BoxedStaticFuture is the type alias of [`futures::future::LocalBoxFuture`].
pub type BoxedStaticFuture<T> = futures::future::LocalBoxFuture<'static, T>;

/// MaybeSend is a marker to determine whether a type is `Send` or not.
/// We use this trait to wrap the `Send` requirement for wasm32 target.
///
/// # Safety
///
/// [`MaybeSend`] is equivalent to `Send` on non-wasm32 target.
/// And it's empty trait on wasm32 target to indicate that a type is not `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}

/// MaybeSend is a marker to determine whether a type is `Send` or not.
/// We use this trait to wrap the `Send` requirement for wasm32 target.
///
/// # Safety
///
/// [`MaybeSend`] is equivalent to `Send` on non-wasm32 target.
/// And it's empty trait on wasm32 target to indicate that a type is not `Send`.
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

/// ConcurrentTasks executes tasks concurrently and collects outputs in submission order.
///
/// Submit inputs with [`Self::execute`] and collect outputs with [`Self::next`].
/// The queue owns the task handles and tracks their completion for concurrency control.
///
/// ConcurrentTasks has two generic types:
///
/// - `I` represents the input type of the task.
/// - `O` represents the output type of the task.
///
/// # Implementation Notes
///
/// The code patterns below are intentional; please do not modify them unless you fully understand these notes.
///
/// ```skip
///  let result = self
///     .tasks
///     .front_mut()                                        // Use `front_mut` instead of `pop_front`
///     .expect("tasks must be available")
///     .await;
/// ...
/// match result {
///     Ok(o) => {
///         let _ = self.tasks.pop_front();                 // `pop_front` after got `Ok(o)`
///         self.results.push_back(o)
///     }
///     Err((i, err)) => {
///         if err.is_temporary() {
///             let task = self.spawn_task(i);
///             self.tasks
///                 .front_mut()
///                 .expect("tasks must be available")
///                 .replace(task)                          // Use replace here to instead of `push_front`
///         } else {
///             self.clear();
///             self.errored = true;
///         }
///         return Err(err);
///     }
/// }
/// ```
///
/// Please keep in mind that there is no guarantee the task will be `await`ed until completion. It's possible
/// the task may be dropped before it resolves. Therefore, we should keep the `Task` in the `tasks` queue until
/// it is resolved.
///
/// For example, users may have a timeout for the task, and the task will be dropped if it exceeds the timeout.
/// If we `pop_front` the task before it resolves, the task will be canceled and the result will be lost.
pub struct ConcurrentTasks<I, O> {
    /// The executor to execute the tasks.
    ///
    /// If user doesn't provide an executor, the tasks will be executed with the default executor.
    executor: Executor,
    /// The factory to create the task.
    ///
    /// Caller of ConcurrentTasks must provide a factory to create the task for executing.
    ///
    /// The factory must accept an input and return a future that resolves to a tuple of input and
    /// output result. If the given result is error, the error will be returned to users and the
    /// task will be retried.
    factory: fn(I) -> BoxedStaticFuture<(I, Result<O>)>,

    /// `tasks` holds the ongoing tasks.
    ///
    /// Please keep in mind that all tasks are running in the background by `Executor`. We only need
    /// to poll the tasks to see if they are ready.
    ///
    /// Dropping task without `await` it will cancel the task.
    tasks: VecDeque<Task<Result<O, (I, Error)>>>,
    /// `results` stores the successful results.
    results: VecDeque<O>,

    /// The maximum number of concurrent tasks.
    concurrent: usize,
    /// The maximum number of completed tasks that can be buffered.
    prefetch: usize,
    /// Tracks the number of tasks that have finished execution but have not yet been collected.
    /// This count is subtracted from the total concurrency capacity, ensuring that the system
    /// always schedules new tasks to maintain the user's desired concurrency level.
    ///
    /// Example: If `concurrency = 10` and `completed_but_unretrieved = 3`,
    ///          the system can still spawn 7 new tasks (since 3 slots are "logically occupied"
    ///          by uncollected results).
    completed_but_unretrieved: Arc<AtomicUsize>,
    /// hitting the last unrecoverable error.
    ///
    /// If concurrent tasks hit an unrecoverable error, it will stop executing new tasks and return
    /// an unrecoverable error to users.
    errored: bool,
}

impl<I: Send + 'static, O: Send + 'static> ConcurrentTasks<I, O> {
    /// Create a new concurrent tasks with given executor, concurrent, prefetch and factory.
    ///
    /// `prefetch` is extra completed tasks allowed in the queue before
    /// [`Self::execute`] waits to collect the head. Pass `0` when the caller
    /// does not collect results while still submitting work; otherwise failed
    /// tasks keep their inputs until collection.
    ///
    /// The factory is a function pointer that shouldn't capture any context.
    pub fn new(
        executor: Executor,
        concurrent: usize,
        prefetch: usize,
        factory: fn(I) -> BoxedStaticFuture<(I, Result<O>)>,
    ) -> Self {
        Self {
            executor,
            factory,

            tasks: VecDeque::with_capacity(concurrent),
            results: VecDeque::with_capacity(concurrent),
            concurrent,
            prefetch,
            completed_but_unretrieved: Arc::default(),
            errored: false,
        }
    }

    /// Return true if the tasks are running concurrently.
    #[inline]
    fn is_concurrent(&self) -> bool {
        self.concurrent > 1
    }

    /// Clear all tasks and results.
    ///
    /// All ongoing tasks will be canceled.
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.results.clear();
    }

    /// Check if there are remaining space to push new tasks.
    #[inline]
    pub fn has_remaining(&self) -> bool {
        let completed = self.completed_but_unretrieved.load(Ordering::Relaxed);
        // Allow up to `prefetch` completed tasks to be buffered
        self.tasks.len() < self.concurrent + completed.min(self.prefetch)
    }

    /// Chunk if there are remaining results to fetch.
    #[inline]
    pub fn has_result(&self) -> bool {
        !self.results.is_empty()
    }

    fn spawn_task(&self, input: I) -> Task<Result<O, (I, Error)>> {
        let completed = self.completed_but_unretrieved.clone();
        let fut = (self.factory)(input)
            // Completed tasks can remain queued while the caller produces more work.
            // Only failures need to retain their input for a retry.
            .map(|(input, result)| result.map_err(|err| (input, err)))
            .inspect(move |_| {
                completed.fetch_add(1, Ordering::Relaxed);
            });

        self.executor.execute(fut)
    }

    /// Execute the task with given input.
    ///
    /// - Execute the task in the current thread if is not concurrent.
    /// - Execute the task in the background if there are available slots.
    /// - Await the first task in the queue if there is no available slots.
    pub async fn execute(&mut self, input: I) -> Result<()> {
        if self.errored {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "concurrent tasks met an unrecoverable error",
            ));
        }

        // Short path for non-concurrent case.
        if !self.is_concurrent() {
            let (_, o) = (self.factory)(input).await;
            return match o {
                Ok(o) => {
                    self.results.push_back(o);
                    Ok(())
                }
                // We don't need to rebuild the future if it's not concurrent.
                Err(err) => Err(err),
            };
        }

        if !self.has_remaining() {
            let result = self
                .tasks
                .front_mut()
                .expect("tasks must be available")
                .await;
            self.completed_but_unretrieved
                .fetch_sub(1, Ordering::Relaxed);
            match result {
                Ok(o) => {
                    let _ = self.tasks.pop_front();
                    self.results.push_back(o)
                }
                Err((i, err)) => {
                    // Retry this task if the error is temporary
                    if err.is_temporary() {
                        let task = self.spawn_task(i);
                        self.tasks
                            .front_mut()
                            .expect("tasks must be available")
                            .replace(task)
                    } else {
                        self.clear();
                        self.errored = true;
                    }
                    return Err(err);
                }
            }
        }

        self.tasks.push_back(self.spawn_task(input));
        Ok(())
    }

    /// Fetch the successful result from the result queue.
    pub async fn next(&mut self) -> Option<Result<O>> {
        if self.errored {
            return Some(Err(Error::new(
                ErrorKind::Unexpected,
                "concurrent tasks met an unrecoverable error",
            )));
        }

        if let Some(result) = self.results.pop_front() {
            return Some(Ok(result));
        }

        if let Some(task) = self.tasks.front_mut() {
            let result = task.await;
            self.completed_but_unretrieved
                .fetch_sub(1, Ordering::Relaxed);
            return match result {
                Ok(o) => {
                    let _ = self.tasks.pop_front();
                    Some(Ok(o))
                }
                Err((i, err)) => {
                    // Retry this task if the error is temporary
                    if err.is_temporary() {
                        let task = self.spawn_task(i);
                        self.tasks
                            .front_mut()
                            .expect("tasks must be available")
                            .replace(task)
                    } else {
                        self.clear();
                        self.errored = true;
                    }
                    Some(Err(err))
                }
            };
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rand::RngExt;
    use tokio::time::sleep;

    use super::*;
    use crate::raw::Duration;

    #[tokio::test]
    async fn test_concurrent_tasks() {
        let executor = Executor::new();

        let mut tasks = ConcurrentTasks::new(executor, 16, 8, |(i, dur)| {
            Box::pin(async move {
                sleep(dur).await;

                // 5% rate to fail.
                if rand::rng().random_range(0..100) > 90 {
                    return (
                        (i, dur),
                        Err(Error::new(ErrorKind::Unexpected, "I'm lucky").set_temporary()),
                    );
                }
                ((i, dur), Ok(i))
            })
        });

        let mut ans = vec![];

        for i in 0..10240 {
            // Sleep up to 10ms
            let dur = Duration::from_millis(rand::rng().random_range(0..10));
            loop {
                let res = tasks.execute((i, dur)).await;
                if res.is_ok() {
                    break;
                }
            }
        }

        loop {
            match tasks.next().await.transpose() {
                Ok(Some(i)) => ans.push(i),
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        assert_eq!(ans, (0..10240).collect::<Vec<_>>())
    }

    #[tokio::test]
    async fn test_prefetch_backpressure() {
        let executor = Executor::new();
        let concurrent = 4;
        let prefetch = 2;

        // Create a slower task to ensure they don't complete immediately
        let mut tasks = ConcurrentTasks::new(executor, concurrent, prefetch, |i: usize| {
            Box::pin(async move {
                sleep(Duration::from_millis(100)).await;
                (i, Ok(i))
            })
        });

        // Initially, we should have space for concurrent tasks
        assert!(tasks.has_remaining(), "Should have space initially");

        // Submit concurrent tasks
        for i in 0..concurrent {
            assert!(tasks.has_remaining(), "Should have space for task {i}");
            tasks.execute(i).await.unwrap();
        }

        // Now we shouldn't have any more space (since no tasks have completed yet)
        assert!(
            !tasks.has_remaining(),
            "Should not have space after submitting concurrent tasks"
        );

        // Wait for some tasks to complete
        sleep(Duration::from_millis(150)).await;

        // Now we should have space up to prefetch limit
        for i in concurrent..concurrent + prefetch {
            assert!(
                tasks.has_remaining(),
                "Should have space for prefetch task {i}"
            );
            tasks.execute(i).await.unwrap();
        }

        // Now has_remaining should return false
        assert!(
            !tasks.has_remaining(),
            "Should not have remaining space after filling up prefetch buffer"
        );

        // Retrieve one result
        let result = tasks.next().await;
        assert!(result.is_some());

        // Now there should be space for one more task
        assert!(
            tasks.has_remaining(),
            "Should have remaining space after retrieving one result"
        );
    }

    #[tokio::test]
    async fn test_prefetch_zero() {
        let executor = Executor::new();
        let concurrent = 4;
        let prefetch = 0; // No prefetching allowed

        let mut tasks = ConcurrentTasks::new(executor, concurrent, prefetch, |i: usize| {
            Box::pin(async move {
                sleep(Duration::from_millis(10)).await;
                (i, Ok(i))
            })
        });

        // With prefetch=0, we can only submit up to concurrent tasks
        for i in 0..concurrent {
            tasks.execute(i).await.unwrap();
        }

        // Should not have space for more
        assert!(
            !tasks.has_remaining(),
            "Should not have remaining space with prefetch=0"
        );

        // Retrieve one result
        let result = tasks.next().await;
        assert!(result.is_some());

        // Now there should be space for exactly one more task
        assert!(
            tasks.has_remaining(),
            "Should have remaining space after retrieving one result"
        );

        // Execute one more
        tasks.execute(concurrent).await.unwrap();

        // Should be full again
        assert!(!tasks.has_remaining(), "Should be full again");
    }
}
