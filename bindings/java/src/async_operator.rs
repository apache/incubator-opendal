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

use std::time::Duration;

use jni::Env;
use jni::EnvUnowned;
use jni::JavaVM;
use jni::jni_sig;
use jni::jni_str;
use jni::objects::JByteArray;
use jni::objects::JClass;
use jni::objects::JObject;
use jni::objects::JString;
use jni::objects::JValue;
use jni::sys::jlong;
use jni::sys::jsize;
use opendal::Entry;
use opendal::Operator;
use opendal::blocking;

use crate::Result;
use crate::convert::{bytes_to_jbytearray, jmap_to_hashmap, jstring_to_string};
use crate::error::ThrowException;
use crate::executor::Executor;
use crate::executor::executor_or_default;
use crate::make_metadata;
use crate::make_operator_info;
use crate::make_presigned_request;
use crate::{
    make_entry, make_list_options, make_read_options, make_stat_options, make_write_options,
};

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_apache_opendal_AsyncOperator_constructor<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    scheme: JString<'local>,
    map: JObject<'local>,
    executor: *const Executor,
) -> jlong {
    env.with_env(|env| intern_constructor(env, scheme, map, executor))
        .resolve::<ThrowException>()
}

fn intern_constructor(
    env: &mut Env,
    scheme: JString,
    map: JObject,
    executor: *const Executor,
) -> Result<jlong> {
    let scheme = jstring_to_string(env, &scheme)?;
    let map = jmap_to_hashmap(env, &map)?;
    let op = Operator::via_iter(scheme, map)?;
    let executor = executor_or_default(env, executor)?;
    let ctx = op
        .context()
        .with_executor(opendal::Executor::with(executor));
    Ok(Box::into_raw(Box::new(op.with_context(ctx))) as jlong)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_duplicate<'local>(
    _: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
) -> jlong {
    let op = unsafe { &mut *op };
    Box::into_raw(Box::new(op.clone())) as jlong
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_disposeInternal<'local>(
    _: EnvUnowned<'local>,
    _: JObject<'local>,
    op: *mut Operator,
) {
    unsafe {
        drop(Box::from_raw(op));
    }
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_write<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    content: JByteArray<'local>,
    write_options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_write(env, op, executor, path, content, write_options))
        .resolve::<ThrowException>()
}

fn intern_write(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    content: JByteArray,
    options: JObject,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let write_opts = make_write_options(env, &options)?;
    let path = jstring_to_string(env, &path)?;
    let content = env.convert_byte_array(content)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.write_options(&path, content, write_opts).await;
        complete_future(id, move |_env| {
            result?;
            Ok(JObject::null())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_stat<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    stat_options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_stat(env, op, executor, path, stat_options))
        .resolve::<ThrowException>()
}

fn intern_stat(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    options: JObject,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;
    let stat_opts = make_stat_options(env, &options)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.stat_options(&path, stat_opts).await;
        complete_future(id, move |env| make_metadata(env, result?));
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_read<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    read_options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_read(env, op, executor, path, read_options))
        .resolve::<ThrowException>()
}

