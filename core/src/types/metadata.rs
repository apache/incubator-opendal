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

use chrono::prelude::*;

use crate::raw::*;
use crate::*;

/// Metadata contains all the information related to a specific path.
///
/// Depending on the context of the requests, the metadata for the same path may vary. For example, two
/// versions of the same path might have different content lengths. Keep in mind that metadata is always
/// tied to the given context and is not a global state.
///
/// ## File Versions
///
/// In systems that support versioning, such as AWS S3, the metadata may represent a specific version
/// of a file.
///
/// Users can access [`Metadata::version`] to retrieve the file's version, if available. They can also
/// use [`Metadata::is_current`] and [`Metadata::is_deleted`] to determine whether the metadata
/// corresponds to the latest version or a deleted one.
///
/// The all possible combinations of `is_current` and `is_deleted` are as follows:
///
/// | `is_current`  | `is_deleted` | description                                                                                                                                                                                                                                                                                                                                                                          |
/// |---------------|--------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
/// | `Some(true)`  | `false`      | **The metadata's associated version is the latest, current version.** This is the normal state, indicating that this version is the most up-to-date and accessible version.                                                                                                                                                                                                          |
/// | `Some(true)`  | `true`       | **The metadata's associated version is the latest, deleted version (Latest Delete Marker or Soft Deleted).** This is particularly important in object storage systems like S3. It signifies that this version is the **most recent delete marker**, indicating the object has been deleted. Subsequent GET requests will return 404 errors unless a specific version ID is provided. |
/// | `Some(false)` | `false`      | **The metadata's associated version is neither the latest version nor deleted.** This indicates that this version is a previous version, still accessible by specifying its version ID.                                                                                                                                                                                              |
/// | `Some(false)` | `true`       | **The metadata's associated version is not the latest version and is deleted.** This represents a historical version that has been marked for deletion. Users will need to specify the version ID to access it, and accessing it may be subject to specific delete marker behavior (e.g., in S3, it might not return actual data but a specific delete marker response).             |
/// | `None`        | `false`      | **The metadata's associated file is not deleted, but its version status is either unknown or it is not the latest version.** This likely indicates that versioning is not enabled for this file, or versioning information is unavailable.                                                                                                                                           |
/// | `None`        | `true`       | **The metadata's associated file is deleted, but its version status is either unknown or it is not the latest version.** This typically means the file was deleted without versioning enabled, or its versioning information is unavailable. This may represent an actual data deletion operation rather than an S3 delete marker.                                                   |
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Metadata {
    mode: EntryMode,

    is_current: Option<bool>,
    is_deleted: bool,

    cache_control: Option<String>,
    content_disposition: Option<String>,
    content_length: Option<u64>,
    content_md5: Option<String>,
    content_range: Option<BytesContentRange>,
    content_type: Option<String>,
    content_encoding: Option<String>,
    etag: Option<String>,
    last_modified: Option<DateTime<Utc>>,
    version: Option<String>,

    user_metadata: Option<HashMap<String, String>>,
}

impl Metadata {
    /// Create a new metadata
    pub fn new(mode: EntryMode) -> Self {
        Self {
            mode,

            is_current: None,
            is_deleted: false,

            cache_control: None,
            content_length: None,
            content_md5: None,
            content_type: None,
            content_encoding: None,
            content_range: None,
            last_modified: None,
            etag: None,
            content_disposition: None,
            version: None,
            user_metadata: None,
        }
    }

    /// mode represent this entry's mode.
    pub fn mode(&self) -> EntryMode {
        self.mode
    }

    /// Set mode for entry.
    pub fn set_mode(&mut self, v: EntryMode) -> &mut Self {
        self.mode = v;
        self
    }

    /// Set mode for entry.
    pub fn with_mode(mut self, v: EntryMode) -> Self {
        self.mode = v;
        self
    }

    /// Returns `true` if this metadata is for a file.
    pub fn is_file(&self) -> bool {
        matches!(self.mode, EntryMode::FILE)
    }

