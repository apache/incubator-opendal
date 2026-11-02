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

use std::fmt;
use std::str;

use crate::EntryMode;
use crate::raw::Timestamp;
use crate::types::compact::CompactFieldWriter;
use crate::types::compact::CompactValues;

const MODE_MASK: u16 = 0b11;
const MODE_FILE: u16 = 0b01;
const MODE_DIR: u16 = 0b10;
const IS_CURRENT_MASK: u16 = 0b11 << 2;
const IS_CURRENT_FALSE: u16 = 0b01 << 2;
const IS_CURRENT_TRUE: u16 = 0b10 << 2;
const IS_DELETED: u16 = 1 << 4;
const HAS_LAST_MODIFIED: u16 = 1 << 5;

const STRING_FIELD_COUNT: usize = 7;
const VALUE_FIELD_COUNT: usize = 9;

#[derive(Clone, Copy)]
#[repr(usize)]
enum ValueField {
    CacheControl,
    ContentDisposition,
    ContentMd5,
    ContentType,
    ContentEncoding,
    Etag,
    Version,
    UserMetadata,
    Path,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct MetadataHeader {
    content_length: u64,
    last_modified_seconds: i64,
    last_modified_nanoseconds: u32,
    flags: u16,
}

impl MetadataHeader {
    fn new(mode: EntryMode) -> Self {
        let mut header = Self {
            content_length: 0,
            last_modified_seconds: 0,
            last_modified_nanoseconds: 0,
            flags: 0,
        };
        header.set_mode(mode);
        header
    }

    fn mode(&self) -> EntryMode {
        match self.flags & MODE_MASK {
            MODE_FILE => EntryMode::FILE,
            MODE_DIR => EntryMode::DIR,
            _ => EntryMode::Unknown,
        }
    }

    fn set_mode(&mut self, mode: EntryMode) {
        self.flags &= !MODE_MASK;
        self.flags |= match mode {
            EntryMode::FILE => MODE_FILE,
            EntryMode::DIR => MODE_DIR,
            EntryMode::Unknown => 0,
        };
    }

    fn is_current(&self) -> Option<bool> {
        match self.flags & IS_CURRENT_MASK {
            IS_CURRENT_FALSE => Some(false),
            IS_CURRENT_TRUE => Some(true),
            _ => None,
        }
    }

    fn set_is_current(&mut self, value: Option<bool>) {
        self.flags &= !IS_CURRENT_MASK;
        self.flags |= match value {
            Some(false) => IS_CURRENT_FALSE,
            Some(true) => IS_CURRENT_TRUE,
            None => 0,
        };
    }

    fn set_flag(&mut self, flag: u16, enabled: bool) {
        if enabled {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }
}

/// Metadata contains all the information related to a specific path.
///
/// Metadata is tied to the operation that produced it. The same path can have
/// different metadata across versions or requests.
///
/// ## File versions
///
/// In systems that support versioning, metadata can represent a specific object
/// version. [`Metadata::version`] returns its version identifier when available.
/// [`Metadata::is_current`] and [`Metadata::is_deleted`] describe whether that
/// version is current or deleted.
///
/// | `is_current` | `is_deleted` | Meaning |
/// | --- | --- | --- |
/// | `Some(true)` | `false` | The current object version. |
/// | `Some(true)` | `true` | The current delete marker or soft-deleted version. |
/// | `Some(false)` | `false` | A previous accessible version. |
/// | `Some(false)` | `true` | A previous deleted version. |
/// | `None` | `false` | A non-deleted object whose current-version status is unknown. |
/// | `None` | `true` | A deleted object whose current-version status is unknown. |
///
/// Metadata is an immutable owned value. Use [`MetadataBuilder`] to create a
/// value and [`Metadata::into_builder`] to create a modified value.
#[derive(Clone)]
pub struct Metadata {
    header: MetadataHeader,
    values: CompactValues,
}

impl PartialEq for Metadata {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
            && (0..ValueField::Path as usize)
                .all(|field| self.values.get(field) == other.values.get(field))
    }
}

impl Eq for Metadata {}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("Metadata");
        ds.field("mode", &self.mode());

