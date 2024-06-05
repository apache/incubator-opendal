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

use crate::raw::BoxedStaticFuture;
use futures::future::RemoteHandle;
use futures::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Execute trait is used to execute task in background.
///
/// # Notes about Timeout Implementation
///
/// Implementing a correct and elegant timeout mechanism is challenging for us.
///
/// The `Execute` trait must be object safe, allowing us to use `Arc<dyn Execute>`. Consequently,
/// we cannot introduce a generic type parameter to `Execute`. We utilize [`RemoteHandle`] to
/// implement the [`Execute::execute`] method. [`RemoteHandle`] operates by transmitting
/// `Future::Output` through a channel, enabling the spawning of [`BoxedStaticFuture<()>`].
///
/// However, for timeouts, we need to spawn a future that resolves after a specified duration.
/// Simply wrapping the future within another timeout future is not feasible because if the timeout
/// is reached and the original future has not completed, it will be dropped—causing any held `Task`
/// to panic.
///
/// As an alternative solution, we developed a `timeout` API. Users of the `Executor` should invoke
/// this API when they require a timeout and combine it with their own futures using
/// [`futures::select`].
///
/// This approach may seem inelegant but it allows us flexibility without being tied specifically
/// to the Tokio runtime.
///
/// PLEASE raising an issue if you have a better solution.
pub trait Execute: Send + Sync + 'static {
    /// Execute async task in background.
    ///
    /// # Behavior
    ///
    /// - Implementor MUST manage the executing futures and keep making progress.
    /// - Implementor MUST NOT drop futures until it's resolved.
    fn execute(&self, f: BoxedStaticFuture<()>);

    /// Return a future that will be resolved after the given timeout.
    ///
    /// Default implementation returns None.
    fn timeout(&self) -> Option<BoxedStaticFuture<()>> {
        None
    }
}

impl Execute for () {
    fn execute(&self, _: BoxedStaticFuture<()>) {
        panic!("concurrent tasks executed with no executor has been enabled")
    }
}

/// Task is generated by Executor that represents an executing task.
///
/// Users can fetch the results by calling `poll` or `.await` on this task.
/// Or, users can cancel the task by `drop` this task handle.
///
/// # Notes
///
/// Users don't need to call `poll` to make progress. All tasks are running in
/// the background.
pub struct Task<T> {
    handle: RemoteHandle<T>,
}

impl<T: 'static> Task<T> {
    /// Create a new task.
    #[inline]
    pub fn new(handle: RemoteHandle<T>) -> Self {
        Self { handle }
    }
}

impl<T: 'static> Future for Task<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.handle.poll_unpin(cx)
    }
}