    /// Returns `true` if this metadata is for a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self.mode, EntryMode::DIR)
    }

    /// Checks whether the metadata corresponds to the most recent version of the file.
    ///
    /// This function is particularly useful when working with versioned objects,
    /// such as those stored in systems like AWS S3 with versioning enabled. It helps
    /// determine if the retrieved metadata represents the current state of the file
    /// or an older version.
    ///
    /// Refer to docs in [`Metadata`] for more information about file versions.
    ///
    /// # Return Value
    ///
    /// The function returns an `Option<bool>` which can have the following values:
    ///
    /// - `Some(true)`:  Indicates that the metadata **is** associated with the latest version of the file.
    ///   The metadata is current and reflects the most up-to-date state.
    /// - `Some(false)`: Indicates that the metadata **is not** associated with the latest version of the file.
    ///   The metadata belongs to an older version, and there might be a more recent version available.
    /// - `None`:      Indicates that the currency of the metadata **cannot be determined**. This might occur if
    ///   versioning is not supported or enabled, or if there is insufficient information to ascertain the version status.
    pub fn is_current(&self) -> Option<bool> {
        self.is_current
    }

    /// Set the `is_current` status of this entry.
    ///
    /// By default, this value will be `None`. Please avoid using this API if it's unclear whether the entry is current.
    /// Set it to `true` if it is known to be the latest; otherwise, set it to `false`.
    pub fn set_is_current(&mut self, is_current: bool) -> &mut Self {
        self.is_current = Some(is_current);
        self
    }

    /// Set the `is_current` status of this entry.
    ///
    /// By default, this value will be `None`. Please avoid using this API if it's unclear whether the entry is current.
    /// Set it to `true` if it is known to be the latest; otherwise, set it to `false`.
    pub fn with_is_current(mut self, is_current: Option<bool>) -> Self {
        self.is_current = is_current;
        self
    }

    /// Checks if the file (or version) associated with this metadata has been deleted.
    ///
    /// This function returns `true` if the file represented by this metadata has been marked for
    /// deletion or has been permanently deleted.
    /// It returns `false` otherwise, indicating that the file (or version) is still present and accessible.
    ///
    /// Refer to docs in [`Metadata`] for more information about file versions.
    ///
    /// # Returns
    ///
    /// `bool`: `true` if the object is considered deleted, `false` otherwise.
    pub fn is_deleted(&self) -> bool {
        self.is_deleted
    }

    /// Set the deleted status of this entry.
    pub fn set_is_deleted(&mut self, v: bool) -> &mut Self {
        self.is_deleted = v;
        self
    }

    /// Set the deleted status of this entry.
    pub fn with_is_deleted(mut self, v: bool) -> Self {
        self.is_deleted = v;
        self
    }

    /// Cache control of this entry.
    ///
    /// Cache-Control is defined by [RFC 7234](https://httpwg.org/specs/rfc7234.html#header.cache-control)
    /// Refer to [MDN Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) for more information.
    pub fn cache_control(&self) -> Option<&str> {
        self.cache_control.as_deref()
    }

    /// Set cache control of this entry.
    ///
    /// Cache-Control is defined by [RFC 7234](https://httpwg.org/specs/rfc7234.html#header.cache-control)
    /// Refer to [MDN Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) for more information.
    pub fn set_cache_control(&mut self, v: &str) -> &mut Self {
        self.cache_control = Some(v.to_string());
        self
    }

    /// Set cache control of this entry.
    ///
    /// Cache-Control is defined by [RFC 7234](https://httpwg.org/specs/rfc7234.html#header.cache-control)
    /// Refer to [MDN Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) for more information.
    pub fn with_cache_control(mut self, v: String) -> Self {
        self.cache_control = Some(v);
        self
    }

    /// Content length of this entry.
    ///
    /// `Content-Length` is defined by [RFC 7230](https://httpwg.org/specs/rfc7230.html#header.content-length)
    ///
    /// Refer to [MDN Content-Length](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Length) for more information.
    ///
    /// # Returns
    ///
    /// Content length of this entry. It will be `0` if the content length is not set by the storage services.
    pub fn content_length(&self) -> u64 {
        self.content_length.unwrap_or_default()
    }

    /// Set content length of this entry.
    pub fn set_content_length(&mut self, v: u64) -> &mut Self {
        self.content_length = Some(v);
        self
    }

    /// Set content length of this entry.
    pub fn with_content_length(mut self, v: u64) -> Self {
        self.content_length = Some(v);
        self
    }

    /// Content MD5 of this entry.
    ///
    /// Content MD5 is defined by [RFC 2616](http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html).
    /// And removed by [RFC 7231](https://www.rfc-editor.org/rfc/rfc7231).
    ///
    /// OpenDAL will try its best to set this value, but not guarantee this value is the md5 of content.
    pub fn content_md5(&self) -> Option<&str> {
        self.content_md5.as_deref()
    }

    /// Set content MD5 of this entry.
    pub fn set_content_md5(&mut self, v: &str) -> &mut Self {
        self.content_md5 = Some(v.to_string());
        self
    }

    /// Set content MD5 of this entry.
    pub fn with_content_md5(mut self, v: String) -> Self {
        self.content_md5 = Some(v);
        self
    }

    /// Content Type of this entry.
    ///
    /// Content Type is defined by [RFC 9110](https://httpwg.org/specs/rfc9110.html#field.content-type).
    ///
    /// Refer to [MDN Content-Type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) for more information.
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Set Content Type of this entry.
    pub fn set_content_type(&mut self, v: &str) -> &mut Self {
        self.content_type = Some(v.to_string());
        self
    }

    /// Set Content Type of this entry.
    pub fn with_content_type(mut self, v: String) -> Self {
        self.content_type = Some(v);
        self
    }

    /// Content Encoding of this entry.
    ///
    /// Content Encoding is defined by [RFC 7231](https://httpwg.org/specs/rfc7231.html#header.content-encoding)
    ///
    /// Refer to [MDN Content-Encoding](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Encoding) for more information.
    pub fn content_encoding(&self) -> Option<&str> {
        self.content_encoding.as_deref()
    }

    /// Set Content Encoding of this entry.
    pub fn set_content_encoding(&mut self, v: &str) -> &mut Self {
        self.content_encoding = Some(v.to_string());
        self
    }

    /// Content Range of this entry.
    ///
    /// Content Range is defined by [RFC 9110](https://httpwg.org/specs/rfc9110.html#field.content-range).
    ///
    /// Refer to [MDN Content-Range](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Range) for more information.
    pub fn content_range(&self) -> Option<BytesContentRange> {
        self.content_range
    }

    /// Set Content Range of this entry.
    pub fn set_content_range(&mut self, v: BytesContentRange) -> &mut Self {
        self.content_range = Some(v);
        self
    }

    /// Set Content Range of this entry.
    pub fn with_content_range(mut self, v: BytesContentRange) -> Self {
        self.content_range = Some(v);
        self
    }

    /// Last modified of this entry.
    ///
    /// `Last-Modified` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.last-modified)
    ///
    /// Refer to [MDN Last-Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) for more information.
    pub fn last_modified(&self) -> Option<DateTime<Utc>> {
        self.last_modified
    }

    /// Set Last modified of this entry.
    pub fn set_last_modified(&mut self, v: DateTime<Utc>) -> &mut Self {
        self.last_modified = Some(v);
        self
    }

    /// Set Last modified of this entry.
    pub fn with_last_modified(mut self, v: DateTime<Utc>) -> Self {
        self.last_modified = Some(v);
        self
    }

    /// ETag of this entry.
    ///
    /// `ETag` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.etag)
    ///
    /// Refer to [MDN ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) for more information.
    ///
    /// OpenDAL will return this value AS-IS like the following:
    ///
    /// - `"33a64df551425fcc55e4d42a148795d9f25f89d4"`
    /// - `W/"0815"`
    ///
    /// `"` is part of etag.
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Set ETag of this entry.
    pub fn set_etag(&mut self, v: &str) -> &mut Self {
        self.etag = Some(v.to_string());
        self
    }

    /// Set ETag of this entry.
    pub fn with_etag(mut self, v: String) -> Self {
        self.etag = Some(v);
        self
    }

    /// Content-Disposition of this entry
    ///
    /// `Content-Disposition` is defined by [RFC 2616](https://www.rfc-editor/rfcs/2616) and
    /// clarified usage in [RFC 6266](https://www.rfc-editor/6266).
    ///
    /// Refer to [MDN Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition) for more information.
    ///
    /// OpenDAL will return this value AS-IS like the following:
    ///
    /// - "inline"
    /// - "attachment"
    /// - "attachment; filename=\"filename.jpg\""
    pub fn content_disposition(&self) -> Option<&str> {
        self.content_disposition.as_deref()
    }

    /// Set Content-Disposition of this entry
    pub fn set_content_disposition(&mut self, v: &str) -> &mut Self {
        self.content_disposition = Some(v.to_string());
        self
    }

    /// Set Content-Disposition of this entry
    pub fn with_content_disposition(mut self, v: String) -> Self {
        self.content_disposition = Some(v);
        self
    }

    /// Retrieves the `version` of the file, if available.
    ///
    /// The version is typically used in systems that support object versioning, such as AWS S3.
    ///
    /// # Returns
    ///
    /// - `Some(&str)`: If the file has a version associated with it,
    ///   this function returns `Some` containing a reference to the version ID string.
    /// - `None`: If the file does not have a version, or if versioning is
    ///   not supported or enabled for the underlying storage system, this function
    ///   returns `None`.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Set the version of the file
    pub fn set_version(&mut self, v: &str) -> &mut Self {
        self.version = Some(v.to_string());
        self
    }

    /// With the version of the file.
    pub fn with_version(mut self, v: String) -> Self {
        self.version = Some(v);
        self
    }

    /// User defined metadata of this entry
    ///
    /// The prefix of the user defined metadata key(for example: in oss, it's x-oss-meta-)
    /// is remove from the key
    pub fn user_metadata(&self) -> Option<&HashMap<String, String>> {
        self.user_metadata.as_ref()
    }

    /// Set user defined metadata of this entry
    pub fn with_user_metadata(&mut self, data: HashMap<String, String>) -> &mut Self {
        self.user_metadata = Some(data);
        self
    }
}
