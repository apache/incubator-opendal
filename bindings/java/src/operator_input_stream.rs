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

use jni::EnvUnowned;
use jni::objects::JByteArray;
use jni::objects::JClass;
use opendal::blocking::StdBytesIterator;

use crate::error::ThrowException;

/// # Safety
///
/// `iterator` must point to a live iterator, with no other calls in progress.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorInputStream_disposeIterator<
    'local,
>(
    _: EnvUnowned<'local>,
    _: JClass<'local>,
    iterator: *mut StdBytesIterator,
) {
    unsafe {
        drop(Box::from_raw(iterator));
    }
}

/// # Safety
///
/// `iterator` must point to a live iterator, with no other calls in progress.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorInputStream_readNextBytes<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    iterator: *mut StdBytesIterator,
) -> JByteArray<'local> {
    env.with_env(|env| -> crate::Result<_> {
        let iterator = unsafe { &mut *iterator };
        match iterator.next().transpose().map_err(|err| {
            err.downcast::<opendal::Error>().unwrap_or_else(|err| {
                opendal::Error::new(opendal::ErrorKind::Unexpected, err.to_string())
            })
        })? {
            None => Ok(JByteArray::default()),
            Some(content) => Ok(env.byte_array_from_slice(&content)?),
        }
    })
    .resolve::<ThrowException>()
}
