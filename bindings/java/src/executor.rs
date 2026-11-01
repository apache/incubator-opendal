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

use std::ffi::c_void;
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::available_parallelism;

use jni::Env;
use jni::EnvUnowned;
use jni::JavaVM;
use jni::jni_sig;
use jni::jni_str;
use jni::objects::JClass;
use jni::objects::JClassLoader;
use jni::objects::JObject;
use jni::objects::JValue;
use jni::sys::{jint, jlong};
use tokio::task::JoinHandle;

use crate::Result;
use crate::error::ThrowException;

// Operators, derived resources, and in-flight tasks own the runtime. The cache
// must not keep JVM-attached workers alive after those owners are released.
static DEFAULT_RUNTIME: Mutex<Weak<Runtime>> = Mutex::new(Weak::new());

#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _: *mut c_void) -> jint {
    // Register the JavaVM singleton so worker threads can attach to the JVM
    // later via `JavaVM::singleton()`.
    let _ = unsafe { JavaVM::from_raw(vm) };
    opendal::install_default();
    jni::sys::JNI_VERSION_1_8
}

#[derive(Clone)]
pub struct Executor {
    runtime: Arc<Runtime>,
}

struct Runtime {
    inner: Option<tokio::runtime::Runtime>,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // The last owner can be released by a completion callback on a worker.
        // Initiate shutdown without blocking that worker waiting for itself.
        if let Some(runtime) = self.inner.take() {
            runtime.shutdown_background();
        }
    }
}

impl opendal::Execute for Executor {
    fn execute(&self, future: opendal::raw::BoxedStaticFuture<()>) {
        let _handle = self.spawn(future);
    }
}

impl Executor {
    pub fn enter_with<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let runtime = self
            .runtime
            .inner
            .as_ref()
            .expect("runtime must be initialized");
        let _guard = runtime.enter();
        f()
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let owner = self.clone();
        let runtime = self
            .runtime
            .inner
            .as_ref()
            .expect("runtime must be initialized");
        runtime.spawn(async move {
            let result = future.await;
            drop(owner);
            result
        })
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_apache_opendal_AsyncExecutor_makeTokioExecutor<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    cores: jint,
) -> jlong {
    env.with_env(|env| -> Result<jlong> {
        if cores <= 0 {
            env.throw_new(
                jni_str!("java/lang/IllegalArgumentException"),
                jni_str!("cores must be positive"),
            )?;
            return Ok(0);
        }
        let executor = make_tokio_executor(env, cores as usize)?;
        Ok(Box::into_raw(Box::new(executor)) as jlong)
    })
    .resolve::<ThrowException>()
}

/// # Safety
///
/// This function should not be called before the AsyncExecutor is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_AsyncExecutor_disposeInternal<'local>(
    _: EnvUnowned<'local>,
    _: JObject<'local>,
    executor: *mut Executor,
) {
    unsafe {
        drop(Box::from_raw(executor));
    }
}

fn make_tokio_executor(env: &mut Env, cores: usize) -> Result<Executor> {
    // FindClass uses the declaring native method's loader on this Java caller.
    // Retain that loader in the runtime's startup hook so native workers can
    // resolve OpenDAL classes even when the system loader cannot see them.
    let class = env.find_class(jni_str!("org/apache/opendal/AsyncExecutor"))?;
    let class_loader = class.get_class_loader(env)?;
    let class_loader = env.new_global_ref(class_loader)?;
    let counter = AtomicUsize::new(0);
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cores)
        .thread_name_fn(move || {
            let id = counter.fetch_add(1, Ordering::SeqCst);
            format!("opendal-tokio-worker-{id}")
        })
        .on_thread_start(move || {
            // `attach_current_thread` creates a permanent attachment; the thread
            // is detached automatically when it exits.
            let vm = JavaVM::singleton().expect("JavaVM singleton must be initialized");
            vm.attach_current_thread(|env| initialize_current_thread(env, &class_loader))
                .expect("attach current thread must succeed");
        })
        .on_thread_stop(|| {
            // Typically, the thread attached to the JVM will be detached automatically
            // when the thread exits. However, there are some edge cases on Windows that
            // may lead to deadlocks. To mitigate this, we explicitly detach the thread here.
            //
            // See https://github.com/apache/opendal/issues/6869 and
            // https://github.com/jni-rs/jni-rs/issues/701 for more details.
            if let Ok(vm) = JavaVM::singleton() {
                let _ = vm.detach_current_thread();
            }
        })
        .enable_all()
        .build()
        .map_err(|e| {
            opendal::Error::new(
                opendal::ErrorKind::Unexpected,
                "Failed to create tokio runtime.",
            )
            .set_source(e)
        })?;
    Ok(Executor {
        runtime: Arc::new(Runtime {
            inner: Some(executor),
        }),
    })
}

fn initialize_current_thread(env: &mut Env, class_loader: &JClassLoader) -> Result<()> {
    let current_thread = env
        .call_static_method(
            jni_str!("java/lang/Thread"),
            jni_str!("currentThread"),
            jni_sig!("()Ljava/lang/Thread;"),
            &[],
        )?
        .l()?;
    // jni-rs consults the context class loader before falling back to FindClass.
    env.call_method(
        &current_thread,
        jni_str!("setContextClassLoader"),
        jni_sig!("(Ljava/lang/ClassLoader;)V"),
        &[JValue::Object(class_loader)],
    )?;
    let thread_name = match std::thread::current().name() {
        Some(thread_name) => env.new_string(thread_name)?,
        None => unreachable!("thread name must be set"),
    };
    env.call_method(
        &current_thread,
        jni_str!("setName"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(&thread_name)],
    )?;
    Ok(())
}

/// Clone the supplied executor, or share the runtime owned by default operators.
#[inline]
pub(crate) fn executor_or_default(env: &mut Env, executor: *const Executor) -> Result<Executor> {
    if !executor.is_null() {
        // SAFETY: The caller must keep a supplied executor alive until its operators close.
        return Ok(unsafe { &*executor }.clone());
    }

    let mut cached = DEFAULT_RUNTIME
        .lock()
        .expect("default runtime lock must not be poisoned");
    if let Some(runtime) = cached.upgrade() {
        return Ok(Executor { runtime });
    }

    let executor = make_tokio_executor(
        env,
        available_parallelism().map(NonZeroUsize::get).unwrap_or(1),
    )?;
    *cached = Arc::downgrade(&executor.runtime);
    Ok(executor)
}
