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
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::jlong;
use opendal::blocking;

use crate::convert;
use crate::error::ThrowException;

/// # Safety
///
/// `op` must point to a live blocking operator for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_createReader<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *const blocking::Operator,
    path: JString<'local>,
    options: JObject<'local>,
) -> jlong {
    env.with_env(|env| -> crate::Result<_> {
        let op = unsafe { &*op };
        let path = convert::jstring_to_string(env, &path)?;
        let options = crate::make_reader_options(env, &options)?;
        let reader = op.reader_options(&path, options)?;
        Ok(Box::into_raw(Box::new(reader)) as jlong)
    })
    .resolve::<ThrowException>()
}

/// # Safety
///
/// `reader` must point to a live blocking reader for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorReader_readBytes<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    reader: *const blocking::Reader,
    offset: jlong,
    length: jlong,
) -> JByteArray<'local> {
    env.with_env(|env| -> crate::Result<_> {
        let reader = unsafe { &*reader };
        let range = convert::offset_length_to_range(offset, length)?;
        let content = reader.read(range)?;
        convert::bytes_to_jbytearray(env, content.to_vec())
    })
    .resolve::<ThrowException>()
}

/// # Safety
///
/// `reader` must point to a live blocking reader for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorReader_createBytesIterator<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    reader: *const blocking::Reader,
    offset: jlong,
    length: jlong,
) -> jlong {
    env.with_env(|_| -> crate::Result<_> {
        let reader = unsafe { &*reader };
        let range = convert::offset_length_to_range(offset, length)?;
        let iter = reader.clone().into_bytes_iterator(range)?;
        Ok(Box::into_raw(Box::new(iter)) as jlong)
    })
    .resolve::<ThrowException>()
}

/// # Safety
///
/// `reader` must be a live handle allocated by `Operator.createReader`, with no calls in progress.
/// It must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorReader_disposeReader<'local>(
    _: EnvUnowned<'local>,
    _: JClass<'local>,
    reader: *mut blocking::Reader,
) {
    unsafe { drop(Box::from_raw(reader)) };
}
