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

use std::future::Future;
use std::mem;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use asyncband::oneshot;
use flume::Receiver;
use flume::Sender;
use monoio::FusionDriver;
use monoio::RuntimeBuilder;
use opendal_core::raw::*;
use opendal_core::*;

use super::MONOIOFS_SCHEME;

pub const BUFFER_SIZE: usize = 2 * 1024 * 1024; // 2 MiB

/// a boxed function that spawns task in current monoio runtime
type TaskSpawner = Box<dyn FnOnce() + Send>;

#[derive(Debug)]
pub struct MonoiofsCore {
    pub info: ServiceInfo,
    pub capability: Capability,

    root: PathBuf,
    /// sender that sends [`TaskSpawner`] to worker threads
    tx: Sender<TaskSpawner>,
    /// join handles of worker threads
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
    pub buf_pool: oio::PooledBuf,
}

impl MonoiofsCore {
    pub fn new(root: PathBuf, worker_threads: usize, io_uring_entries: u32) -> Self {
        // Since users use monoiofs in a context of tokio, all monoio
        // operations need to be dispatched to a dedicated thread pool
        // where a monoio runtime runs on each thread. Here we spawn
        // these worker threads.
        let (tx, rx) = flume::unbounded();
        let threads = (0..worker_threads)
            .map(move |i| {
                let rx = rx.clone();
                std::thread::Builder::new()
                    .name(format!("monoiofs-worker-{i}"))
                    .spawn(move || Self::worker_entrypoint(rx, io_uring_entries))
                    .expect("spawn worker thread should success")
            })
            .collect();
        let threads = Mutex::new(threads);

        Self {
            info: ServiceInfo::new(MONOIOFS_SCHEME, root.to_string_lossy(), ""),
            capability: Capability {
                stat: true,

                read: true,

                write: true,
                write_can_append: true,

                delete: true,
                rename: true,
                create_dir: true,
                copy: true,
                shared: true,
                ..Default::default()
            },
            root,
            tx,
            threads,
            buf_pool: oio::PooledBuf::new(16).with_initial_capacity(BUFFER_SIZE),
        }
    }

