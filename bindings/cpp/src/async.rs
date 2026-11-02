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

use anyhow::Result;
use asyncband::mutex::Mutex;
use cxx_async::CxxAsyncException;
use opendal as od;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

#[cxx::bridge(namespace = opendal::ffi::async_op)]
mod ffi {
    struct HashMapValue {
        key: String,
        value: String,
    }

    // here we have to use raw pointers since:
    // 1. cxx-async futures requires 'static lifetime (and it's hard to change for now)
    // 2. cxx SharedPtr cannot accept Rust types as type parameters for now
    pub struct OperatorPtr {
        op: *const Operator,
    }

    pub struct ReaderPtr {
        id: usize,
    }

    pub struct ListerPtr {
        id: usize,
    }

    extern "Rust" {
        type Operator;

        fn new_operator(scheme: &str, configs: Vec<HashMapValue>) -> Result<Box<Operator>>;
        unsafe fn operator_read(op: OperatorPtr, path: String) -> RustFutureRead;
        unsafe fn operator_write(op: OperatorPtr, path: String, bs: &[u8]) -> RustFutureWrite;
        unsafe fn operator_list(op: OperatorPtr, path: String) -> RustFutureList;
        unsafe fn operator_exists(op: OperatorPtr, path: String) -> RustFutureBool;
        unsafe fn operator_create_dir(op: OperatorPtr, path: String) -> RustFutureWrite;
        unsafe fn operator_copy(op: OperatorPtr, from: String, to: String) -> RustFutureWrite;
        unsafe fn operator_rename(op: OperatorPtr, from: String, to: String) -> RustFutureWrite;
        unsafe fn operator_delete(op: OperatorPtr, path: String) -> RustFutureWrite;
        unsafe fn operator_reader(op: OperatorPtr, path: String) -> RustFutureReaderId;
        unsafe fn operator_lister(op: OperatorPtr, path: String) -> RustFutureListerId;

        unsafe fn reader_read(reader: ReaderPtr, start: u64, len: u64) -> RustFutureRead;
        unsafe fn lister_next(lister: ListerPtr) -> RustFutureEntryOption;

        fn delete_reader(reader: ReaderPtr);
        fn delete_lister(lister: ListerPtr);
    }

    extern "C++" {
        #[namespace = opendal::ffi::async_op]
        type RustFutureRead = super::RustFutureRead;
        #[namespace = opendal::ffi::async_op]
        type RustFutureWrite = super::RustFutureWrite;
        #[namespace = opendal::ffi::async_op]
        type RustFutureList = super::RustFutureList;
        #[namespace = opendal::ffi::async_op]
        type RustFutureBool = super::RustFutureBool;
        #[namespace = opendal::ffi::async_op]
        type RustFutureReaderId = super::RustFutureReaderId;
        #[namespace = opendal::ffi::async_op]
        type RustFutureListerId = super::RustFutureListerId;
        #[namespace = opendal::ffi::async_op]
        type RustFutureEntryOption = super::RustFutureEntryOption;
    }
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureRead {
    type Output = Vec<u8>;
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureWrite {
    type Output = ();
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureList {
    type Output = Vec<String>;
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureBool {
    type Output = bool;
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureReaderId {
    type Output = usize;
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureListerId {
    type Output = usize;
}

#[cxx_async::bridge(namespace = opendal::ffi::async_op)]
unsafe impl Future for RustFutureEntryOption {
    type Output = String;
}

pub struct Operator(od::Operator);

// Global storage for readers and listers to avoid Send issues with raw pointers
static READER_STORAGE: OnceLock<Mutex<HashMap<usize, Arc<od::Reader>>>> = OnceLock::new();
static READER_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();
static LISTER_STORAGE: OnceLock<Mutex<HashMap<usize, Arc<Mutex<od::Lister>>>>> = OnceLock::new();
static LISTER_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();

fn get_reader_storage() -> &'static Mutex<HashMap<usize, Arc<od::Reader>>> {
    READER_STORAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_reader_counter() -> &'static Mutex<usize> {
    READER_COUNTER.get_or_init(|| Mutex::new(0))
}

fn get_lister_storage() -> &'static Mutex<HashMap<usize, Arc<Mutex<od::Lister>>>> {
    LISTER_STORAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_lister_counter() -> &'static Mutex<usize> {
    LISTER_COUNTER.get_or_init(|| Mutex::new(0))
}

fn new_operator(scheme: &str, configs: Vec<ffi::HashMapValue>) -> Result<Box<Operator>> {
    let map: HashMap<String, String> = configs
        .into_iter()
        .map(|value| (value.key, value.value))
        .collect();

    od::init_default_registry();

    let op = Box::new(Operator(od::Operator::via_iter(scheme, map)?));

    Ok(op)
}

impl Deref for ffi::OperatorPtr {
    type Target = Operator;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.op }
    }
}

unsafe impl Send for ffi::OperatorPtr {}
unsafe impl Send for ffi::ReaderPtr {}
unsafe impl Send for ffi::ListerPtr {}

unsafe fn operator_read(op: ffi::OperatorPtr, path: String) -> RustFutureRead {
    RustFutureRead::fallible(async move {
        Ok(op
            .0
            .read(&path)
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?
            .to_vec())
    })
}

unsafe fn operator_write(op: ffi::OperatorPtr, path: String, bs: &[u8]) -> RustFutureWrite {
    // Copy before building the future, which outlives the borrowed slice.
    let bs = bs.to_vec();
    RustFutureWrite::fallible(async move {
        op.0.write(&path, bs)
            .await
            .map(|_| ())
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))
    })
}

