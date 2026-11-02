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

use crate::{datetime_to_timestamp, timestamp_to_datetime};
use object_store::{
    Attribute, AttributeValue, GetOptions, GetRange, ObjectMeta, PutOptions, PutResult,
};
use opendal::raw::*;
use opendal::*;
use std::borrow::Cow;

/// Parse OpStat arguments to object_store GetOptions for head requests
pub fn parse_op_stat(args: &OpStat) -> Result<GetOptions> {
    let mut options = GetOptions {
        head: true, // This is a head request
        ..Default::default()
    };

    if let Some(version) = args.version() {
        options.version = Some(version.to_string());
    }

    if let Some(if_match) = args.if_match() {
        options.if_match = Some(if_match.to_string());
    }

    if let Some(if_none_match) = args.if_none_match() {
        options.if_none_match = Some(if_none_match.to_string());
    }

    if let Some(if_modified_since) = args.if_modified_since() {
        options.if_modified_since = timestamp_to_datetime(if_modified_since);
    }

    if let Some(if_unmodified_since) = args.if_unmodified_since() {
        options.if_unmodified_since = timestamp_to_datetime(if_unmodified_since);
    }

    Ok(options)
}

/// Parse OpRead arguments to object_store GetOptions
pub fn parse_op_read(args: &OpRead, range: BytesRange) -> Result<GetOptions> {
    let mut options = GetOptions::default();

    if let Some(version) = args.version() {
        options.version = Some(version.to_string());
    }

    if let Some(if_match) = args.if_match() {
        options.if_match = Some(if_match.to_string());
    }

    if let Some(if_none_match) = args.if_none_match() {
        options.if_none_match = Some(if_none_match.to_string());
    }

    if let Some(if_modified_since) = args.if_modified_since() {
        options.if_modified_since = timestamp_to_datetime(if_modified_since);
    }

    if let Some(if_unmodified_since) = args.if_unmodified_since() {
        options.if_unmodified_since = timestamp_to_datetime(if_unmodified_since);
    }

    match range {
        BytesRange::Range {
            offset: 0,
            size: None,
        } => {}
        BytesRange::Range { offset, size } => match size {
            Some(size) => {
                let end = offset.checked_add(size).ok_or_else(|| {
                    Error::new(ErrorKind::RangeNotSatisfied, "range exceeds content length")
                        .with_context("offset", offset)
                        .with_context("size", size)
                })?;
                options.range = Some(GetRange::Bounded(offset..end));
            }
            None => {
                options.range = Some(GetRange::Offset(offset));
            }
        },
        BytesRange::Suffix { size } => {
            options.range = Some(GetRange::Suffix(size));
        }
    }

    Ok(options)
}

/// Parse OpWrite arguments to object_store PutOptions
pub fn parse_op_write(args: &OpWrite) -> Result<PutOptions> {
    let mut opts = PutOptions::default();

    if let Some(content_type) = args.content_type() {
        opts.attributes.insert(
            Attribute::ContentType,
            AttributeValue::from(content_type.to_string()),
        );
    }

    if let Some(content_disposition) = args.content_disposition() {
        opts.attributes.insert(
            Attribute::ContentDisposition,
            AttributeValue::from(content_disposition.to_string()),
        );
    }

    if let Some(cache_control) = args.cache_control() {
        opts.attributes.insert(
            Attribute::CacheControl,
            AttributeValue::from(cache_control.to_string()),
        );
    }

    if let Some(user_metadata) = args.user_metadata() {
        for (key, value) in user_metadata {
            opts.attributes.insert(
                Attribute::Metadata(Cow::from(key.to_string())),
                AttributeValue::from(value.to_string()),
            );
        }
    }
    Ok(opts)
}

/// Convert PutOptions to PutMultipartOptions
pub fn format_put_multipart_options(opts: PutOptions) -> object_store::PutMultipartOptions {
    object_store::PutMultipartOptions {
        attributes: opts.attributes,
        ..Default::default()
    }
}

/// Format PutResult to OpenDAL Metadata
pub fn format_put_result(result: PutResult) -> Metadata {
    let mut metadata = MetadataBuilder::unknown();
    if let Some(etag) = &result.e_tag {
        metadata.etag(etag);
    }
    if let Some(version) = &result.version {
        metadata.version(version);
    }
    metadata.build()
}

/// Format `object_store::ObjectMeta` to `opendal::Metadata`.
pub fn format_metadata(meta: &ObjectMeta) -> Metadata {
    let mut metadata = MetadataBuilder::file(meta.size);
    if let Some(last_modified) = datetime_to_timestamp(meta.last_modified) {
        metadata.last_modified(last_modified);
    }
    if let Some(etag) = &meta.e_tag {
        metadata.etag(etag);
    }
    if let Some(version) = &meta.version {
        metadata.version(version);
    }
    metadata.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_op_read_suffix_range() -> Result<()> {
        let opts = parse_op_read(&OpRead::new(), BytesRange::suffix(8))?;

        assert_eq!(opts.range, Some(GetRange::Suffix(8)));
        Ok(())
    }
}
