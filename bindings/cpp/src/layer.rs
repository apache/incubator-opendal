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

use opendal as od;

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
    fn apply(&self, op: od::Operator) -> od::Operator {
        match self {
            Self::Retry {
                jitter,
                factor,
                min_delay,
                max_delay,
                max_times,
            } => {
                let mut layer = od::layers::RetryLayer::new()
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
                od::layers::TimeoutLayer::new()
                    .with_timeout(*timeout)
                    .with_io_timeout(*io_timeout),
            ),
        }
    }
}

pub struct LayerBuilder {
    layers: Vec<OperatorLayer>,
}

impl Default for LayerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerBuilder {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    pub fn add_timeout(&mut self, timeout_ns: u64, io_timeout_ns: u64) {
        self.layers.push(OperatorLayer::Timeout {
            timeout: Duration::from_nanos(timeout_ns),
            io_timeout: Duration::from_nanos(io_timeout_ns),
        });
    }

    pub fn add_retry(
        &mut self,
        jitter: bool,
        factor: f32,
        min_delay_ns: u64,
        max_delay_ns: u64,
        max_times: u64,
    ) {
        self.layers.push(OperatorLayer::Retry {
            jitter,
            factor,
            min_delay: Duration::from_nanos(min_delay_ns),
            max_delay: Duration::from_nanos(max_delay_ns),
            max_times,
        });
    }

    pub fn apply(&self, mut op: od::Operator) -> od::Operator {
        for layer in &self.layers {
            op = layer.apply(op);
        }
        op
    }
}
