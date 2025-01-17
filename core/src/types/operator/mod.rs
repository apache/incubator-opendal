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

//! Operator's API will be split into different mods.

#[allow(clippy::module_inception)]
mod operator;
pub use operator::Operator;

mod blocking_operator;
pub use blocking_operator::BlockingOperator;

mod builder;
pub use builder::OperatorBuilder;

mod metadata;
pub use metadata::OperatorInfo;

pub mod operator_functions;
pub mod operator_futures;

// TODO: should we make the registry module public or export the OperatorFactory and OperatorRegistry
// types directly?
mod registry;
// TODO: warning as not used. How can we expose them as public api?
pub use registry::OperatorFactory;
pub use registry::OperatorRegistry;
pub use registry::GLOBAL_OPERATOR_REGISTRY;
