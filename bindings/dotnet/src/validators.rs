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

pub mod layer;
pub mod options;

use crate::error::OpenDALError;
use crate::utils::config_invalid_error;

pub(crate) fn validate_positive_integer<T: PartialOrd + From<u8>>(
    value: T,
    field: &str,
) -> Result<(), OpenDALError> {
    if value <= T::from(0) {
        return Err(config_invalid_error(format!(
            "{field} must be greater than zero"
        )));
    }

    Ok(())
}

pub(crate) fn validate_positive_finite_f32(value: f32, field: &str) -> Result<(), OpenDALError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(config_invalid_error(format!(
            "{field} must be a positive finite number"
        )));
    }

    Ok(())
}

pub(crate) fn validate_checked_add_u64(
    left: u64,
    right: u64,
    context: &str,
) -> Result<u64, OpenDALError> {
    left.checked_add(right)
        .ok_or_else(|| config_invalid_error(format!("{context} overflow")))
}

pub(crate) mod prelude {
    pub(crate) use super::layer::{
        validate_concurrent_limit_options, validate_retry_options, validate_throttle_options,
        validate_timeout_options,
    };
    pub(crate) use super::options::{
        validate_list_limit, validate_read_chunk, validate_read_concurrent, validate_read_gap,
        validate_read_range_end, validate_write_chunk, validate_write_concurrent,
    };
}