        if let Some(value) = self.is_current() {
            ds.field("is_current", &value);
        }
        if self.is_deleted() {
            ds.field("is_deleted", &true);
        }
        if let Some(value) = self.cache_control() {
            ds.field("cache_control", &value);
        }
        if let Some(value) = self.content_disposition() {
            ds.field("content_disposition", &value);
        }
        if self.is_file() {
            ds.field("content_length", &self.content_length());
        }
        if let Some(value) = self.content_md5() {
            ds.field("content_md5", &value);
        }
        if let Some(value) = self.content_type() {
            ds.field("content_type", &value);
        }
        if let Some(value) = self.content_encoding() {
            ds.field("content_encoding", &value);
        }
        if let Some(value) = self.etag() {
            ds.field("etag", &value);
        }
        if let Some(value) = self.last_modified() {
            ds.field("last_modified", &value);
        }
        if let Some(value) = self.version() {
            ds.field("version", &value);
        }
        if let Some(value) = self.user_metadata() {
            ds.field("user_metadata", &value);
        }

        ds.finish()
    }
}

impl Metadata {
    /// Consume this metadata and return a builder initialized with its values.
    #[inline]
    pub fn into_builder(self) -> MetadataBuilder {
        MetadataBuilder::from_metadata(self)
    }

    /// Return this entry's mode.
    #[inline]
    pub fn mode(&self) -> EntryMode {
        self.header.mode()
    }

    /// Return `true` if this metadata is for a file.
    #[inline]
    pub fn is_file(&self) -> bool {
        self.mode().is_file()
    }

    /// Return `true` if this metadata is for a directory.
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.mode().is_dir()
    }

    /// Return whether this metadata describes the current object version.
    ///
    /// `None` means the service did not report whether the version is current.
    #[inline]
    pub fn is_current(&self) -> Option<bool> {
        self.header.is_current()
    }

    /// Return whether this metadata describes a deleted object or delete marker.
    #[inline]
    pub fn is_deleted(&self) -> bool {
        self.header.flags & IS_DELETED != 0
    }

    /// Return this entry's Cache-Control value.
    ///
    /// See [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html).
    #[inline]
    pub fn cache_control(&self) -> Option<&str> {
        self.string(ValueField::CacheControl)
    }

    /// Return the full content length of this entry.
    ///
    /// For file metadata returned by stat, list, or read operations, this value
    /// is the complete object size even when a read returns only a range.
    /// Directory and unknown metadata return zero.
    #[inline]
    pub fn content_length(&self) -> u64 {
        self.header.content_length
    }

    /// Return this entry's Content-MD5 value.
    ///
    /// OpenDAL returns the service-provided value and does not guarantee that it
    /// is the MD5 digest of the content.
    #[inline]
    pub fn content_md5(&self) -> Option<&str> {
        self.string(ValueField::ContentMd5)
    }

    /// Return this entry's Content-Type value.
    ///
    /// See [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#field.content-type).
    #[inline]
    pub fn content_type(&self) -> Option<&str> {
        self.string(ValueField::ContentType)
    }

    /// Return this entry's Content-Encoding value.
    ///
    /// See [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#field.content-encoding).
    #[inline]
    pub fn content_encoding(&self) -> Option<&str> {
        self.string(ValueField::ContentEncoding)
    }

    /// Return this entry's Last-Modified timestamp.
    ///
    /// See [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#field.last-modified).
    #[inline]
    pub fn last_modified(&self) -> Option<Timestamp> {
        if self.header.flags & HAS_LAST_MODIFIED == 0 {
            return None;
        }

        Some(
            Timestamp::new(
                self.header.last_modified_seconds,
                self.header.last_modified_nanoseconds as i32,
            )
            .expect("metadata stores a previously validated timestamp"),
        )
    }

    /// Return this entry's service-provided ETag as-is.
    ///
    /// Quotes and weak-validator prefixes are part of the returned value. See
    /// [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html#field.etag).
    #[inline]
    pub fn etag(&self) -> Option<&str> {
        self.string(ValueField::Etag)
    }

    /// Return this entry's Content-Disposition value.
    ///
    /// See [RFC 6266](https://www.rfc-editor.org/rfc/rfc6266.html).
    #[inline]
    pub fn content_disposition(&self) -> Option<&str> {
        self.string(ValueField::ContentDisposition)
    }

    /// Return this entry's service-specific version identifier.
    #[inline]
    pub fn version(&self) -> Option<&str> {
        self.string(ValueField::Version)
    }

    /// Return a borrowed view of this entry's user metadata.
    ///
    /// Service-specific metadata prefixes are removed from the returned keys.
    #[inline]
    pub fn user_metadata(&self) -> Option<UserMetadata<'_>> {
        self.values
            .get(ValueField::UserMetadata as usize)
            .map(UserMetadata::new)
    }

    #[inline]
    pub(crate) fn path(&self) -> Option<&str> {
        self.string(ValueField::Path)
    }

    #[inline]
    fn string(&self, field: ValueField) -> Option<&str> {
        self.values.get_str(field as usize)
    }
}

