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
use std::sync::Mutex as StdMutex;
use std::sync::Weak;

use asyncband::mutex::Mutex;
use futures::FutureExt;
use futures::TryStreamExt;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::types::PyWeakrefReference;
use pyo3_async_runtimes::tokio::future_into_py;

use crate::*;

const ASYNC_LISTER_BATCH_SIZE: usize = 64;

#[pyclass(unsendable, module = "opendal")]
pub struct BlockingLister(ocore::blocking::Lister);

impl BlockingLister {
    /// Create a new blocking lister.
    pub fn new(inner: ocore::blocking::Lister) -> Self {
        Self(inner)
    }
}

#[pymethods]
impl BlockingLister {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __next__(mut slf: PyRefMut<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        match slf.0.next() {
            Some(Ok(entry)) => Ok(Some(Entry::new(entry).into_py_any(slf.py())?)),
            Some(Err(err)) => {
                let pyerr = format_pyerr(err);
                Err(pyerr)
            }
            None => Ok(None),
        }
    }
}

#[pyclass(module = "opendal")]
pub struct AsyncLister {
    state: Arc<Mutex<AsyncListerState>>,
    requeued: Arc<StdMutex<VecDeque<ocore::Entry>>>,
    deliveries: Arc<StdMutex<Vec<Weak<EntryDelivery>>>>,
}

struct AsyncListerState {
    lister: ocore::Lister,
    buffered: VecDeque<ocore::Result<ocore::Entry>>,
    exhausted: bool,
}

impl AsyncListerState {
    fn new(lister: ocore::Lister) -> Self {
        Self {
            lister,
            buffered: VecDeque::with_capacity(ASYNC_LISTER_BATCH_SIZE - 1),
            exhausted: false,
        }
    }

    async fn next(
        &mut self,
        requeued: &StdMutex<VecDeque<ocore::Entry>>,
    ) -> Option<ocore::Result<ocore::Entry>> {
        if let Some(entry) = requeued.lock().unwrap().pop_front() {
            return Some(Ok(entry));
        }
        if let Some(entry) = self.buffered.pop_front() {
            return Some(entry);
        }
        if self.exhausted {
            return None;
        }

        let first = match self.lister.try_next().await {
            Ok(Some(entry)) => Ok(entry),
            Ok(None) => {
                self.exhausted = true;
                return None;
            }
            Err(err) => {
                self.exhausted = true;
                return Some(Err(err));
            }
        };

        // Only drain entries that are ready now. The first pending item stays in
        // `Lister` and becomes the next refill's awaited item.
        for _ in 1..ASYNC_LISTER_BATCH_SIZE {
            match self.lister.try_next().now_or_never() {
                Some(Ok(Some(entry))) => self.buffered.push_back(Ok(entry)),
                Some(Ok(None)) => {
                    self.exhausted = true;
                    break;
                }
                Some(Err(err)) => {
                    self.buffered.push_back(Err(err));
                    self.exhausted = true;
                    break;
                }
                None => break,
            }
        }

        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_async_lister_state_batches_ready_entries() {
        pyo3_async_runtimes::tokio::get_runtime().block_on(async {
            let op = ocore::Operator::new(ocore::services::Memory::default()).unwrap();
            let expected: Vec<_> = (0..130).map(|idx| format!("items/{idx:03}")).collect();
            for path in &expected {
                op.write(path, b"test".to_vec()).await.unwrap();
            }

            let mut state = AsyncListerState::new(op.lister("items/").await.unwrap());
            let requeued = StdMutex::new(VecDeque::new());
            let first = state.next(&requeued).await.unwrap().unwrap();
            assert_eq!(first.path(), expected[0]);
            assert_eq!(state.buffered.len(), ASYNC_LISTER_BATCH_SIZE - 1);

            let mut actual = vec![first.path().to_string()];
            while let Some(entry) = state.next(&requeued).await {
                actual.push(entry.unwrap().path().to_string());
                assert!(state.buffered.len() < ASYNC_LISTER_BATCH_SIZE);
            }

            assert_eq!(actual, expected);
            assert!(state.exhausted);
            assert!(state.buffered.is_empty());
            assert!(state.next(&requeued).await.is_none());

            let requeued = Arc::new(StdMutex::new(VecDeque::new()));
            let delivery = EntryDelivery::new(requeued.clone());
            delivery.reserve(first.clone());
            delivery.complete(true);
            assert_eq!(
                requeued.lock().unwrap().pop_front().unwrap().path(),
                first.path()
            );

            let delivery = EntryDelivery::new(requeued.clone());
            delivery.complete(true);
            delivery.reserve(first.clone());
            assert_eq!(
                requeued.lock().unwrap().pop_front().unwrap().path(),
                first.path()
            );

            let delivery = EntryDelivery::new(requeued.clone());
            delivery.reserve(first);
            delivery.complete(false);
            assert!(requeued.lock().unwrap().is_empty());
        });
    }
}

impl AsyncLister {
    pub fn new(lister: ocore::Lister) -> Self {
        Self {
            state: Arc::new(Mutex::new(AsyncListerState::new(lister))),
            requeued: Arc::new(StdMutex::new(VecDeque::new())),
            deliveries: Arc::new(StdMutex::new(Vec::new())),
        }
    }

