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

/// Identifies the operation being performed.
///
/// Most variants map to methods on the [`Service`][crate::raw::Service] trait,
/// although some names differ to improve readability.
///
/// An operation might have different meanings and costs depending on a
/// storage service.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
#[non_exhaustive]
pub enum Operation {
    /// Operation to retrieve information about the specified storage services.
    #[default]
    Info,
    /// Operation to create a directory.
    CreateDir,
    /// Operation to read a file.
    Read,
    /// Operation to write to a file.
    Write,
    /// Operation to copy a file.
    Copy,
    /// Operation to compose multiple files into one file.
    Compose,
    /// Operation to rename a file.
    Rename,
    /// Operation to restore a file.
    Restore,
    /// Operation to stat a file or a directory.
    Stat,
    /// Operation to delete files.
    Delete,
    /// Operation to get the next file from the list.
    List,
    /// Operation to generate a presigned URL.
    Presign,
}

impl Operation {
    /// Convert self into static str.
    pub fn into_static(self) -> &'static str {
        self.into()
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.into_static())
    }
}

impl From<Operation> for &'static str {
    fn from(v: Operation) -> &'static str {
        match v {
            Operation::Info => "info",
            Operation::CreateDir => "create_dir",
            Operation::Read => "read",
            Operation::Write => "write",
            Operation::Copy => "copy",
            Operation::Compose => "compose",
            Operation::Rename => "rename",
            Operation::Restore => "restore",
            Operation::Stat => "stat",
            Operation::Delete => "delete",
            Operation::List => "list",
            Operation::Presign => "presign",
        }
    }
}

impl From<Operation> for String {
    fn from(v: Operation) -> Self {
        v.into_static().to_string()
    }
}

/// A service-specific operation name attached to HTTP requests.
///
/// While [`Operation`] describes the OpenDAL-level operation (e.g., read, write, list),
/// `ServiceOperation` describes the specific backend API call being made as a supplement.
///
/// Services attach this as an HTTP request extension so that observability layers
/// can provide finer-grained breakdowns of HTTP traffic.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ServiceOperation(pub &'static str);