/// Mutable construction state for [`Metadata`].
pub struct MetadataBuilder {
    header: MetadataHeader,
    values: CompactValues,
    strings: [Option<String>; STRING_FIELD_COUNT],
    user_metadata: Option<Vec<(String, String)>>,
    path: Option<String>,
}

impl MetadataBuilder {
    /// Create a builder for file metadata with its complete content length.
    #[inline]
    pub fn file(content_length: u64) -> Self {
        let mut builder = Self::fresh(EntryMode::FILE);
        builder.header.content_length = content_length;
        builder
    }

    /// Create a builder for directory metadata.
    #[inline]
    pub fn dir() -> Self {
        Self::fresh(EntryMode::DIR)
    }

    /// Create a builder for metadata whose entry mode is not known.
    #[inline]
    pub fn unknown() -> Self {
        Self::fresh(EntryMode::Unknown)
    }

    fn fresh(mode: EntryMode) -> Self {
        Self {
            header: MetadataHeader::new(mode),
            values: CompactValues::default(),
            strings: Default::default(),
            user_metadata: None,
            path: None,
        }
    }

    fn from_metadata(metadata: Metadata) -> Self {
        Self {
            header: metadata.header,
            values: metadata.values,
            strings: Default::default(),
            user_metadata: None,
            path: None,
        }
    }

    /// Set the entry mode to file and replace its complete content length.
    pub fn set_file(&mut self, content_length: u64) -> &mut Self {
        self.header.set_mode(EntryMode::FILE);
        self.header.content_length = content_length;
        self
    }

    /// Set the entry mode to directory and discard any file content length.
    pub fn set_dir(&mut self) -> &mut Self {
        self.header.set_mode(EntryMode::DIR);
        self.header.content_length = 0;
        self
    }

    /// Set the entry mode to unknown and discard any file content length.
    pub fn set_unknown(&mut self) -> &mut Self {
        self.header.set_mode(EntryMode::Unknown);
        self.header.content_length = 0;
        self
    }

    /// Set whether this metadata describes the current object version.
    pub fn is_current(&mut self, value: Option<bool>) -> &mut Self {
        self.header.set_is_current(value);
        self
    }

    /// Set whether this metadata describes a deleted object.
    pub fn is_deleted(&mut self, value: bool) -> &mut Self {
        self.header.set_flag(IS_DELETED, value);
        self
    }

