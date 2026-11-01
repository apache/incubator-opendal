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

/// EntryMode represents the mode.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum EntryMode {
    /// FILE means the path has data to read.
    FILE,
    /// DIR means the path can be listed.
    DIR,
    /// Unknown means we don't know what we can do on this path.
    #[default]
    Unknown,
}

impl EntryMode {
    /// Check if this mode is FILE.
    pub fn is_file(self) -> bool {
        self == EntryMode::FILE
    }

    /// Check if this mode is DIR.
    pub fn is_dir(self) -> bool {
        self == EntryMode::DIR
    }

    /// Create entry mode from given path.
    #[allow(dead_code)]
    pub fn from_path(path: &str) -> Self {
        if path.ends_with('/') {
            EntryMode::DIR
        } else {
            EntryMode::FILE
        }
    }
}

impl Display for EntryMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryMode::FILE => write!(f, "file"),
            EntryMode::DIR => write!(f, "dir"),
            EntryMode::Unknown => write!(f, "unknown"),
        }
    }
}
