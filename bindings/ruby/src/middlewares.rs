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

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use magnus::Error;
use magnus::RModule;
use magnus::Ruby;
use magnus::method;
use magnus::prelude::*;

use opendal::raw::Layer as OCoreLayer;

use crate::operator::Operator;
use crate::*;

// Applies an OpenDAL layer (treated as middleware in Ruby).
//
// Magnus provides `TypedData` and the `magnus::wrap` macro for strong typing between Ruby and Rust,
// but we avoid exposing Rust types directly in Ruby to allow packaging layers in separate gems.
//
// Instead, we rely on duck typing: each Ruby-defined layer must implement `#apply_to(operator)`.
// This internal function helps apply a Rust layer by locking the inner Layer and wrapping the result
// back into an `Operator`.
fn apply_layer<T>(
    ruby: &Ruby,
    inner: &Arc<Mutex<T>>,
    operator: &Operator,
    name: &'static str,
) -> Result<Operator, Error>
where
    T: OCoreLayer + Clone + Send + Sync + 'static,
{
    let guard = inner.lock().map_err(|_| {
        Error::new(
            ruby.exception_runtime_error(),
            format!("poisoned {name} mutex"),
        )
    })?;

    let layered = operator.async_op.clone().layer(guard.clone());

    Ok(Operator::from_operator(layered))
}

/// @yard
/// Adds retry for temporary failed operations.
///
/// See [`opendal::layers::RetryLayer`] for more information.
#[magnus::wrap(class = "OpenDal::Middleware::Retry")]
struct RetryMiddleware(Arc<Mutex<ocore::layers::RetryLayer>>);

impl RetryMiddleware {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(ocore::layers::RetryLayer::default())))
    }

    fn apply_to(ruby: &Ruby, rb_self: &Self, operator: &Operator) -> Result<Operator, Error> {
        apply_layer(ruby, &rb_self.0, operator, "OpenDal::Middleware::Retry")
    }
}

/// @yard
/// Adds concurrent request limit.
///
/// See [`opendal::layers::ConcurrentLimitLayer`] for more information.
#[magnus::wrap(class = "OpenDal::Middleware::ConcurrentLimit")]
struct ConcurrentLimitMiddleware(Arc<Mutex<ocore::layers::ConcurrentLimitLayer>>);

impl ConcurrentLimitMiddleware {
    fn new(permits: usize) -> Self {
        Self(Arc::new(Mutex::new(
            ocore::layers::ConcurrentLimitLayer::new(permits),
        )))
    }

    fn apply_to(ruby: &Ruby, rb_self: &Self, operator: &Operator) -> Result<Operator, Error> {
        apply_layer(
            ruby,
            &rb_self.0,
            operator,
            "OpenDal::Middleware::ConcurrentLimit",
        )
    }
}

/// @yard
/// Adds a bandwidth rate limiter to the underlying services.
///
/// See [`opendal::layers::ThrottleLayer`] for more information.
#[magnus::wrap(class = "OpenDal::Middleware::Throttle")]
struct ThrottleMiddleware(Arc<Mutex<ocore::layers::ThrottleLayer>>);

impl ThrottleMiddleware {
    fn new(bandwidth: u32, burst: u32) -> Self {
        Self(Arc::new(Mutex::new(ocore::layers::ThrottleLayer::new(
            bandwidth, burst,
        ))))
    }

    fn apply_to(ruby: &Ruby, rb_self: &Self, operator: &Operator) -> Result<Operator, Error> {
        apply_layer(ruby, &rb_self.0, operator, "OpenDal::Middleware::Throttle")
    }
}

fn parse_duration(ruby: &Ruby, val: f64) -> Result<Duration, Error> {
    Duration::try_from_secs_f64(val).map_err(|e| {
        Error::new(
            ruby.exception_arg_error(),
            format!("invalid float duration: {e}"),
        )
    })
}

/// @yard
/// Adds timeout for every operation to avoid slow or unexpected hang operations.
///
/// See [`opendal::layers::TimeoutLayer`] for more information.
#[magnus::wrap(class = "OpenDal::Middleware::Timeout")]
struct TimeoutMiddleware(Arc<Mutex<ocore::layers::TimeoutLayer>>);

impl TimeoutMiddleware {
    fn new(ruby: &Ruby, timeout: f64, io_timeout: f64) -> Result<Self, Error> {
        let layer = ocore::layers::TimeoutLayer::new()
            .with_timeout(parse_duration(ruby, timeout)?)
            .with_io_timeout(parse_duration(ruby, io_timeout)?);
        Ok(Self(Arc::new(Mutex::new(layer))))
    }

    fn apply_to(ruby: &Ruby, rb_self: &Self, operator: &Operator) -> Result<Operator, Error> {
        apply_layer(ruby, &rb_self.0, operator, "OpenDal::Middleware::Timeout")
    }
}

pub fn include(ruby: &Ruby, middleware_module: &RModule) -> Result<(), Error> {
    middleware_module.define_class("Retry", ruby.class_object())?;

    let retry = middleware_module.define_class("Retry", ruby.class_object())?;
    retry.define_singleton_method("new", function!(RetryMiddleware::new, 0))?;
    retry.define_method("apply_to", method!(RetryMiddleware::apply_to, 1))?;

    let concurrent_limit =
        middleware_module.define_class("ConcurrentLimit", ruby.class_object())?;
    concurrent_limit
        .define_singleton_method("new", function!(ConcurrentLimitMiddleware::new, 1))?;
    concurrent_limit.define_method("apply_to", method!(ConcurrentLimitMiddleware::apply_to, 1))?;

    let throttle_middleware = middleware_module.define_class("Throttle", ruby.class_object())?;
    throttle_middleware.define_singleton_method("new", function!(ThrottleMiddleware::new, 2))?;
    throttle_middleware.define_method("apply_to", method!(ThrottleMiddleware::apply_to, 1))?;

    let timeout_middleware = middleware_module.define_class("Timeout", ruby.class_object())?;
    timeout_middleware.define_singleton_method("new", function!(TimeoutMiddleware::new, 2))?;
    timeout_middleware.define_method("apply_to", method!(TimeoutMiddleware::apply_to, 1))?;

    Ok(())
}
