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

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Bound;
use std::ops::Range;
use std::ops::RangeBounds;
use std::str::FromStr;

use crate::*;

/// BytesRange carries a range of content.
///
/// `Display` formats the range without the `bytes=` prefix.
/// [`BytesRange::to_header`] builds the HTTP `Range` header value.
///
/// Bounded ranges with a zero size or an overflowing end render as an empty
/// string. [`BytesRange::to_header`] also returns an empty string for them.
///
/// `<unit>` should always be `bytes`.
///
/// ```text
/// Range: bytes=<range-start>-
/// Range: bytes=<range-start>-<range-end>
/// Range: bytes=-<suffix-length>
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytesRange {
    /// Read from `offset` with optional `size`.
    Range {
        /// Offset of the range.
        offset: u64,
        /// Size of the range.
        size: Option<u64>,
    },
    /// Read the last `size` bytes.
    Suffix {
        /// Size of the suffix range.
        size: u64,
    },
}

impl Default for BytesRange {
    fn default() -> Self {
        BytesRange::new(0, None)
    }
}

impl BytesRange {
    /// Create a new `BytesRange`
    ///
    /// It better to use `BytesRange::from(1024..2048)` to construct.
    ///
    /// # Note
    ///
    /// The behavior for `None` and `Some` of `size` is different.
    ///
    /// - size=None => `bytes=<offset>-`, read from `<offset>` until the end
    /// - size=Some(1024) => `bytes=<offset>-<offset + 1024>`, read 1024 bytes starting from the `<offset>`
    pub fn new(offset: u64, size: Option<u64>) -> Self {
        BytesRange::Range { offset, size }
    }

    /// Create a suffix `BytesRange` that reads the last `size` bytes.
    pub fn suffix(size: u64) -> Self {
        BytesRange::Suffix { size }
    }

    /// Get offset of BytesRange.
    pub fn offset(&self) -> u64 {
        match self {
            BytesRange::Range { offset, .. } => *offset,
            BytesRange::Suffix { .. } => 0,
        }
    }

    /// Get size of BytesRange.
    pub fn size(&self) -> Option<u64> {
        match self {
            BytesRange::Range { size, .. } => *size,
            BytesRange::Suffix { size } => Some(*size),
        }
    }

    /// Returns true if this range is a suffix range.
    pub fn is_suffix(&self) -> bool {
        matches!(self, BytesRange::Suffix { .. })
    }

    /// Advance the range by `n` bytes.
    ///
    /// # Panics
    ///
    /// Panic if advancing the offset would overflow or if input `n` is larger
    /// than the size of the range.
    pub fn advance(&mut self, n: u64) {
        match self {
            BytesRange::Range { offset, size } => {
                *offset = offset
                    .checked_add(n)
                    .expect("BytesRange::advance overflow: offset + n exceeds u64::MAX");
                *size = size.map(|size| {
                    size.checked_sub(n)
                        .expect("BytesRange::advance underflow: n exceeds range size")
                });
            }
            BytesRange::Suffix { size } => {
                *size = size
                    .checked_sub(n)
                    .expect("BytesRange::advance underflow: n exceeds suffix size");
            }
        }
    }

    /// Check if this range is full of this content.
    ///
    /// If this range is full, we don't need to specify it in http request.
    pub fn is_full(&self) -> bool {
        matches!(
            self,
            BytesRange::Range {
                offset: 0,
                size: None
            }
        )
    }

    /// Convert bytes range into Range header.
    ///
    /// Returns an empty string for a range with no valid HTTP representation --
    /// a zero size, or an end that overflows -- which [`Display`] renders as
    /// an empty string.
    pub fn to_header(&self) -> String {
        let range = self.to_string();
        if range.is_empty() {
            String::new()
        } else {
            format!("bytes={range}")
        }
    }

    /// Convert bytes range into rust range.
    pub fn to_range(&self) -> impl RangeBounds<u64> {
        let (offset, size) = match self {
            BytesRange::Range { offset, size } => (*offset, *size),
            BytesRange::Suffix { .. } => {
                panic!("BytesRange::to_range is not supported for suffix ranges")
            }
        };

        (
            Bound::Included(offset),
            match size {
                Some(size) => Bound::Excluded(
                    offset
                        .checked_add(size)
                        .expect("BytesRange::to_range overflow: offset + size exceeds u64::MAX"),
                ),
                None => Bound::Unbounded,
            },
        )
    }

