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
use cxx_async::CxxAsyncException;
use opendal as od;
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::str::FromStr;

#[cxx::bridge(namespace = "opendal::ffi::async")]
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

    extern "Rust" {
        type Operator;

        fn new_operator(scheme: &str, configs: Vec<HashMapValue>) -> Result<Box<Operator>>;
        unsafe fn operator_read(op: OperatorPtr, path: String) -> RustFutureRead;
        unsafe fn operator_write(op: OperatorPtr, path: String, bs: Vec<u8>) -> RustFutureWrite;
    }

    extern "C++" {
        type RustFutureRead = super::RustFutureRead;
        type RustFutureWrite = super::RustFutureWrite;
    }
}

#[cxx_async::bridge(namespace = opendal::ffi::async)]
unsafe impl Future for RustFutureRead {
    type Output = Vec<u8>;
}

#[cxx_async::bridge(namespace = opendal::ffi::async)]
unsafe impl Future for RustFutureWrite {
    type Output = ();
}

pub struct Operator(od::Operator);

fn new_operator(scheme: &str, configs: Vec<ffi::HashMapValue>) -> Result<Box<Operator>> {
    let scheme = od::Scheme::from_str(scheme)?;

    let map: HashMap<String, String> = configs
        .into_iter()
        .map(|value| (value.key, value.value))
        .collect();

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

unsafe fn operator_read(op: ffi::OperatorPtr, path: String) -> RustFutureRead {
    RustFutureRead::fallible(async move {
        Ok((*op)
            .0
            .read(&path)
            .await
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?
            .to_vec())
    })
}

unsafe fn operator_write(op: ffi::OperatorPtr, path: String, bs: Vec<u8>) -> RustFutureWrite {
    RustFutureWrite::fallible(async move {
        Ok((*op)
            .0
            .write(&path, bs)
            .await
            .map(|_| ())
            .map_err(|e| CxxAsyncException::new(e.to_string().into_boxed_str()))?)
    })
}
