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

use std::panic::{AssertUnwindSafe, catch_unwind};

use opendal::Operator;
use opendal::services::Memory;

const SMOKE_OK: i32 = 0;
const SMOKE_ERROR: i32 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn opendal_moonbit_smoke() -> i32 {
    match catch_unwind(AssertUnwindSafe(|| Operator::new(Memory::default()))) {
        Ok(Ok(_)) => SMOKE_OK,
        Ok(Err(_)) | Err(_) => SMOKE_ERROR,
    }
}
