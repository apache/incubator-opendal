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

#![allow(
    rustdoc::broken_intra_doc_links,
    reason = "YARD's syntax for documentation"
)]
#![allow(rustdoc::invalid_html_tags, reason = "YARD's syntax for documentation")]
#![allow(rustdoc::bare_urls, reason = "YARD's syntax for documentation")]

use magnus::Error;
use magnus::RModule;
use magnus::method;

use crate::*;

/// @yard
/// Metadata about an object.
#[magnus::wrap(class = "OpenDal::Metadata", free_immediately, size)]
pub struct Metadata(ocore::Metadata);

impl Metadata {
    pub fn new(metadata: ocore::Metadata) -> Self {
        Self(metadata)
    }
}

impl Metadata {
    /// @yard
    /// @def mode
    /// The entry mode of this Metadata.
    /// @return [String] one of `File`, `Directory`, `Unknown`.
    ///    `Unknown` means OpenDAL doesn't have actions to the entry.
    pub fn mode(&self) -> &str {
        match self.0.mode() {
            ocore::EntryMode::FILE => "File",
            ocore::EntryMode::DIR => "Directory",
            ocore::EntryMode::Unknown => "Unknown",
        }
    }

    /// @yard
    /// @def content_disposition
    /// Content-Disposition of this object
    /// @return [String, nil]
    pub fn content_disposition(&self) -> Option<&str> {
        self.0.content_disposition()
    }

    /// @yard
    /// @def content_length
    /// Content length of this entry.
    /// @return [Integer]
    pub fn content_length(&self) -> u64 {
        self.0.content_length()
    }

    /// @yard
    /// @def content_md5
    /// Content MD5 of this entry.
    /// @return [String, nil]
    pub fn content_md5(&self) -> Option<&str> {
        self.0.content_md5()
    }

    /// @yard
    /// @def content_type
    /// Content Type of this entry.
    /// @return [String, nil]
    pub fn content_type(&self) -> Option<&str> {
        self.0.content_type()
    }

    /// @yard
    /// @def etag
    /// ETag of this entry.
    /// @return [String, nil]
    pub fn etag(&self) -> Option<&str> {
        self.0.etag()
    }
}

pub fn include(ruby: &Ruby, gem_module: &RModule) -> Result<(), Error> {
    let class = gem_module.define_class("Metadata", ruby.class_object())?;
    class.define_method("mode", method!(Metadata::mode, 0))?;
    class.define_method(
        "content_disposition",
        method!(Metadata::content_disposition, 0),
    )?;
    class.define_method("content_length", method!(Metadata::content_length, 0))?;
    class.define_method("content_md5", method!(Metadata::content_md5, 0))?;
    class.define_method("content_type", method!(Metadata::content_type, 0))?;
    class.define_method("etag", method!(Metadata::etag, 0))?;

    Ok(())
}