unsafe fn operator_list(op: ffi::OperatorPtr, path: String) -> RustFutureList {
    RustFutureList::fallible(async move {
        let entries =
            op.0.list(&path)
                .await
                .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?;
        Ok(entries.into_iter().map(|e| e.path().to_string()).collect())
    })
}

unsafe fn operator_exists(op: ffi::OperatorPtr, path: String) -> RustFutureBool {
    RustFutureBool::fallible(async move {
        op.0.exists(&path)
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))
    })
}

unsafe fn operator_create_dir(op: ffi::OperatorPtr, path: String) -> RustFutureWrite {
    RustFutureWrite::fallible(async move {
        op.0.create_dir(&path)
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))
    })
}

unsafe fn operator_copy(op: ffi::OperatorPtr, from: String, to: String) -> RustFutureWrite {
    RustFutureWrite::fallible(async move {
        op.0.copy(&from, &to)
            .await
            .map(|_| ())
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))
    })
}

unsafe fn operator_rename(op: ffi::OperatorPtr, from: String, to: String) -> RustFutureWrite {
    RustFutureWrite::fallible(async move {
        op.0.rename(&from, &to)
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))
    })
}

unsafe fn operator_delete(op: ffi::OperatorPtr, path: String) -> RustFutureWrite {
    RustFutureWrite::fallible(async move {
        op.0.delete(&path)
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))
    })
}

unsafe fn operator_reader(op: ffi::OperatorPtr, path: String) -> RustFutureReaderId {
    RustFutureReaderId::fallible(async move {
        let reader =
            op.0.reader(&path)
                .await
                .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?;

        // Store the reader in global storage and return an ID
        let reader_arc = Arc::new(reader);
        let mut counter = get_reader_counter().lock().await;
        *counter += 1;
        let id = *counter;

        let mut storage = get_reader_storage().lock().await;
        storage.insert(id, reader_arc);

        Ok(id)
    })
}

unsafe fn operator_lister(op: ffi::OperatorPtr, path: String) -> RustFutureListerId {
    RustFutureListerId::fallible(async move {
        let lister =
            op.0.lister(&path)
                .await
                .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?;

        // Store the lister in global storage and return an ID
        let lister_arc = Arc::new(Mutex::new(lister));
        let mut counter = get_lister_counter().lock().await;
        *counter += 1;
        let id = *counter;

        let mut storage = get_lister_storage().lock().await;
        storage.insert(id, lister_arc);

        Ok(id)
    })
}

unsafe fn reader_read(reader: ffi::ReaderPtr, start: u64, len: u64) -> RustFutureRead {
    RustFutureRead::fallible(async move {
        let storage = get_reader_storage().lock().await;
        let reader_arc = storage
            .get(&reader.id)
            .ok_or_else(|| CxxAsyncException::new("Invalid reader ID".into()))?
            .clone();
        drop(storage);

        let buffer = reader_arc
            .read(start..(start + len))
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?;
        Ok(buffer.to_vec())
    })
}

unsafe fn lister_next(lister: ffi::ListerPtr) -> RustFutureEntryOption {
    RustFutureEntryOption::fallible(async move {
        use futures::TryStreamExt;

        let storage = get_lister_storage().lock().await;
        let lister_arc = storage
            .get(&lister.id)
            .ok_or_else(|| CxxAsyncException::new("Invalid lister ID".into()))?
            .clone();
        drop(storage);

        let mut lister_guard = lister_arc.lock().await;
        match lister_guard.try_next().await {
            Ok(Some(entry)) => Ok(entry.path().to_string()),
            Ok(None) => Ok(String::new()), // Empty string indicates end of iteration
            Err(e) => Err(CxxAsyncException::new(e.to_string().into_boxed_str())),
        }
    })
}

fn delete_reader(reader: ffi::ReaderPtr) {
    // Use blocking lock since this is called from C++ destructors
    if let Some(mut storage) = get_reader_storage().try_lock() {
        storage.remove(&reader.id);
    }
    // If we can't get the lock immediately, we'll just skip cleanup
    // This is better than panicking in a destructor
}

fn delete_lister(lister: ffi::ListerPtr) {
    // Use blocking lock since this is called from C++ destructors
    if let Some(mut storage) = get_lister_storage().try_lock() {
        storage.remove(&lister.id);
    }
    // If we can't get the lock immediately, we'll just skip cleanup
    // This is better than panicking in a destructor
}
