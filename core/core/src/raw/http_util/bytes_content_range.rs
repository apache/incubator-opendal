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

use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Range;
use std::ops::RangeInclusive;
use std::str::FromStr;

use crate::*;

/// BytesContentRange is the content range of bytes.
///
/// `<unit>` should always be `bytes`.
///
/// ```text
/// Content-Range: bytes <range-start>-<range-end>/<size>
/// Content-Range: bytes <range-start>-<range-end>/*
/// Content-Range: bytes */<size>
/// ```
///
/// # Notes
///
/// ## Usage of the default.
///
/// `BytesContentRange::default` is not a valid content range.
/// Please make sure their comes up with `with_range` or `with_size` call.
///
/// ## Allow clippy::len_without_is_empty
///
/// BytesContentRange implements `len()` but not `is_empty()` because it's useless.
/// - When BytesContentRange's range is known, it must be non-empty.
/// - When BytesContentRange's range is no known, we don't know whether it's empty.
#[allow(clippy::len_without_is_empty)]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BytesContentRange(
    /// Start position of the range. `None` means unknown.
    Option<u64>,
    /// End position of the range. `None` means unknown.
    Option<u64>,
    /// Size of the whole content. `None` means unknown.
    Option<u64>,
);

impl BytesContentRange {
    /// Update BytesContentRange with range.
    ///
    /// The range is inclusive: `[start..=end]` as described in `content-range`.
    pub fn with_range(mut self, start: u64, end: u64) -> Self {
        self.0 = Some(start);
        self.1 = Some(end);
        self
    }

    /// Update BytesContentRange with size.
    pub fn with_size(mut self, size: u64) -> Self {
        self.2 = Some(size);
        self
    }

    /// Get the length that specified by this BytesContentRange, return `None` if range is not known.
    pub fn len(&self) -> Option<u64> {
        if let (Some(start), Some(end)) = (self.0, self.1) {
            if end < start {
                Some(0)
            } else {
                Some(end - start + 1)
            }
        } else {
            None
        }
    }

    /// Get the size of this BytesContentRange, return `None` if size is not known.
    pub fn size(&self) -> Option<u64> {
        self.2
    }

    /// Get the range inclusive of this BytesContentRange, return `None` if range is not known.
    pub fn range(&self) -> Option<Range<u64>> {
        if let (Some(start), Some(end)) = (self.0, self.1) {
            Some(start..end + 1)
        } else {
            None
        }
    }

    /// Get the range inclusive of this BytesContentRange, return `None` if range is not known.
    pub fn range_inclusive(&self) -> Option<RangeInclusive<u64>> {
        if let (Some(start), Some(end)) = (self.0, self.1) {
            Some(start..=end)
        } else {
            None
        }
    }

    /// Convert bytes content range into Content-Range header.
    pub fn to_header(&self) -> String {
        format!("bytes {self}")
    }
}

impl Display for BytesContentRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.0, self.1, self.2) {
            (Some(start), Some(end), Some(size)) => write!(f, "{start}-{end}/{size}"),
            (Some(start), Some(end), None) => write!(f, "{start}-{end}/*"),
            (None, None, Some(size)) => write!(f, "*/{size}"),
            _ => unreachable!("invalid bytes range: {:?}", self),
        }
    }
}

impl FromStr for BytesContentRange {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let s = value.strip_prefix("bytes ").ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "header content range is invalid")
                .with_operation("BytesContentRange::from_str")
                .with_context("value", value)
        })?;

        let parse_int_error = |e: std::num::ParseIntError| {
            Error::new(ErrorKind::Unexpected, "header content range is invalid")
                .with_operation("BytesContentRange::from_str")
                .with_context("value", value)
                .set_source(e)
        };

        if let Some(size) = s.strip_prefix("*/") {
            return Ok(
                BytesContentRange::default().with_size(size.parse().map_err(parse_int_error)?)
            );
        }

        let s: Vec<_> = s.split('/').collect();
        if s.len() != 2 {
            return Err(
                Error::new(ErrorKind::Unexpected, "header content range is invalid")
                    .with_operation("BytesContentRange::from_str")
                    .with_context("value", value),
            );
        }

        let v: Vec<_> = s[0].split('-').collect();
        if v.len() != 2 {
            return Err(
                Error::new(ErrorKind::Unexpected, "header content range is invalid")
                    .with_operation("BytesContentRange::from_str")
                    .with_context("value", value),
            );
        }
        let start: u64 = v[0].parse().map_err(parse_int_error)?;
        let end: u64 = v[1].parse().map_err(parse_int_error)?;
        if end < start {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "header content range is invalid: end is less than start",
            )
            .with_operation("BytesContentRange::from_str")
            .with_context("value", value));
        }
        let mut bcr = BytesContentRange::default().with_range(start, end);

        // Handle size part first.
        if s[1] != "*" {
            bcr = bcr.with_size(s[1].parse().map_err(parse_int_error)?)
        };

        Ok(bcr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_content_range_from_str() -> Result<()> {
        let cases = vec![
            (
                "range start with unknown size",
                "bytes 123-123/*",
                BytesContentRange::default().with_range(123, 123),
            ),
            (
                "range start with known size",
                "bytes 123-123/1024",
                BytesContentRange::default()
                    .with_range(123, 123)
                    .with_size(1024),
            ),
            (
                "only have size",
                "bytes */1024",
                BytesContentRange::default().with_size(1024),
            ),
        ];

        for (name, input, expected) in cases {
            let actual = input.parse()?;

            assert_eq!(expected, actual, "{name}")
        }

        Ok(())
    }

    #[test]
    fn test_bytes_content_range_to_string() {
        let h = BytesContentRange::default().with_size(1024);
        assert_eq!(h.to_string(), "*/1024");

        let h = BytesContentRange::default().with_range(0, 1023);
        assert_eq!(h.to_string(), "0-1023/*");

        let h = BytesContentRange::default()
            .with_range(0, 1023)
            .with_size(1024);
        assert_eq!(h.to_string(), "0-1023/1024");
    }

    #[test]
    fn test_bytes_content_range_to_header() {
        let h = BytesContentRange::default().with_size(1024);
        assert_eq!(h.to_header(), "bytes */1024");

        let h = BytesContentRange::default().with_range(0, 1023);
        assert_eq!(h.to_header(), "bytes 0-1023/*");

        let h = BytesContentRange::default()
            .with_range(0, 1023)
            .with_size(1024);
        assert_eq!(h.to_header(), "bytes 0-1023/1024");
    }

    #[test]
    fn test_bytes_content_range_from_str_invalid_end_less_than_start() {
        let cases = vec!["bytes 100-50/*", "bytes 10-9/100", "bytes 1-0/100"];

        for input in cases {
            let result: Result<BytesContentRange> = input.parse();
            assert!(
                result.is_err(),
                "expected error for invalid content range {input}, got {result:?}"
            );
        }
    }

    #[test]
    fn test_bytes_content_range_len_no_underflow() {
        // len() should not underflow when end < start.
        let bcr = BytesContentRange::default().with_range(100, 50);
        assert_eq!(bcr.len(), Some(0));
    }
}
