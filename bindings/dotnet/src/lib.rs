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

use std::collections::HashMap;
use std::os::raw::c_char;
use std::str::FromStr;

/// # Safety
///
/// Not yet.
#[no_mangle]
pub unsafe extern "C" fn blocking_operator_construct(
    scheme: *const c_char,
) -> *const opendal::BlockingOperator {
    if scheme.is_null() {
        return std::ptr::null();
    }

    let scheme = match opendal::Scheme::from_str(std::ffi::CStr::from_ptr(scheme).to_str().unwrap())
    {
        Ok(scheme) => scheme,
        Err(_) => return std::ptr::null(),
    };

    let mut map = HashMap::<String, String>::default();
    map.insert("root".to_string(), "/tmp".to_string());
    let op = match opendal::Operator::via_iter(scheme, map) {
        Ok(op) => op.blocking(),
        Err(err) => {
            println!("err={err:?}");
            return std::ptr::null();
        }
    };

    Box::leak(Box::new(op))
}

/// # Safety
///
/// Not yet.
#[no_mangle]
pub unsafe extern "C" fn blocking_operator_write(
    op: *const opendal::BlockingOperator,
    path: *const c_char,
    content: *const c_char,
) {
    let op = &*(op);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let content = std::ffi::CStr::from_ptr(content).to_str().unwrap();
    op.write(path, content.to_owned()).map(|_| ()).unwrap()
}

/// # Safety
///
/// Not yet.
#[no_mangle]
pub unsafe extern "C" fn blocking_operator_read(
    op: *const opendal::BlockingOperator,
    path: *const c_char,
) -> *const c_char {
    let op = &*(op);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let mut res = op.read(path).unwrap().to_vec();
    res.push(0);
    std::ffi::CString::from_vec_with_nul(res)
        .unwrap()
        .into_raw()
}
