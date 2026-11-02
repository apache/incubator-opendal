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

use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use opendal::raw::Timestamp;

use crate::error::OpenDALError;
use crate::validators::prelude::{
    validate_list_limit, validate_read_chunk, validate_read_concurrent, validate_read_gap,
    validate_read_range_end, validate_write_chunk, validate_write_concurrent,
};

fn parse_bool(values: &HashMap<String, String>, key: &str) -> Result<Option<bool>, OpenDALError> {
    let Some(raw) = values.get(key) else {
        return Ok(None);
    };

    match raw.to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(crate::utils::config_invalid_error(format!(
            "invalid boolean value for {key}: {raw}"
        ))),
    }
}

fn parse_number<T>(values: &HashMap<String, String>, key: &str) -> Result<Option<T>, OpenDALError>
where
    T: FromStr,
    T::Err: Display,
{
    let Some(raw) = values.get(key) else {
        return Ok(None);
    };

    raw.parse::<T>().map(Some).map_err(|err| {
        crate::utils::config_invalid_error(format!(
            "invalid {} value for {key}: {raw}, {err}",
            std::any::type_name::<T>()
        ))
    })
}

fn parse_timestamp(
    values: &HashMap<String, String>,
    key: &str,
) -> Result<Option<Timestamp>, OpenDALError> {
    let Some(raw) = values.get(key) else {
        return Ok(None);
    };

    let millis = raw.parse::<i64>().map_err(|err| {
        crate::utils::config_invalid_error(format!(
            "invalid timestamp milliseconds for {key}: {raw}, {err}"
        ))
    })?;

    Timestamp::from_millisecond(millis)
        .map(Some)
        .map_err(OpenDALError::from_opendal_error)
}

fn parse_string(values: &HashMap<String, String>, key: &str) -> Option<String> {
    values.get(key).cloned()
}

pub fn parse_read_options(
    values: &HashMap<String, String>,
) -> Result<opendal::options::ReadOptions, OpenDALError> {
    let mut options = opendal::options::ReadOptions::default();

    let offset = parse_number::<u64>(values, "offset")?.unwrap_or_default();
    let length = parse_number::<u64>(values, "length")?;
    if offset > 0 || length.is_some() {
        options.range = match validate_read_range_end(offset, length)? {
            Some(end) => (offset..end).into(),
            None => (offset..).into(),
        };
    }

    options.version = parse_string(values, "version");
    options.if_match = parse_string(values, "if_match");
    options.if_none_match = parse_string(values, "if_none_match");
    options.if_modified_since = parse_timestamp(values, "if_modified_since")?;
    options.if_unmodified_since = parse_timestamp(values, "if_unmodified_since")?;

    if let Some(concurrent) = parse_number::<usize>(values, "concurrent")? {
        validate_read_concurrent(concurrent)?;
        options.concurrent = concurrent;
    }

    if let Some(chunk) = parse_number::<usize>(values, "chunk")? {
        validate_read_chunk(chunk)?;
        options.chunk = Some(chunk);
    }

    if let Some(gap) = parse_number::<usize>(values, "gap")? {
        validate_read_gap(gap)?;
        options.gap = Some(gap);
    }

    options.override_content_type = parse_string(values, "override_content_type");
    options.override_cache_control = parse_string(values, "override_cache_control");
    options.override_content_disposition = parse_string(values, "override_content_disposition");

    Ok(options)
}

pub fn parse_write_options(
    values: &HashMap<String, String>,
) -> Result<opendal::options::WriteOptions, OpenDALError> {
    let mut options = opendal::options::WriteOptions::default();

    if let Some(append) = parse_bool(values, "append")? {
        options.append = append;
    }

    options.cache_control = parse_string(values, "cache_control");
    options.content_type = parse_string(values, "content_type");
    options.content_disposition = parse_string(values, "content_disposition");
    options.content_encoding = parse_string(values, "content_encoding");
    options.if_match = parse_string(values, "if_match");
    options.if_none_match = parse_string(values, "if_none_match");

    if let Some(if_not_exists) = parse_bool(values, "if_not_exists")? {
        options.if_not_exists = if_not_exists;
    }

    if let Some(concurrent) = parse_number::<usize>(values, "concurrent")? {
        validate_write_concurrent(concurrent)?;
        options.concurrent = concurrent;
    }

    if let Some(chunk) = parse_number::<usize>(values, "chunk")? {
        validate_write_chunk(chunk)?;
        options.chunk = Some(chunk);
    }

    let user_metadata = values
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("user_metadata.")
                .map(|k| (k.to_string(), value.to_string()))
        })
        .collect::<HashMap<_, _>>();

    if !user_metadata.is_empty() {
        options.user_metadata = Some(user_metadata);
    }

    Ok(options)
}

pub fn parse_stat_options(
    values: &HashMap<String, String>,
) -> Result<opendal::options::StatOptions, OpenDALError> {
    Ok(opendal::options::StatOptions {
        version: parse_string(values, "version"),
        if_match: parse_string(values, "if_match"),
        if_none_match: parse_string(values, "if_none_match"),
        if_modified_since: parse_timestamp(values, "if_modified_since")?,
        if_unmodified_since: parse_timestamp(values, "if_unmodified_since")?,
        override_content_type: parse_string(values, "override_content_type"),
        override_cache_control: parse_string(values, "override_cache_control"),
        override_content_disposition: parse_string(values, "override_content_disposition"),
        ..Default::default()
    })
}

pub fn parse_list_options(
    values: &HashMap<String, String>,
) -> Result<opendal::options::ListOptions, OpenDALError> {
    let mut options = opendal::options::ListOptions::default();

    if let Some(recursive) = parse_bool(values, "recursive")? {
        options.recursive = recursive;
    }

    if let Some(limit) = parse_number::<usize>(values, "limit")? {
        validate_list_limit(limit)?;
        options.limit = Some(limit);
    }

    options.start_after = parse_string(values, "start_after");

    if let Some(versions) = parse_bool(values, "versions")? {
        options.versions = versions;
    }

    if let Some(deleted) = parse_bool(values, "deleted")? {
        options.deleted = deleted;
    }

    Ok(options)
}

pub fn parse_delete_options(
    values: &HashMap<String, String>,
) -> Result<opendal::options::DeleteOptions, OpenDALError> {
    Ok(opendal::options::DeleteOptions {
        version: parse_string(values, "version"),
        recursive: parse_bool(values, "recursive")?.unwrap_or(false),
        if_match: parse_string(values, "if_match"),
        ..Default::default()
    })
}
