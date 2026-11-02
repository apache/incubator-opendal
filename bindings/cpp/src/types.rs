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

use opendal as od;

use super::ffi;

impl From<od::Metadata> for ffi::Metadata {
    fn from(meta: od::Metadata) -> Self {
        let mode = meta.mode().into();
        let content_length = meta.content_length();
        let cache_control = meta.cache_control().map(ToOwned::to_owned).into();
        let content_disposition = meta.content_disposition().map(ToOwned::to_owned).into();
        let content_md5 = meta.content_md5().map(ToOwned::to_owned).into();
        let content_type = meta.content_type().map(ToOwned::to_owned).into();
        let content_encoding = meta.content_encoding().map(ToOwned::to_owned).into();
        let etag = meta.etag().map(ToOwned::to_owned).into();
        let last_modified = meta.last_modified().map(|time| time.to_string()).into();
        let version = meta.version().map(ToOwned::to_owned).into();
        let is_current = meta.is_current().into();
        let is_deleted = meta.is_deleted();

        Self {
            mode,
            content_length,
            cache_control,
            content_disposition,
            content_md5,
            content_type,
            content_encoding,
            etag,
            last_modified,
            version,
            is_current,
            is_deleted,
        }
    }
}

impl From<od::Entry> for ffi::Entry {
    fn from(entry: od::Entry) -> Self {
        let (path, _) = entry.into_parts();
        Self { path }
    }
}

impl From<od::EntryMode> for ffi::EntryMode {
    fn from(mode: od::EntryMode) -> Self {
        match mode {
            od::EntryMode::FILE => Self::File,
            od::EntryMode::DIR => Self::Dir,
            _ => Self::Unknown,
        }
    }
}

impl From<Option<String>> for ffi::OptionalString {
    fn from(s: Option<String>) -> Self {
        match s {
            Some(s) => Self {
                has_value: true,
                value: s,
            },
            None => Self {
                has_value: false,
                value: String::default(),
            },
        }
    }
}

impl From<Option<bool>> for ffi::OptionalBool {
    fn from(b: Option<bool>) -> Self {
        match b {
            Some(b) => Self {
                has_value: true,
                value: b,
            },
            None => Self {
                has_value: false,
                value: false,
            },
        }
    }
}

impl From<Option<od::Entry>> for ffi::OptionalEntry {
    fn from(entry: Option<od::Entry>) -> Self {
        match entry {
            Some(entry) => Self {
                has_value: true,
                value: entry.into(),
            },
            None => Self {
                has_value: false,
                value: ffi::Entry {
                    path: String::default(),
                },
            },
        }
    }
}