    /// Convert bytes range into rust range with usize.
    pub fn to_range_as_usize(self) -> impl RangeBounds<usize> {
        let (range_offset, range_size) = match self {
            BytesRange::Range { offset, size } => (offset, size),
            BytesRange::Suffix { .. } => {
                panic!("BytesRange::to_range_as_usize is not supported for suffix ranges")
            }
        };

        let offset: usize = range_offset
            .try_into()
            .expect("BytesRange::to_range_as_usize: offset exceeds usize::MAX");
        (
            Bound::Included(offset),
            match range_size {
                Some(size) => {
                    let end: usize = range_offset
                        .checked_add(size)
                        .expect(
                            "BytesRange::to_range_as_usize overflow: offset + size exceeds u64::MAX",
                        )
                        .try_into()
                        .expect(
                            "BytesRange::to_range_as_usize: offset + size exceeds usize::MAX",
                        );
                    Bound::Excluded(end)
                }
                None => Bound::Unbounded,
            },
        )
    }

    /// Convert bytes range into a checked content slice range.
    pub fn to_content_range(self, content_length: usize) -> Result<Range<usize>> {
        let (range_offset, range_size) = match self {
            BytesRange::Range { offset, size } => (offset, size),
            BytesRange::Suffix { size } => {
                let content_length = content_length as u64;
                let start = content_length.saturating_sub(size);
                let start: usize = start.try_into().map_err(|_| {
                    Error::new(ErrorKind::RangeNotSatisfied, "range exceeds content length")
                        .with_operation("BytesRange::to_content_range")
                        .with_context("size", size)
                        .with_context("content_length", content_length)
                })?;
                let end: usize = content_length.try_into().map_err(|_| {
                    Error::new(ErrorKind::RangeNotSatisfied, "range exceeds content length")
                        .with_operation("BytesRange::to_content_range")
                        .with_context("size", size)
                        .with_context("content_length", content_length)
                })?;
                return Ok(start..end);
            }
        };

        if range_size == Some(0) {
            return Ok(0..0);
        }

        if range_offset >= content_length as u64 && range_size.is_none() {
            return Ok(content_length..content_length);
        }

        let offset: usize = range_offset.try_into().map_err(|_| {
            Error::new(ErrorKind::RangeNotSatisfied, "range exceeds content length")
                .with_operation("BytesRange::to_content_range")
                .with_context("offset", range_offset)
                .with_context("content_length", content_length)
        })?;

        match range_size {
            Some(size) => {
                let end = range_offset
                    .checked_add(size)
                    .ok_or_else(|| {
                        Error::new(ErrorKind::RangeNotSatisfied, "range exceeds content length")
                            .with_operation("BytesRange::to_content_range")
                            .with_context("offset", range_offset)
                            .with_context("size", size)
                            .with_context("content_length", content_length)
                    })?
                    .try_into()
                    .map_err(|_| {
                        Error::new(ErrorKind::RangeNotSatisfied, "range exceeds content length")
                            .with_operation("BytesRange::to_content_range")
                            .with_context("offset", range_offset)
                            .with_context("size", size)
                            .with_context("content_length", content_length)
                    })?;

                if end > content_length {
                    return Err(Error::new(
                        ErrorKind::RangeNotSatisfied,
                        "range exceeds content length",
                    )
                    .with_operation("BytesRange::to_content_range")
                    .with_context("offset", range_offset)
                    .with_context("size", size)
                    .with_context("content_length", content_length));
                }

                Ok(offset..end)
            }
            None => Ok(offset..content_length),
        }
    }
}

impl Display for BytesRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            BytesRange::Range { offset, size: None } => write!(f, "{offset}-"),
            // A zero-size bounded range has no valid HTTP Range representation, and
            // neither does a range whose end overflows u64. Display must stay
            // total -- returning Err makes `format!` panic -- so these render
            // as an empty string, which `to_header` maps to an empty header.
            BytesRange::Range { size: Some(0), .. } => Ok(()),
            BytesRange::Range {
                offset,
                size: Some(size),
            } => match offset.checked_add(size - 1) {
                Some(end) => write!(f, "{offset}-{end}"),
                None => Ok(()),
            },
            BytesRange::Suffix { size } => write!(f, "-{size}"),
        }
    }
}

