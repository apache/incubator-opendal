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

/// Entry is returned by `Page` or `BlockingPage` during list operations.
///
/// # Notes
///
/// Differences between `crate::Entry` and `oio::Entry`:
///
/// - `crate::Entry` is the user's public API and have less public methods.
/// - `oio::Entry` is the raw API and doesn't expose to users.
#[derive(Debug, Clone)]
pub struct Entry {
    meta: Metadata,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.path() == other.path() && self.meta == other.meta
    }
}

impl Eq for Entry {}

impl Entry {
    /// Create a new entry by its corresponding underlying storage.
    ///
    /// # Panics
    ///
    /// Panics when the compact metadata block containing `path` exceeds `u16::MAX` bytes.
    pub fn new(path: &str, meta: Metadata) -> Entry {
        Self::with(path.to_string(), meta)
    }

    /// Create a new entry with given value.
    ///
    /// # Panics
    ///
    /// Panics when the compact metadata block containing `path` exceeds `u16::MAX` bytes.
    pub fn with(mut path: String, meta: Metadata) -> Entry {
        // Normalize path as `/` if it's empty.
        if path.is_empty() {
            path = "/".to_string();
        }

        debug_assert!(
            meta.mode().is_dir() == path.ends_with('/'),
            "mode {:?} not match with path {}",
            meta.mode(),
            path
        );

        let mut builder = meta.into_builder();
        builder.path(path);
        Entry {
            meta: builder.build(),
        }
    }

    /// Set path for entry.
    ///
    /// # Panics
    ///
    /// Panics when the resulting compact metadata block exceeds `u16::MAX` bytes.
    pub fn set_path(&mut self, path: &str) -> &mut Self {
        let mut builder = self.meta.clone().into_builder();
        builder.path(path);
        self.meta = builder.build();
        self
    }

    /// Get the path of entry.
    pub fn path(&self) -> &str {
        self.meta
            .path()
            .expect("listed entry metadata contains its path")
    }

    /// Get entry's mode.
    pub fn mode(&self) -> EntryMode {
        self.meta.mode()
    }

    /// Consume self to convert into an Entry.
    ///
    /// NOTE: implement this by hand to avoid leaking raw entry to end-users.
    pub(crate) fn into_entry(self) -> crate::Entry {
        crate::Entry::from_metadata(self.meta)
    }

    /// Get metadata of entry.
    pub fn metadata(&self) -> &Metadata {
        &self.meta
    }

    /// Consume this entry to get its path and metadata.
    pub fn into_parts(self) -> (String, Metadata) {
        (self.path().to_string(), self.meta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_equality_includes_path() {
        let metadata = MetadataBuilder::file(0).build();
        assert_ne!(
            Entry::new("first", metadata.clone()),
            Entry::new("second", metadata)
        );
    }
}
