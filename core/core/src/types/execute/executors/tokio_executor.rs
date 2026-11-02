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

use crate::raw::*;
use crate::*;

/// Executor that uses the [`tokio::task::spawn`] to execute futures.
#[derive(Default)]
pub struct TokioExecutor {}

impl Execute for TokioExecutor {
    /// Tokio's JoinHandle has its own `abort` support, so dropping handle won't cancel the task.
    fn execute(&self, f: BoxedStaticFuture<()>) {
        let _handle = tokio::task::spawn(f);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use tokio::time::sleep;

    use super::*;
    use crate::Executor;

    #[tokio::test]
    async fn test_tokio_executor() {
        let executor = Executor::with(TokioExecutor::default());

        let finished = Arc::new(AtomicBool::new(false));

        let finished_clone = finished.clone();
        let _task = executor.execute(async move {
            sleep(Duration::from_secs(1)).await;
            finished_clone.store(true, Ordering::Relaxed);
        });

        sleep(Duration::from_secs(2)).await;
        // Task must have been finished even without await task.
        assert!(finished.load(Ordering::Relaxed))
    }
}