impl FromStr for BytesRange {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let s = value.strip_prefix("bytes=").ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "header range is invalid")
                .with_operation("BytesRange::from_str")
                .with_context("value", value)
        })?;

        if s.contains(',') {
            return Err(Error::new(ErrorKind::Unexpected, "header range is invalid")
                .with_operation("BytesRange::from_str")
                .with_context("value", value));
        }

        let v = s.split('-').collect::<Vec<_>>();
        if v.len() != 2 {
            return Err(Error::new(ErrorKind::Unexpected, "header range is invalid")
                .with_operation("BytesRange::from_str")
                .with_context("value", value));
        }

        let parse_int_error = |e: std::num::ParseIntError| {
            Error::new(ErrorKind::Unexpected, "header range is invalid")
                .with_operation("BytesRange::from_str")
                .with_context("value", value)
                .set_source(e)
        };

        if v[1].is_empty() {
            // <range-start>-
            Ok(BytesRange::new(
                v[0].parse().map_err(parse_int_error)?,
                None,
            ))
        } else if v[0].is_empty() {
            // -<suffix-length>
            Ok(BytesRange::suffix(v[1].parse().map_err(parse_int_error)?))
        } else {
            // <range-start>-<range-end>
            let start: u64 = v[0].parse().map_err(parse_int_error)?;
            let end: u64 = v[1].parse().map_err(parse_int_error)?;
            if end < start {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "header range is invalid: end is less than start",
                )
                .with_operation("BytesRange::from_str")
                .with_context("value", value));
            }
            Ok(BytesRange::new(start, Some(end - start + 1)))
        }
    }
}