    fn reconcile_deliveries(&self, py: Python<'_>) -> PyResult<()> {
        // asyncio schedules done callbacks on the next loop turn. Reconcile here
        // as well so iteration immediately after CancelledError cannot skip an entry.
        let deliveries = {
            let mut tracked = self.deliveries.lock().unwrap();
            let mut active = Vec::with_capacity(tracked.len());
            tracked.retain(|delivery| {
                if let Some(delivery) = delivery.upgrade() {
                    active.push(delivery);
                    true
                } else {
                    false
                }
            });
            active
        };

        for delivery in deliveries {
            delivery.reconcile(py)?;
        }
        Ok(())
    }
}

struct EntryDeliveryState {
    entry: Option<ocore::Entry>,
    cancelled: bool,
    future: Option<Py<PyWeakrefReference>>,
}

struct EntryDelivery {
    state: StdMutex<EntryDeliveryState>,
    requeued: Arc<StdMutex<VecDeque<ocore::Entry>>>,
}

impl EntryDelivery {
    fn new(requeued: Arc<StdMutex<VecDeque<ocore::Entry>>>) -> Self {
        Self {
            state: StdMutex::new(EntryDeliveryState {
                entry: None,
                cancelled: false,
                future: None,
            }),
            requeued,
        }
    }

    fn set_future(&self, future: Py<PyWeakrefReference>) {
        self.state.lock().unwrap().future = Some(future);
    }

    fn reserve(&self, entry: ocore::Entry) -> ocore::Entry {
        let result = entry.clone();
        let mut state = self.state.lock().unwrap();
        if state.cancelled {
            drop(state);
            self.requeued.lock().unwrap().push_front(entry);
        } else {
            state.entry = Some(entry);
        }
        result
    }

    fn complete(&self, cancelled: bool) {
        let entry = {
            let mut state = self.state.lock().unwrap();
            state.cancelled = cancelled;
            state.future = None;
            state.entry.take().filter(|_| cancelled)
        };
        if let Some(entry) = entry {
            self.requeued.lock().unwrap().push_front(entry);
        }
    }

    fn reconcile(&self, py: Python<'_>) -> PyResult<()> {
        let future = {
            let state = self.state.lock().unwrap();
            state
                .future
                .as_ref()
                .and_then(|future| future.bind(py).upgrade())
        };
        if let Some(future) = future
            && future
                .call_method0(pyo3::intern!(py, "cancelled"))?
                .is_truthy()?
        {
            self.complete(true);
        }
        Ok(())
    }
}

#[pyclass]
struct EntryDeliveryCallback(Arc<EntryDelivery>);

#[pymethods]
impl EntryDeliveryCallback {
    fn __call__(&self, future: &Bound<PyAny>) -> PyResult<()> {
        let cancelled = future
            .call_method0(pyo3::intern!(future.py(), "cancelled"))?
            .is_truthy()?;
        self.0.complete(cancelled);
        Ok(())
    }
}

fn complete_future(future: Bound<'_, PyAny>, result: PyResult<Py<PyAny>>) -> PyResult<Py<PyAny>> {
    let py = future.py();
    match result {
        Ok(value) => {
            future.call_method1(pyo3::intern!(py, "set_result"), (value,))?;
        }
        Err(err) => {
            future.call_method1(pyo3::intern!(py, "set_exception"), (err.into_value(py),))?;
        }
    }
    Ok(future.unbind())
}

#[pymethods]
impl AsyncLister {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __anext__(slf: PyRefMut<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        slf.reconcile_deliveries(slf.py())?;

        if let Some(mut state) = slf.state.try_lock() {
            let mut requeued = slf.requeued.lock().unwrap();
            if !requeued.is_empty() || !state.buffered.is_empty() || state.exhausted {
                let py = slf.py();
                let future = pyo3_async_runtimes::get_running_loop(py)?
                    .call_method0(pyo3::intern!(py, "create_future"))?;
                let entry = requeued
                    .pop_front()
                    .map(Ok)
                    .or_else(|| state.buffered.pop_front());
                let result = match entry {
                    Some(Ok(entry)) => Entry::new(entry).into_py_any(py),
                    Some(Err(err)) => Err(format_pyerr(err)),
                    None => Err(PyStopAsyncIteration::new_err("stream exhausted")),
                };
                return complete_future(future, result).map(Some);
            }
        }

        let state = slf.state.clone();
        let requeued = slf.requeued.clone();
        let delivery = Arc::new(EntryDelivery::new(requeued.clone()));
        let delivery_task = delivery.clone();
        let fut = future_into_py(slf.py(), async move {
            let mut state = state.lock().await;
            match state.next(&requeued).await {
                Some(Ok(entry)) => Python::attach(|py| {
                    let entry = delivery_task.reserve(entry);
                    let py_obj = Entry::new(entry).into_py_any(py)?;
                    Ok(Some(py_obj))
                }),
                Some(Err(err)) => Err(format_pyerr(err)),
                None => Err(PyStopAsyncIteration::new_err("stream exhausted")),
            }
        })?;
        delivery.set_future(PyWeakrefReference::new(&fut)?.unbind());
        slf.deliveries
            .lock()
            .unwrap()
            .push(Arc::downgrade(&delivery));
        fut.call_method1(
            pyo3::intern!(slf.py(), "add_done_callback"),
            (EntryDeliveryCallback(delivery),),
        )?;

        Ok(Some(fut.into()))
    }
}
