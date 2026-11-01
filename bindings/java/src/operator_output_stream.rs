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

use jni::Env;
use jni::EnvUnowned;
use jni::objects::JByteArray;
use jni::objects::JClass;
use jni::objects::JObject;
use jni::objects::JString;
use jni::sys::jlong;
use opendal::blocking;

use crate::convert::jstring_to_string;
use crate::error::ThrowException;

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorOutputStream_constructWriter<
    'local,
>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
    options: JObject<'local>,
) -> jlong {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_construct_write(env, op_ref, path, options)
    })
    .resolve::<ThrowException>()
}

fn intern_construct_write(
    env: &mut Env,
    op: &mut blocking::Operator,
    path: JString,
    options: JObject,
) -> crate::Result<jlong> {
    use crate::make_write_options;

    let path = jstring_to_string(env, &path)?;
    let options = make_write_options(env, &options)?;
    let writer = op.writer_options(&path, options)?;
    Ok(Box::into_raw(Box::new(writer)) as jlong)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorOutputStream_disposeWriter<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    writer: *mut blocking::Writer,
) {
    env.with_env(|_env| {
        let mut writer = unsafe { Box::from_raw(writer) };
        intern_dispose_write(&mut writer)
    })
    .resolve::<ThrowException>()
}

fn intern_dispose_write(writer: &mut blocking::Writer) -> crate::Result<()> {
    writer.close()?;
    Ok(())
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OperatorOutputStream_writeBytes<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    writer: *mut blocking::Writer,
    content: JByteArray<'local>,
) {
    env.with_env(|env| {
        let writer_ref = unsafe { &mut *writer };
        intern_write_bytes(env, writer_ref, content)
    })
    .resolve::<ThrowException>()
}

fn intern_write_bytes(
    env: &mut Env,
    writer: &mut blocking::Writer,
    content: JByteArray,
) -> crate::Result<()> {
    let content = env.convert_byte_array(content)?;
    writer.write(content)?;
    Ok(())
}