    /// Set the Cache-Control value.
    pub fn cache_control(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::CacheControl, value.into())
    }

    /// Set the Content-MD5 value.
    pub fn content_md5(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::ContentMd5, value.into())
    }

    /// Set the Content-Type value.
    pub fn content_type(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::ContentType, value.into())
    }

    /// Set the Content-Encoding value.
    pub fn content_encoding(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::ContentEncoding, value.into())
    }

    /// Set the last-modified timestamp.
    pub fn last_modified(&mut self, value: Timestamp) -> &mut Self {
        let value = value.into_inner();
        self.header.last_modified_seconds = value.as_second();
        self.header.last_modified_nanoseconds = value.subsec_nanosecond() as u32;
        self.header.set_flag(HAS_LAST_MODIFIED, true);
        self
    }

    /// Set the ETag.
    pub fn etag(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::Etag, value.into())
    }

    /// Set the Content-Disposition value.
    pub fn content_disposition(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::ContentDisposition, value.into())
    }

    /// Set the object version.
    pub fn version(&mut self, value: impl Into<String>) -> &mut Self {
        self.set_string(ValueField::Version, value.into())
    }

    /// Set user metadata from owned key-value pairs.
    ///
    /// Duplicate keys are rejected because user metadata has map semantics.
    ///
    /// # Panics
    ///
    /// Panics when `values` contains duplicate keys.
    pub fn user_metadata(
        &mut self,
        values: impl IntoIterator<Item = (String, String)>,
    ) -> &mut Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        assert!(
            values.windows(2).all(|pair| pair[0].0 != pair[1].0),
            "user metadata contains duplicate keys"
        );
        self.user_metadata = Some(values);
        self
    }

    /// Finish this builder as immutable metadata.
    ///
    /// # Panics
    ///
    /// Panics when the compact value block, including its index, exceeds `u16::MAX` bytes.
    pub fn build(self) -> Metadata {
        let Self {
            header,
            values,
            strings,
            user_metadata,
            path,
        } = self;
        let has_value_edits =
            strings.iter().any(Option::is_some) || user_metadata.is_some() || path.is_some();
        if !has_value_edits {
            return Metadata { header, values };
        }
        let mut fields = values.fields::<VALUE_FIELD_COUNT>();
        for (index, value) in strings.iter().enumerate() {
            if let Some(value) = value {
                fields[index] = Some(value.as_bytes());
            }
        }
        let user_metadata = user_metadata.as_deref();
        if let Some(path) = path.as_deref() {
            fields[ValueField::Path as usize] = Some(path.as_bytes());
        }

        let mut lengths = fields.map(|value| value.map(<[u8]>::len));
        if let Some(user_metadata) = user_metadata {
            lengths[ValueField::UserMetadata as usize] =
                Some(user_metadata_encoded_len(user_metadata));
        }

        let values = CompactValues::encode_with(&lengths, |field, output| {
            if field == ValueField::UserMetadata as usize
                && let Some(user_metadata) = user_metadata
            {
                write_user_metadata(user_metadata, output);
            } else {
                output.write(fields[field].expect("present field has a value"));
            }
        });

        Metadata { header, values }
    }

    pub(crate) fn path(&mut self, value: impl Into<String>) -> &mut Self {
        self.path = Some(value.into());
        self
    }

    fn set_string(&mut self, field: ValueField, value: String) -> &mut Self {
        let index = field as usize;
        debug_assert!(index < STRING_FIELD_COUNT);
        self.strings[index] = Some(value);
        self
    }
}

/// Borrowed user metadata stored in a [`Metadata`] value.
#[derive(Clone, Copy)]
pub struct UserMetadata<'a> {
    encoded: &'a [u8],
}

impl<'a> UserMetadata<'a> {
    #[inline]
    pub(crate) fn new(encoded: &'a [u8]) -> Self {
        Self { encoded }
    }

    /// Return the value associated with `key`.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&'a str> {
        let mut left = 0;
        let mut right = self.len();
        let header_len = 2 + right * 4;
        while left < right {
            let middle = left + (right - left) / 2;
            let (candidate, value) = user_metadata_entry(self.encoded, middle, header_len);
            match candidate.cmp(key) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => return Some(value),
            }
        }
        None
    }

    /// Return the number of user metadata pairs.
    #[inline]
    pub fn len(&self) -> usize {
        user_metadata_count(self.encoded)
    }

    /// Return `true` when no user metadata pairs are present.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for UserMetadata<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(*self).finish()
    }
}

