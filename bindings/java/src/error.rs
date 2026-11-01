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

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;

use jni::Env;
use jni::errors::ErrorPolicy;
use jni::jni_sig;
use jni::jni_str;
use jni::objects::JThrowable;
use jni::objects::JValue;
use jni::strings::JNIString;
use opendal::ErrorKind;

pub(crate) struct Error {
    inner: opendal::Error,
}

impl Error {
    pub(crate) fn throw(&self, env: &mut Env) {
        if let Err(err) = self.do_throw(env) {
            match err {
                jni::errors::Error::JavaException => {
                    // The exception has been thrown; it will propagate to the JVM.
                }
                _ => env.fatal_error(JNIString::new(err.to_string()).as_ref()),
            }
        }
    }

    pub(crate) fn to_exception<'local>(
        &self,
        env: &mut Env<'local>,
    ) -> jni::errors::Result<JThrowable<'local>> {
        let code = env.new_string(match self.inner.kind() {
            ErrorKind::Unexpected => "Unexpected",
            ErrorKind::Unsupported => "Unsupported",
            ErrorKind::ConfigInvalid => "ConfigInvalid",
            ErrorKind::NotFound => "NotFound",
            ErrorKind::PermissionDenied => "PermissionDenied",
            ErrorKind::IsADirectory => "IsADirectory",
            ErrorKind::NotADirectory => "NotADirectory",
            ErrorKind::AlreadyExists => "AlreadyExists",
            ErrorKind::RateLimited => "RateLimited",
            ErrorKind::IsSameFile => "IsSameFile",
            ErrorKind::ConditionNotMatch => "ConditionNotMatch",
            ErrorKind::RangeNotSatisfied => "RangeNotSatisfied",
            ErrorKind::Conflict => "Conflict",
            _ => "Unexpected",
        })?;
        let message = env.new_string(format!("{:?}", self.inner))?;
        let exception = env.new_object(
            jni_str!("org/apache/opendal/OpenDALException"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&code), JValue::Object(&message)],
        )?;
        // SAFETY: `exception` was just constructed as an `OpenDALException`, a
        // subclass of `java.lang.Throwable`.
        Ok(unsafe { JThrowable::from_raw(env, exception.into_raw()) })
    }

    fn do_throw(&self, env: &mut Env) -> jni::errors::Result<()> {
        let exception = self.to_exception(env)?;
        env.throw(exception)
    }
}

/// A native-method [`ErrorPolicy`] that converts a Rust [`Error`] into an
/// `OpenDALException` and a panic into a `RuntimeException`, returning the
/// default value to the JVM in both cases.
pub(crate) enum ThrowException {}

impl<T: Default> ErrorPolicy<T, Error> for ThrowException {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = ();

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        err: Error,
    ) -> jni::errors::Result<T> {
        if !env.exception_check() {
            err.throw(env);
        }
        Ok(T::default())
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<T> {
        if !env.exception_check() {
            fn downcast_payload(payload: Box<dyn std::any::Any + Send + 'static>) -> String {
                let payload = match payload.downcast::<&'static str>() {
                    Ok(payload) => return payload.to_string(),
                    Err(payload) => payload,
                };

                let payload = match payload.downcast::<String>() {
                    Ok(payload) => return *payload,
                    Err(payload) => payload,
                };

                let _ = payload;
                "native method panicked".to_string()
            }

            let payload = downcast_payload(payload);
            env.fatal_error(JNIString::new(payload).as_ref());
        }
        Ok(T::default())
    }
}

impl From<opendal::Error> for Error {
    fn from(err: opendal::Error) -> Self {
        Self { inner: err }
    }
}

impl From<jni::errors::Error> for Error {
    fn from(err: jni::errors::Error) -> Self {
        opendal::Error::new(ErrorKind::Unexpected, err.to_string()).into()
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}
