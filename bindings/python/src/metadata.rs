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

use crate::*;
use std::collections::HashMap;

/// Entry
///
/// An entry representing a path and its associated metadata.
///
/// Notes
/// -----
/// If this entry is a directory, ``path`` **must** end with ``/``.
/// Otherwise, ``path`` **must not** end with ``/``.
#[pyclass(module = "opendal.types")]
pub struct Entry(ocore::Entry);

impl Entry {
    pub fn new(entry: ocore::Entry) -> Self {
        Self(entry)
    }
}
#[pymethods]
impl Entry {
    /// The path of entry relative to the operator's root.
    #[getter]
    pub fn path(&self) -> &str {
        self.0.path()
    }

    /// The name of entry, representing the last segment of the path.
    #[getter]
    pub fn name(&self) -> &str {
        self.0.name()
    }

    /// The metadata of this entry.
    #[getter]
    pub fn metadata(&self) -> Metadata {
        Metadata::new(self.0.metadata().clone())
    }
    fn __str__(&self) -> &str {
        self.0.path()
    }
    fn __repr__(&self) -> String {
        format!(
            "Entry(path={:?}, metadata={})",
            self.path(),
            self.metadata().__repr__()
        )
    }
}

/// The metadata of an ``Entry``.
///
/// The metadata is always tied to a specific context and is not a global
/// state. For example, two versions of the same path might have different
/// content lengths.
///
/// Notes
/// -----
/// In systems that support versioning, such as AWS S3, the metadata may
/// represent a specific version of a file. Use :attr:`version` to get
/// the version of a file if it is available.
#[pyclass(module = "opendal.types")]
pub struct Metadata(ocore::Metadata);

impl Metadata {
    pub fn new(meta: ocore::Metadata) -> Self {
        Self(meta)
    }
}
#[pymethods]
impl Metadata {
    /// The content disposition of this entry.
    #[getter]
    pub fn content_disposition(&self) -> Option<&str> {
        self.0.content_disposition()
    }

    /// The content length of this entry.
    #[getter]
    pub fn content_length(&self) -> u64 {
        self.0.content_length()
    }

    /// The content MD5 of this entry.
    #[getter]
    pub fn content_md5(&self) -> Option<&str> {
        self.0.content_md5()
    }

    /// The content type of this entry.
    #[getter]
    pub fn content_type(&self) -> Option<&str> {
        self.0.content_type()
    }

    /// The content encoding of this entry.
    #[getter]
    pub fn content_encoding(&self) -> Option<&str> {
        self.0.content_encoding()
    }

    /// The ETag of this entry.
    #[getter]
    pub fn etag(&self) -> Option<&str> {
        self.0.etag()
    }

    /// The mode of this entry.
    #[getter]
    pub fn mode(&self) -> EntryMode {
        EntryMode::new(self.0.mode())
    }

    /// Whether this entry is a file.
    #[getter]
    pub fn is_file(&self) -> bool {
        self.mode().is_file()
    }

    /// Whether this entry is a directory.
    #[getter]
    pub fn is_dir(&self) -> bool {
        self.mode().is_dir()
    }

    /// The last modified timestamp of this entry.
    #[getter]
    pub fn last_modified(&self) -> Option<jiff::Timestamp> {
        self.0.last_modified().map(Into::into)
    }

    /// The version of this entry.
    #[getter]
    pub fn version(&self) -> Option<&str> {
        self.0.version()
    }

    /// The user-defined metadata of this entry.
    #[getter]
    pub fn user_metadata(&self) -> Option<HashMap<String, String>> {
        self.0.user_metadata().map(|metadata| {
            metadata
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        })
    }
    pub fn __repr__(&self) -> String {
        let mut parts = vec![];

        parts.push(format!("mode={}", self.0.mode()));
        parts.push(format!(
            "content_disposition={:?}",
            self.0.content_disposition()
        ));
        parts.push(format!("content_length={}", self.0.content_length()));
        parts.push(format!("content_md5={:?}", self.0.content_md5()));
        parts.push(format!("content_type={:?}", self.0.content_type()));
        parts.push(format!("content_encoding={:?}", self.0.content_encoding()));
        parts.push(format!("etag={:?}", self.0.etag()));
        parts.push(format!("last_modified={:?}", self.0.last_modified()));
        parts.push(format!("version={:?}", self.0.version()));
        parts.push(format!("user_metadata={:?}", self.0.user_metadata()));

        format!("Metadata({})", parts.join(", "))
    }
}

/// EntryMode
///
/// The mode of an entry, indicating if it is a file or a directory.
#[pyclass(
    eq,
    eq_int,
    hash,
    frozen,
    module = "opendal.types",
    skip_from_py_object
)]
#[pyo3(rename_all = "PascalCase")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryMode {
    /// The entry is a file and has data to read.
    FILE,
    /// The entry is a directory and can be listed.
    DIR,
    /// The mode of the entry is unknown.
    Unknown,
}

impl EntryMode {
    pub fn new(mode: ocore::EntryMode) -> Self {
        match mode {
            ocore::EntryMode::FILE => Self::FILE,
            ocore::EntryMode::DIR => Self::DIR,
            ocore::EntryMode::Unknown => Self::Unknown,
        }
    }
}
#[pymethods]
impl EntryMode {
    /// Check if the entry mode is `File`.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if the entry is a file.
    pub fn is_file(&self) -> bool {
        matches!(self, EntryMode::FILE)
    }

    /// Check if the entry mode is `Dir`.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if the entry is a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self, EntryMode::DIR)
    }
}
