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

use std::ffi::c_void;
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};
use std::thread::available_parallelism;

use crate::error::OpenDALError;
use crate::result::OpendalExecutorResult;
use crate::utils::config_invalid_error;

static DEFAULT_EXECUTOR: OnceLock<Arc<Executor>> = OnceLock::new();

pub struct Executor {
    runtime: tokio::runtime::Runtime,
}

impl Executor {
    fn new(threads: usize) -> Result<Self, OpenDALError> {
        if threads == 0 {
            return Err(config_invalid_error(
                "executor threads must be greater than 0",
            ));
        }

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(threads)
            .enable_all()
            .build()
            .map_err(|e| {
                OpenDALError::from_opendal_error(
                    opendal::Error::new(
                        opendal::ErrorKind::Unexpected,
                        "failed to create tokio runtime",
                    )
                    .set_source(e),
                )
            })?;

        Ok(Self { runtime })
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }
}

fn default_executor() -> Result<Arc<Executor>, OpenDALError> {
    if let Some(executor) = DEFAULT_EXECUTOR.get() {
        return Ok(executor.clone());
    }

    let threads = available_parallelism().map(NonZeroUsize::get).unwrap_or(1);
    let executor = Arc::new(Executor::new(threads)?);

    if DEFAULT_EXECUTOR.set(executor.clone()).is_ok() {
        return Ok(executor);
    }

    if let Some(existing) = DEFAULT_EXECUTOR.get() {
        return Ok(existing.clone());
    }

    Ok(executor)
}

/// Resolve the executor an operator binds at construction.
///
/// # Safety
///
/// `executor` must be either null or a pointer previously returned by
/// `executor_create` that stays alive for the duration of this call. The
/// returned clone keeps the runtime alive on its own afterwards.
pub(crate) unsafe fn executor_or_default(
    executor: *const c_void,
) -> Result<Arc<Executor>, OpenDALError> {
    if executor.is_null() {
        return default_executor();
    }

    // SAFETY: per the contract above the pointer came from `executor_create`'s
    // `Arc::into_raw` and is still alive, so bumping the strong count and
    // rebuilding an owned clone is sound.
    unsafe {
        let executor = executor as *const Executor;
        Arc::increment_strong_count(executor);
        Ok(Arc::from_raw(executor))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn executor_create(threads: usize) -> OpendalExecutorResult {
    match Executor::new(threads) {
        Ok(executor) => OpendalExecutorResult::ok(Arc::into_raw(Arc::new(executor)) as *mut c_void),
        Err(error) => OpendalExecutorResult::from_error(error),
    }
}

/// # Safety
///
/// - `executor` must be either null or a pointer returned by `executor_create`.
/// - The pointer must not be used after this call.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn executor_free(executor: *mut c_void) {
    if executor.is_null() {
        return;
    }

    // SAFETY: reclaims the reference handed out by `executor_create`.
    // Operators hold their own clones, so in-flight work keeps its runtime.
    unsafe {
        drop(Arc::from_raw(executor as *const Executor));
    }
}