    /// Reject `..` traversal and absolute keys so they cannot escape `root`,
    /// matching the `fs` backend (#7684). Confinement lives here because
    /// `normalize_path` deliberately leaves `.`/`..` unresolved (RFC 0112) and
    /// `PathBuf::join` discards the base when the key is absolute.
    pub fn prepare_path(&self, path: &str) -> Result<PathBuf> {
        use std::path::Component;
        let trimmed = path.trim_end_matches('/');
        if Path::new(trimmed).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(
                Error::new(ErrorKind::NotFound, "path escapes the configured root")
                    .with_context("path", path),
            );
        }
        Ok(self.root.join(trimmed))
    }

    /// join root and path, create parent dirs
    pub async fn prepare_write_path(&self, path: &str) -> Result<PathBuf> {
        let path = self.prepare_path(path)?;
        let parent = path
            .parent()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "path should have parent but not, it must be malformed",
                )
                .with_context("input", path.to_string_lossy())
            })?
            .to_path_buf();
        self.dispatch(move || monoio::fs::create_dir_all(parent))
            .await
            .map_err(new_std_io_error)?;
        Ok(path)
    }

    /// entrypoint of each worker thread, sets up monoio runtimes and channels
    fn worker_entrypoint(rx: Receiver<TaskSpawner>, io_uring_entries: u32) {
        let mut rt = RuntimeBuilder::<FusionDriver>::new()
            .enable_all()
            .with_entries(io_uring_entries)
            .build()
            .expect("monoio runtime initialize should success");
        // run an infinite loop that receives TaskSpawner and calls
        // them in a context of monoio
        rt.block_on(async {
            while let Ok(spawner) = rx.recv_async().await {
                spawner();
            }
        })
    }

    /// Create a TaskSpawner, send it to the thread pool and wait
    /// for its result. Task panic will propagate.
    pub async fn dispatch<F, Fut, T>(&self, f: F) -> T
    where
        F: FnOnce() -> Fut + 'static + Send,
        Fut: Future<Output = T>,
        T: 'static + Send,
    {
        // oneshot channel to send result back
        let (tx, rx) = oneshot::channel();
        let result = self
            .tx
            .send_async(Box::new(move || {
                // task will be spawned on current thread, task panic
                // will cause current worker thread panic
                monoio::spawn(async move {
                    // discard the result if send failed due to
                    // MonoiofsCore::dispatch cancelled
                    let _ = tx.send(f().await);
                });
            }))
            .await;
        self.unwrap(result);
        self.unwrap(rx.await)
    }

    /// Create a TaskSpawner, send it to the thread pool and spawn the task.
    pub async fn spawn<F, Fut, T>(&self, f: F)
    where
        F: FnOnce() -> Fut + 'static + Send,
        Fut: Future<Output = T> + 'static,
        T: 'static,
    {
        let result = self
            .tx
            .send_async(Box::new(move || {
                // task will be spawned on current thread, task panic
                // will cause current worker thread panic
                monoio::spawn(f());
            }))
            .await;
        self.unwrap(result);
    }

    /// This method always panics. It is called only when at least a
    /// worker thread has panicked or meet a broken rx, which is
    /// unrecoverable. It propagates worker thread's panic if there
    /// is any and panics on normally exited thread.
    pub fn propagate_worker_panic(&self) -> ! {
        let mut guard = self.threads.lock().expect("worker thread has panicked");
        // wait until the panicked thread exits
        std::thread::sleep(Duration::from_millis(100));
        let threads = mem::take(&mut *guard);
        // we don't know which thread panicked, so check them one by one
        for thread in threads {
            if thread.is_finished() {
                // worker thread runs an infinite loop, hence finished
                // thread must have panicked or meet a broken rx.
                match thread.join() {
                    // rx is broken
                    Ok(()) => panic!("worker thread should not exit, tx may be dropped"),
                    // thread has panicked
                    Err(e) => std::panic::resume_unwind(e),
                }
            }
        }
        unreachable!("this method should panic")
    }

    /// Unwrap result if result is Ok, otherwise propagates worker thread's
    /// panic. This method facilitates panic propagation in situation where
    /// Err returned by broken channel indicates that the worker thread has
    /// panicked.
    pub fn unwrap<T, E>(&self, result: Result<T, E>) -> T {
        match result {
            Ok(result) => result,
            Err(_) => self.propagate_worker_panic(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use asyncband::mpsc;
    use asyncband::mpsc::UnboundedSender;

    use super::*;

    fn new_core(worker_threads: usize) -> Arc<MonoiofsCore> {
        Arc::new(MonoiofsCore::new(PathBuf::new(), worker_threads, 1024))
    }

    async fn dispatch_simple(core: Arc<MonoiofsCore>) {
        let result = core.dispatch(|| async { 42 }).await;
        assert_eq!(result, 42);
        let bytes: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let bytes_clone = bytes.clone();
        let result = core.dispatch(move || async move { bytes }).await;
        assert_eq!(result, bytes_clone);
    }

    async fn dispatch_concurrent(core: Arc<MonoiofsCore>) {
        let (tx, mut rx) = mpsc::unbounded();

        fn spawn_task(core: Arc<MonoiofsCore>, tx: UnboundedSender<u64>, sleep_millis: u64) {
            tokio::spawn(async move {
                let result = core
                    .dispatch(move || async move {
                        monoio::time::sleep(Duration::from_millis(sleep_millis)).await;
                        sleep_millis
                    })
                    .await;
                assert_eq!(result, sleep_millis);
                tx.send(result).unwrap();
            });
        }

        spawn_task(core.clone(), tx.clone(), 200);
        spawn_task(core.clone(), tx.clone(), 20);
        drop(tx);
        let first = rx.recv().await.ok();
        let second = rx.recv().await.ok();
        let third = rx.recv().await.ok();
        assert_eq!(first, Some(20));
        assert_eq!(second, Some(200));
        assert_eq!(third, None);
    }

    async fn dispatch_panic(core: Arc<MonoiofsCore>) {
        core.dispatch(|| async { panic!("BOOM") }).await;
    }

    #[tokio::test]
    async fn test_monoio_single_thread_dispatch() {
        let core = new_core(1);
        assert_eq!(core.threads.lock().unwrap().len(), 1);
        dispatch_simple(core).await;
    }

    #[tokio::test]
    async fn test_monoio_single_thread_dispatch_concurrent() {
        let core = new_core(1);
        dispatch_concurrent(core).await;
    }

    #[tokio::test]
    #[should_panic(expected = "BOOM")]
    async fn test_monoio_single_thread_dispatch_panic() {
        let core = new_core(1);
        dispatch_panic(core).await;
    }

    #[tokio::test]
    async fn test_monoio_multi_thread_dispatch() {
        let core = new_core(4);
        assert_eq!(core.threads.lock().unwrap().len(), 4);
        dispatch_simple(core).await;
    }

    #[tokio::test]
    async fn test_monoio_multi_thread_dispatch_concurrent() {
        let core = new_core(4);
        dispatch_concurrent(core).await;
    }

    #[tokio::test]
    #[should_panic(expected = "BOOM")]
    async fn test_monoio_multi_thread_dispatch_panic() {
        let core = new_core(4);
        dispatch_panic(core).await;
    }

    #[tokio::test]
    async fn test_prepare_path_rejects_parent_dir() {
        let core = Arc::new(MonoiofsCore::new(PathBuf::from("/data/root"), 1, 1024));
        for key in ["../etc/passwd", "../../etc/passwd", "a/../../b", "a/.."] {
            let err = core.prepare_path(key).unwrap_err();
            assert_eq!(
                err.kind(),
                ErrorKind::NotFound,
                "key should be rejected: {key}"
            );
        }
    }

    #[tokio::test]
    async fn test_prepare_path_rejects_absolute_path() {
        let core = Arc::new(MonoiofsCore::new(PathBuf::from("/data/root"), 1, 1024));
        // `PathBuf::join` drops the root when the key is absolute.
        for key in ["/etc/passwd", "//etc/passwd"] {
            let err = core.prepare_path(key).unwrap_err();
            assert_eq!(
                err.kind(),
                ErrorKind::NotFound,
                "key should be rejected: {key}"
            );
        }
    }

    #[tokio::test]
    async fn test_prepare_path_allows_normal_keys() {
        let core = Arc::new(MonoiofsCore::new(PathBuf::from("/data/root"), 1, 1024));
        // Normal keys, `.` (CurDir), and trailing slashes resolve unchanged.
        assert_eq!(
            core.prepare_path("a/b.txt").unwrap(),
            PathBuf::from("/data/root/a/b.txt")
        );
        assert_eq!(
            core.prepare_path("a/b/").unwrap(),
            PathBuf::from("/data/root/a/b")
        );
        assert_eq!(
            core.prepare_path("./a/b").unwrap(),
            PathBuf::from("/data/root/a/b")
        );
        // A key containing `..` only as a substring of a name is not a traversal.
        assert_eq!(
            core.prepare_path("a..b").unwrap(),
            PathBuf::from("/data/root/a..b")
        );
    }
}
