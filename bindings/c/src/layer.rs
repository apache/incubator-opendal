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
use std::time::Duration;

use ::opendal as core;

enum OperatorLayer {
    Retry {
        jitter: bool,
        factor: f32,
        min_delay: Duration,
        max_delay: Duration,
        max_times: u64,
    },
    Timeout {
        timeout: Duration,
        io_timeout: Duration,
    },
}

impl OperatorLayer {
    fn apply(&self, op: core::Operator) -> core::Operator {
        match self {
            Self::Retry {
                jitter,
                factor,
                min_delay,
                max_delay,
                max_times,
            } => {
                let mut layer = core::layers::RetryLayer::new()
                    .with_factor(*factor)
                    .with_min_delay(*min_delay)
                    .with_max_delay(*max_delay)
                    .with_max_times(*max_times as usize);
                if *jitter {
                    layer = layer.with_jitter();
                }
                op.layer(layer)
            }
            Self::Timeout {
                timeout,
                io_timeout,
            } => op.layer(
                core::layers::TimeoutLayer::new()
                    .with_timeout(*timeout)
                    .with_io_timeout(*io_timeout),
            ),
        }
    }
}

/// \brief The layers to apply when initializing an opendal_operator.
///
/// \note This is also a heap-allocated struct, please free it after you use it.
#[repr(C)]
pub struct opendal_operator_layers {
    /// The pointer to the Vec<OperatorLayer> in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

impl opendal_operator_layers {
    pub(crate) fn apply(&self, mut op: core::Operator) -> core::Operator {
        for layer in self.deref() {
            op = layer.apply(op);
        }
        op
    }

    fn deref(&self) -> &[OperatorLayer] {
        // Safety: the inner should never be null once constructed.
        unsafe { &*(self.inner as *mut Vec<OperatorLayer>) }
    }

    fn deref_mut(&mut self) -> &mut Vec<OperatorLayer> {
        // Safety: the inner should never be null once constructed.
        unsafe { &mut *(self.inner as *mut Vec<OperatorLayer>) }
    }

    /// \brief Construct a heap-allocated opendal_operator_layers.
    #[no_mangle]
    pub extern "C" fn opendal_operator_layers_new() -> *mut Self {
        let layers: Vec<OperatorLayer> = Vec::new();
        let layers = Self {
            inner: Box::into_raw(Box::new(layers)) as _,
        };
        Box::into_raw(Box::new(layers))
    }

    /// \brief Add a retry layer.
    pub fn add_retry(
        &mut self,
        jitter: bool,
        factor: f32,
        min_delay_ns: u64,
        max_delay_ns: u64,
        max_times: u64,
    ) {
        self.deref_mut().push(OperatorLayer::Retry {
            jitter,
            factor,
            min_delay: Duration::from_nanos(min_delay_ns),
            max_delay: Duration::from_nanos(max_delay_ns),
            max_times,
        });
    }

    /// \brief Add a retry layer.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_layers_add_retry(
        &mut self,
        jitter: bool,
        factor: f32,
        min_delay_ns: u64,
        max_delay_ns: u64,
        max_times: u64,
    ) {
        self.add_retry(jitter, factor, min_delay_ns, max_delay_ns, max_times);
    }

    /// \brief Add a timeout layer.
    pub fn add_timeout(&mut self, timeout_ns: u64, io_timeout_ns: u64) {
        self.deref_mut().push(OperatorLayer::Timeout {
            timeout: Duration::from_nanos(timeout_ns),
            io_timeout: Duration::from_nanos(io_timeout_ns),
        });
    }

    /// \brief Add a timeout layer.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_layers_add_timeout(
        &mut self,
        timeout_ns: u64,
        io_timeout_ns: u64,
    ) {
        self.add_timeout(timeout_ns, io_timeout_ns);
    }

    /// \brief Free the allocated memory used by opendal_operator_layers.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_layers_free(ptr: *mut opendal_operator_layers) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut Vec<OperatorLayer>));
                drop(Box::from_raw(ptr));
            }
        }
    }
}