impl serde::Serialize for UserMetadata<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(*self)
    }
}

/// Iterator over borrowed user metadata pairs.
pub struct UserMetadataIntoIter<'a> {
    encoded: &'a [u8],
    index: usize,
    header_len: usize,
}

impl<'a> Iterator for UserMetadataIntoIter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if 2 + self.index * 4 == self.header_len {
            return None;
        }
        let entry = user_metadata_entry(self.encoded, self.index, self.header_len);
        self.index += 1;
        Some(entry)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.header_len - 2) / 4 - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for UserMetadataIntoIter<'_> {}

impl<'a> IntoIterator for UserMetadata<'a> {
    type Item = (&'a str, &'a str);
    type IntoIter = UserMetadataIntoIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        UserMetadataIntoIter {
            encoded: self.encoded,
            index: 0,
            header_len: 2 + len * 4,
        }
    }
}

impl<'a> IntoIterator for &UserMetadata<'a> {
    type Item = (&'a str, &'a str);
    type IntoIter = UserMetadataIntoIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        (*self).into_iter()
    }
}

pub(crate) fn user_metadata_encoded_len(values: &[(String, String)]) -> usize {
    u16::try_from(values.len()).expect("user metadata contains too many pairs");
    let header_len = size_of::<u16>() + values.len() * 2 * size_of::<u16>();
    let payload_len: usize = values
        .iter()
        .map(|(key, value)| key.len() + value.len())
        .sum();
    let total_len = header_len
        .checked_add(payload_len)
        .expect("user metadata length overflowed");
    assert!(
        total_len <= u16::MAX as usize,
        "user metadata exceeds 64 KiB"
    );
    total_len
}

pub(crate) fn write_user_metadata(
    values: &[(String, String)],
    output: &mut CompactFieldWriter<'_>,
) {
    let count = u16::try_from(values.len()).expect("user metadata contains too many pairs");
    let header_len = size_of::<u16>() + values.len() * 2 * size_of::<u16>();
    output.write(&count.to_le_bytes());

    let mut payload_offset = header_len;
    for (key, value) in values {
        for part in [key.as_bytes(), value.as_bytes()] {
            payload_offset += part.len();
            let end = u16::try_from(payload_offset).expect("user metadata length was validated");
            output.write(&end.to_le_bytes());
        }
    }

    for (key, value) in values {
        output.write(key.as_bytes());
        output.write(value.as_bytes());
    }
}

#[inline]
fn user_metadata_count(encoded: &[u8]) -> usize {
    u16::from_le_bytes(
        encoded[..2]
            .try_into()
            .expect("user metadata contains its pair count"),
    ) as usize
}

#[inline]
fn user_metadata_entry(encoded: &[u8], index: usize, header_len: usize) -> (&str, &str) {
    debug_assert!(index * 4 + 6 <= header_len);
    let key_offset = index * 2;
    let key_start = if key_offset == 0 {
        header_len
    } else {
        user_metadata_offset(encoded, key_offset - 1)
    };
    let key_end = user_metadata_offset(encoded, key_offset);
    let value_end = user_metadata_offset(encoded, key_offset + 1);
    (
        // User metadata enters this encoding from owned UTF-8 strings.
        unsafe { str::from_utf8_unchecked(&encoded[key_start..key_end]) },
        // User metadata enters this encoding from owned UTF-8 strings.
        unsafe { str::from_utf8_unchecked(&encoded[key_end..value_end]) },
    )
}

