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
use jni::jni_str;
use jni::objects::JByteArray;
use jni::objects::JClass;
use jni::objects::JObject;
use jni::objects::JObjectArray;
use jni::objects::JString;
use jni::sys::jlong;
use jni::sys::jsize;
use opendal::blocking;

use crate::Result;
use crate::convert::{bytes_to_jbytearray, jstring_to_string};
use crate::error::ThrowException;
use crate::make_metadata;
use crate::{make_entry, make_list_options, make_stat_options, make_write_options};

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_disposeInternal<'local>(
    _: EnvUnowned<'local>,
    _: JObject<'local>,
    op: *mut blocking::Operator,
) {
    unsafe {
        drop(Box::from_raw(op));
    }
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_duplicate<'local>(
    _: EnvUnowned<'local>,
    _: JObject<'local>,
    op: *mut blocking::Operator,
) -> jlong {
    let op = unsafe { &mut *op };
    Box::into_raw(Box::new(op.clone())) as jlong
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_read<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
    read_options: JObject<'local>,
) -> JByteArray<'local> {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_read(env, op_ref, path, read_options)
    })
    .resolve::<ThrowException>()
}

fn intern_read<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
    options: JObject<'local>,
) -> Result<JByteArray<'local>> {
    use crate::make_read_options;

    let path = jstring_to_string(env, &path)?;
    let options = make_read_options(env, &options)?;
    let content = op.read_options(&path, options)?;

    bytes_to_jbytearray(env, content.to_bytes())
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_write<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
    content: JByteArray<'local>,
    write_options: JObject<'local>,
) {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_write(env, op_ref, path, content, write_options)
    })
    .resolve::<ThrowException>()
}

fn intern_write<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
    content: JByteArray<'local>,
    options: JObject<'local>,
) -> Result<()> {
    let path = jstring_to_string(env, &path)?;
    let content = env.convert_byte_array(content)?;
    let write_opts = make_write_options(env, &options)?;

    let _ = op.write_options(&path, content, write_opts)?;
    Ok(())
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_stat<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
    stat_options: JObject<'local>,
) -> JObject<'local> {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_stat(env, op_ref, path, stat_options)
    })
    .resolve::<ThrowException>()
}

fn intern_stat<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
    options: JObject<'local>,
) -> Result<JObject<'local>> {
    let path = jstring_to_string(env, &path)?;
    let stat_opts = make_stat_options(env, &options)?;
    let metadata = op.stat_options(&path, stat_opts)?;
    make_metadata(env, metadata)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_delete<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
) {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_delete(env, op_ref, path)
    })
    .resolve::<ThrowException>()
}

fn intern_delete<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
) -> Result<()> {
    let path = jstring_to_string(env, &path)?;
    Ok(op.delete(&path)?)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_createDir<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
) {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_create_dir(env, op_ref, path)
    })
    .resolve::<ThrowException>()
}

fn intern_create_dir<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
) -> Result<()> {
    let path = jstring_to_string(env, &path)?;
    Ok(op.create_dir(&path)?)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_copy<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    source_path: JString<'local>,
    target_path: JString<'local>,
) {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_copy(env, op_ref, source_path, target_path)
    })
    .resolve::<ThrowException>()
}

fn intern_copy<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    source_path: JString<'local>,
    target_path: JString<'local>,
) -> Result<()> {
    let source_path = jstring_to_string(env, &source_path)?;
    let target_path = jstring_to_string(env, &target_path)?;

    op.copy(&source_path, &target_path)?;
    Ok(())
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_rename<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    source_path: JString<'local>,
    target_path: JString<'local>,
) {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_rename(env, op_ref, source_path, target_path)
    })
    .resolve::<ThrowException>()
}

fn intern_rename<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    source_path: JString<'local>,
    target_path: JString<'local>,
) -> Result<()> {
    let source_path = jstring_to_string(env, &source_path)?;
    let target_path = jstring_to_string(env, &target_path)?;

    Ok(op.rename(&source_path, &target_path)?)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_removeAll<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
) {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_remove_all(env, op_ref, path)
    })
    .resolve::<ThrowException>()
}

fn intern_remove_all<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
) -> Result<()> {
    use opendal::options::DeleteOptions;
    let path = jstring_to_string(env, &path)?;

    Ok(op.delete_options(
        &path,
        DeleteOptions {
            recursive: true,
            ..Default::default()
        },
    )?)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_Operator_list<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut blocking::Operator,
    path: JString<'local>,
    options: JObject<'local>,
) -> JObjectArray<'local> {
    env.with_env(|env| {
        let op_ref = unsafe { &mut *op };
        intern_list(env, op_ref, path, options)
    })
    .resolve::<ThrowException>()
}

fn intern_list<'local>(
    env: &mut Env<'local>,
    op: &mut blocking::Operator,
    path: JString<'local>,
    options: JObject<'local>,
) -> Result<JObjectArray<'local>> {
    let path = jstring_to_string(env, &path)?;
    let list_opts = make_list_options(env, &options)?;
    let entries = op.list_options(&path, list_opts)?;

    let jarray = env.new_object_array(
        entries.len() as jsize,
        jni_str!("org/apache/opendal/Entry"),
        JObject::null(),
    )?;

    for (idx, entry) in entries.into_iter().enumerate() {
        let entry = make_entry(env, entry)?;
        jarray.set_element(env, idx, &entry)?;
    }

    Ok(jarray)
}
