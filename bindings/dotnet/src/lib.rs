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

//! Rust FFI layer backing the OpenDAL .NET binding.
//!
//! This crate exposes `extern "C"` APIs consumed by C# via P/Invoke and keeps
//! interop memory ownership explicit through dedicated release functions.

mod buffer;
mod capability;
mod chunk;
mod entry;
mod error;
mod executor;
mod metadata;
mod operator;
mod operator_info;
mod options;
mod presign;
mod result;
mod utils;
mod validators;