fn intern_read(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    options: JObject,
) -> Result<jlong> {
    // Prepare inputs before spawning
    let id = request_id(env)?;
    let path_str = jstring_to_string(env, &path)?;
    let read_opts = make_read_options(env, &options)?;

    // Clone operator handle to move into the task
    let op_cloned = unsafe { &*op }.clone();

    executor_or_default(env, executor)?.spawn(async move {
        let result = op_cloned.read_options(&path_str, read_opts).await;
        complete_future(id, move |env| {
            let buffer = result?;
            Ok(bytes_to_jbytearray(env, buffer.to_bytes())?.into())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_delete<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
) -> jlong {
    env.with_env(|env| intern_delete(env, op, executor, path))
        .resolve::<ThrowException>()
}

fn intern_delete(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.delete(&path).await;
        complete_future(id, move |_env| {
            result?;
            Ok(JObject::null())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_makeBlockingOp<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
) -> jlong {
    env.with_env(|env| intern_make_blocking_op(env, op, executor))
        .resolve::<ThrowException>()
}

fn intern_make_blocking_op(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
) -> Result<jlong> {
    let op = unsafe { &mut *op };
    let op = executor_or_default(env, executor)?
        .enter_with(move || blocking::Operator::new(op.clone()))?;
    Ok(Box::into_raw(Box::new(op)) as jlong)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_makeOperatorInfo<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
) -> JObject<'local> {
    env.with_env(|env| intern_make_operator_info(env, op))
        .resolve::<ThrowException>()
}

fn intern_make_operator_info<'local>(
    env: &mut Env<'local>,
    op: *mut Operator,
) -> Result<JObject<'local>> {
    let op = unsafe { &mut *op };
    make_operator_info(env, op.info())
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_createDir<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
) -> jlong {
    env.with_env(|env| intern_create_dir(env, op, executor, path))
        .resolve::<ThrowException>()
}

fn intern_create_dir(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.create_dir(&path).await;
        complete_future(id, move |_env| {
            result?;
            Ok(JObject::null())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_copy<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    source_path: JString<'local>,
    target_path: JString<'local>,
) -> jlong {
    env.with_env(|env| intern_copy(env, op, executor, source_path, target_path))
        .resolve::<ThrowException>()
}

fn intern_copy(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    source_path: JString,
    target_path: JString,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let source_path = jstring_to_string(env, &source_path)?;
    let target_path = jstring_to_string(env, &target_path)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.copy(&source_path, &target_path).await;
        complete_future(id, move |_env| {
            result?;
            Ok(JObject::null())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_rename<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    source_path: JString<'local>,
    target_path: JString<'local>,
) -> jlong {
    env.with_env(|env| intern_rename(env, op, executor, source_path, target_path))
        .resolve::<ThrowException>()
}

fn intern_rename(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    source_path: JString,
    target_path: JString,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let source_path = jstring_to_string(env, &source_path)?;
    let target_path = jstring_to_string(env, &target_path)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.rename(&source_path, &target_path).await;
        complete_future(id, move |_env| {
            result?;
            Ok(JObject::null())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_removeAll<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
) -> jlong {
    env.with_env(|env| intern_remove_all(env, op, executor, path))
        .resolve::<ThrowException>()
}

fn intern_remove_all(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.delete_with(&path).recursive(true).await;
        complete_future(id, move |_env| {
            result?;
            Ok(JObject::null())
        });
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_list<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_list(env, op, executor, path, options))
        .resolve::<ThrowException>()
}

fn intern_list(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    options: JObject,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;
    let list_opts = make_list_options(env, &options)?;
    executor_or_default(env, executor)?.spawn(async move {
        let result = op.list_options(&path, list_opts).await;
        complete_future(id, move |env| make_entries(env, result?));
    });

    Ok(id)
}

fn make_entries<'local>(env: &mut Env<'local>, entries: Vec<Entry>) -> Result<JObject<'local>> {
    let jarray = env.new_object_array(
        entries.len() as jsize,
        jni_str!("org/apache/opendal/Entry"),
        JObject::null(),
    )?;

    for (idx, entry) in entries.into_iter().enumerate() {
        let entry = make_entry(env, entry)?;
        jarray.set_element(env, idx, &entry)?;
    }

    Ok(jarray.into())
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_presignRead<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    expire: jlong,
    options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_presign_read(env, op, executor, path, expire, options))
        .resolve::<ThrowException>()
}

fn intern_presign_read(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    expire: jlong,
    options: JObject,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let read_opts = make_read_options(env, &options)?;
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;
    let expire = Duration::from_nanos(expire as u64);

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.presign_read_options(&path, expire, read_opts).await;
        complete_future(id, move |env| make_presigned_request(env, result?));
    });

    Ok(id)
}
/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_presignWrite<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    expire: jlong,
    options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_presign_write(env, op, executor, path, expire, options))
        .resolve::<ThrowException>()
}

fn intern_presign_write(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    expire: jlong,
    options: JObject,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let write_opts = make_write_options(env, &options)?;
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;
    let expire = Duration::from_nanos(expire as u64);

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.presign_write_options(&path, expire, write_opts).await;
        complete_future(id, move |env| make_presigned_request(env, result?));
    });

    Ok(id)
}

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncOperator_presignStat<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    op: *mut Operator,
    executor: *const Executor,
    path: JString<'local>,
    expire: jlong,
    options: JObject<'local>,
) -> jlong {
    env.with_env(|env| intern_presign_stat(env, op, executor, path, expire, options))
        .resolve::<ThrowException>()
}

fn intern_presign_stat(
    env: &mut Env,
    op: *mut Operator,
    executor: *const Executor,
    path: JString,
    expire: jlong,
    options: JObject,
) -> Result<jlong> {
    let op = unsafe { &*op }.clone();
    let stat_opts = make_stat_options(env, &options)?;
    let id = request_id(env)?;

    let path = jstring_to_string(env, &path)?;
    let expire = Duration::from_nanos(expire as u64);

    executor_or_default(env, executor)?.spawn(async move {
        let result = op.presign_stat_options(&path, expire, stat_opts).await;
        complete_future(id, move |env| make_presigned_request(env, result?));
    });

    Ok(id)
}

/// Complete the Java `CompletableFuture` identified by `id`.
///
/// This runs on a `tokio` worker thread that was permanently attached to the
/// JVM in `on_thread_start`, so `attach_current_thread` only pushes a scoped
/// JNI frame here. The `build` closure turns the awaited operation result into a
/// Java object within that frame; on error the future is completed
/// exceptionally. All local references created during completion live and die
/// within the frame.
fn complete_future<F>(id: jlong, build: F)
where
    F: for<'a> FnOnce(&mut Env<'a>) -> Result<JObject<'a>>,
{
    let vm = JavaVM::singleton().expect("JavaVM singleton must be initialized");
    vm.attach_current_thread(|env| -> Result<()> {
        let future = get_future(env, id)?;
        match build(env) {
            Ok(object) => {
                env.call_method(
                    &future,
                    jni_str!("complete"),
                    jni_sig!("(Ljava/lang/Object;)Z"),
                    &[JValue::Object(&object)],
                )?;
            }
            Err(err) => {
                let exception = err.to_exception(env)?;
                env.call_method(
                    &future,
                    jni_str!("completeExceptionally"),
                    jni_sig!("(Ljava/lang/Throwable;)Z"),
                    &[JValue::Object(&exception)],
                )?;
            }
        }
        Ok(())
    })
    .expect("complete future must succeed");
}

fn request_id(env: &mut Env) -> Result<jlong> {
    Ok(env
        .call_static_method(
            jni_str!("org/apache/opendal/AsyncOperator$AsyncRegistry"),
            jni_str!("requestId"),
            jni_sig!("()J"),
            &[],
        )?
        .j()?)
}

fn get_future<'local>(env: &mut Env<'local>, id: jlong) -> Result<JObject<'local>> {
    Ok(env
        .call_static_method(
            jni_str!("org/apache/opendal/AsyncOperator$AsyncRegistry"),
            jni_str!("get"),
            jni_sig!("(J)Ljava/util/concurrent/CompletableFuture;"),
            &[JValue::Long(id)],
        )?
        .l()?)
}