#[inline]
fn user_metadata_offset(encoded: &[u8], index: usize) -> usize {
    let offset = 2 + index * 2;
    u16::from_le_bytes(
        encoded[offset..offset + 2]
            .try_into()
            .expect("user metadata contains a complete offset"),
    ) as usize
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn metadata_layout_is_40_bytes() {
        assert_eq!(size_of::<MetadataHeader>(), 24);
        assert_eq!(size_of::<CompactValues>(), 16);
        assert_eq!(size_of::<Metadata>(), 40);
    }

    #[test]
    fn debug_metadata_omits_default_values() {
        let metadata = MetadataBuilder::file(0).build();
        assert_eq!(
            format!("{metadata:?}"),
            "Metadata { mode: FILE, content_length: 0 }"
        );
    }

    #[test]
    fn metadata_roundtrip() {
        let timestamp = Timestamp::new(-1, -123_456_789).unwrap();
        let mut builder = MetadataBuilder::file(42);
        builder
            .is_current(Some(false))
            .is_deleted(true)
            .last_modified(timestamp)
            .version("v1")
            .user_metadata([
                ("owner".to_string(), "opendal".to_string()),
                ("region".to_string(), "us-east-1".to_string()),
            ]);
        let metadata = builder.build();

        assert_eq!(metadata.mode(), EntryMode::FILE);
        assert_eq!(metadata.is_current(), Some(false));
        assert!(metadata.is_deleted());
        assert_eq!(metadata.content_length(), 42);
        assert_eq!(metadata.last_modified(), Some(timestamp));
        assert_eq!(metadata.version(), Some("v1"));
        assert_eq!(
            metadata.user_metadata().unwrap().get("owner"),
            Some("opendal")
        );
        assert_eq!(
            metadata
                .user_metadata()
                .unwrap()
                .into_iter()
                .collect::<HashMap<_, _>>()
                .len(),
            2
        );
    }

    #[test]
    fn into_builder_preserves_values_for_header_changes() {
        let mut builder = MetadataBuilder::file(0);
        builder.etag("etag");
        let metadata = builder.build();
        let cloned = metadata.clone();

        let mut builder = metadata.into_builder();
        builder.set_file(42);
        let changed = builder.build();

        assert_eq!(cloned.content_length(), 0);
        assert_eq!(changed.content_length(), 42);
        assert_eq!(changed.etag(), Some("etag"));
        assert_eq!(cloned.etag(), Some("etag"));
    }

    #[test]
    fn mode_transitions_reset_content_length() {
        let mut builder = MetadataBuilder::file(42);
        builder.set_dir();
        let directory = builder.build();
        assert!(directory.is_dir());
        assert_eq!(directory.content_length(), 0);

        let mut builder = directory.into_builder();
        builder.set_file(7);
        let file = builder.build();
        assert!(file.is_file());
        assert_eq!(file.content_length(), 7);

        let mut builder = file.into_builder();
        builder.set_unknown();
        let unknown = builder.build();
        assert_eq!(unknown.mode(), EntryMode::Unknown);
        assert_eq!(unknown.content_length(), 0);
    }

    #[test]
    fn into_builder_does_not_mutate_shared_packed_values() {
        let mut builder = MetadataBuilder::file(0);
        builder.etag("original").version("version").user_metadata([
            ("owner".to_string(), "opendal".to_string()),
            ("region".to_string(), "us-east-1".to_string()),
        ]);
        let original = builder.build();

        let mut builder = original.clone().into_builder();
        builder
            .etag("changed")
            .user_metadata([("owner".to_string(), "another-owner".to_string())]);
        let changed = builder.build();

        assert_eq!(original.etag(), Some("original"));
        assert_eq!(original.version(), Some("version"));
        assert_eq!(original.user_metadata().unwrap().len(), 2);
        assert_eq!(changed.etag(), Some("changed"));
        assert_eq!(changed.version(), Some("version"));
        assert_eq!(
            changed.user_metadata().unwrap().get("owner"),
            Some("another-owner")
        );
    }

    #[test]
    fn empty_user_metadata_is_present() {
        let mut builder = MetadataBuilder::file(0);
        builder.user_metadata([]);
        let metadata = builder.build();

        assert!(metadata.user_metadata().unwrap().is_empty());
    }

    #[test]
    fn internal_entry_path_does_not_change_metadata_equality() {
        let metadata = MetadataBuilder::file(0).build();
        let mut builder = metadata.clone().into_builder();
        builder.path("path");
        let metadata_with_path = builder.build();

        assert_eq!(metadata, metadata_with_path);
    }
}
