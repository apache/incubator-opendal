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

//! Validators for operation option payloads.

use crate::error::OpenDALError;
use crate::validators::validate_checked_add_u64;

pub fn validate_read_range_end(
    offset: u64,
    length: Option<u64>,
) -> Result<Option<u64>, OpenDALError> {
    let Some(size) = length else {
        return Ok(None);
    };

    let end = validate_checked_add_u64(offset, size, "offset + length in read options")?;

    Ok(Some(end))
}