impl<T> From<T> for BytesRange
where
    T: RangeBounds<u64>,
{
    fn from(range: T) -> Self {
        let offset = match range.start_bound().cloned() {
            Bound::Included(n) => n,
            Bound::Excluded(n) => n.saturating_add(1),
            Bound::Unbounded => 0,
        };
        let size = match range.end_bound().cloned() {
            Bound::Included(n) => Some(n.saturating_add(1).saturating_sub(offset)),
            Bound::Excluded(n) => Some(n.saturating_sub(offset)),
            Bound::Unbounded => None,
        };

        BytesRange::new(offset, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_range_display_zero_size() {
        // Zero-size ranges have no valid HTTP representation, so Display
        // renders them as an empty string instead of failing: a Display
        // failure makes `format!` panic.
        assert_eq!(BytesRange::new(0, Some(0)).to_string(), "");
        assert_eq!(BytesRange::new(5, Some(0)).to_string(), "");
    }

    #[test]
    fn test_bytes_range_to_string() {
        let h = BytesRange::new(0, Some(1024));
        assert_eq!(h.to_string(), "0-1023");

        let h = BytesRange::new(1024, None);
        assert_eq!(h.to_string(), "1024-");

        let h = BytesRange::new(1024, Some(1024));
        assert_eq!(h.to_string(), "1024-2047");

        let h = BytesRange::suffix(1024);
        assert_eq!(h.to_string(), "-1024");
    }

    #[test]
    fn test_bytes_range_to_header_is_empty_when_unrepresentable() {
        // `Display` renders these as an empty string, and `to_header` maps
        // that to an empty header.
        assert_eq!(BytesRange::new(0, Some(0)).to_header(), "");
        assert_eq!(BytesRange::new(5, Some(0)).to_header(), "");
        assert_eq!(BytesRange::new(u64::MAX, Some(2)).to_header(), "");
    }

    #[test]
    fn test_bytes_range_to_header() {
        let h = BytesRange::new(0, Some(1024));
        assert_eq!(h.to_header(), "bytes=0-1023");

        let h = BytesRange::new(1024, None);
        assert_eq!(h.to_header(), "bytes=1024-");

        let h = BytesRange::new(1024, Some(1024));
        assert_eq!(h.to_header(), "bytes=1024-2047");

        let h = BytesRange::suffix(1024);
        assert_eq!(h.to_header(), "bytes=-1024");
    }

    #[test]
    fn test_bytes_range_from_range_bounds() {
        assert_eq!(BytesRange::new(0, None), BytesRange::from(..));
        assert_eq!(BytesRange::new(10, None), BytesRange::from(10..));
        assert_eq!(BytesRange::new(0, Some(11)), BytesRange::from(..=10));
        assert_eq!(BytesRange::new(0, Some(10)), BytesRange::from(..10));
        assert_eq!(BytesRange::new(10, Some(10)), BytesRange::from(10..20));
        assert_eq!(BytesRange::new(10, Some(11)), BytesRange::from(10..=20));
    }

    #[test]
    fn test_bytes_range_from_str() -> Result<()> {
        let cases = vec![
            ("range-start", "bytes=123-", BytesRange::new(123, None)),
            ("range", "bytes=123-124", BytesRange::new(123, Some(2))),
            ("one byte", "bytes=0-0", BytesRange::new(0, Some(1))),
            ("suffix", "bytes=-123", BytesRange::suffix(123)),
            (
                "lower case header",
                "bytes=0-0",
                BytesRange::new(0, Some(1)),
            ),
        ];

        for (name, input, expected) in cases {
            let actual = input.parse()?;

            assert_eq!(expected, actual, "{name}")
        }

        Ok(())
    }

    #[test]
    fn test_bytes_range_from_str_invalid_end_less_than_start() {
        let cases = vec!["bytes=100-50", "bytes=10-9", "bytes=1-0"];

        for input in cases {
            let result: Result<BytesRange> = input.parse();
            assert!(
                result.is_err(),
                "expected error for invalid range {input}, got {result:?}"
            );
        }
    }

    #[allow(clippy::reversed_empty_ranges)]
    #[test]
    fn test_bytes_range_from_range_bounds_underflow() {
        // Invalid ranges where end < start should produce zero-size ranges
        // rather than underflowing.
        assert_eq!(BytesRange::new(100, Some(0)), BytesRange::from(100..50));
        assert_eq!(BytesRange::new(10, Some(0)), BytesRange::from(10..=5));
        assert_eq!(BytesRange::new(5, Some(0)), BytesRange::from(5..0));
        assert_eq!(BytesRange::new(5, Some(0)), BytesRange::from(5..=0));
    }

    #[test]
    fn test_bytes_range_from_range_bounds_u64_max() {
        // Boundary cases near u64::MAX must not overflow.
        assert_eq!(
            BytesRange::new(0, Some(u64::MAX)),
            BytesRange::from(..=u64::MAX)
        );
        assert_eq!(
            BytesRange::new(0, Some(u64::MAX)),
            BytesRange::from(..u64::MAX)
        );
        assert_eq!(
            BytesRange::new(1, Some(u64::MAX.saturating_sub(1))),
            BytesRange::from(1..=u64::MAX)
        );
        // Excluded start at u64::MAX must not overflow.
        assert_eq!(
            BytesRange::new(u64::MAX, None),
            BytesRange::from((u64::MAX)..)
        );
    }

    #[test]
    fn test_bytes_range_display_overflow() {
        // offset=u64::MAX, size=2 would overflow in Display (u64::MAX + 1),
        // so it renders as an empty string instead of failing.
        assert_eq!(BytesRange::new(u64::MAX, Some(2)).to_string(), "");
    }

    #[test]
    #[should_panic(expected = "BytesRange::to_range overflow")]
    fn test_bytes_range_to_range_overflow() {
        let range = BytesRange::new(u64::MAX, Some(1));
        let _ = range.to_range();
    }

    #[test]
    #[should_panic(expected = "BytesRange::to_range_as_usize")]
    fn test_bytes_range_to_range_as_usize_overflow() {
        let range = BytesRange::new(u64::MAX, Some(1));
        let _ = range.to_range_as_usize();
    }

    #[test]
    fn test_bytes_range_to_content_range() -> Result<()> {
        assert_eq!(BytesRange::new(2, Some(4)).to_content_range(10)?, 2..6);
        assert_eq!(BytesRange::new(2, None).to_content_range(10)?, 2..10);
        assert_eq!(BytesRange::suffix(4).to_content_range(10)?, 6..10);
        assert_eq!(BytesRange::suffix(10).to_content_range(10)?, 0..10);
        assert_eq!(BytesRange::suffix(20).to_content_range(10)?, 0..10);
        assert_eq!(BytesRange::suffix(0).to_content_range(10)?, 10..10);
        assert_eq!(BytesRange::suffix(4).to_content_range(0)?, 0..0);
        assert_eq!(BytesRange::new(10, None).to_content_range(10)?, 10..10);
        assert_eq!(BytesRange::new(20, None).to_content_range(10)?, 10..10);
        assert_eq!(
            BytesRange::new(u64::MAX, None).to_content_range(10)?,
            10..10
        );
        assert_eq!(
            BytesRange::new(u64::MAX, Some(0)).to_content_range(10)?,
            0..0
        );

        let err = BytesRange::new(8, Some(4))
            .to_content_range(10)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::RangeNotSatisfied);

        let err = BytesRange::new(u64::MAX, Some(1))
            .to_content_range(10)
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::RangeNotSatisfied);

        Ok(())
    }

    #[test]
    #[should_panic(expected = "BytesRange::advance overflow")]
    fn test_bytes_range_advance_offset_overflow() {
        let mut range = BytesRange::new(u64::MAX, None);
        range.advance(1);
    }

    #[test]
    #[should_panic(expected = "BytesRange::advance underflow")]
    fn test_bytes_range_advance_size_underflow() {
        let mut range = BytesRange::new(0, Some(1));
        range.advance(2);
    }

    #[test]
    fn test_bytes_range_advance_suffix() {
        let mut range = BytesRange::suffix(4);
        range.advance(2);
        assert_eq!(range, BytesRange::suffix(2));
    }

    #[test]
    #[should_panic(expected = "BytesRange::advance underflow")]
    fn test_bytes_range_advance_suffix_underflow() {
        let mut range = BytesRange::suffix(1);
        range.advance(2);
    }
}
