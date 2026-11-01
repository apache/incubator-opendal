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

use foyer::Error as FoyerError;

use opendal_core::Error;
use opendal_core::ErrorKind;

#[derive(Debug)]
pub(crate) enum FetchError {
    SizeTooLarge,
    Source { kind: ErrorKind, message: String },
}

impl FetchError {
    pub(crate) fn from_error(err: Error) -> Self {
        Self::Source {
            kind: err.kind(),
            message: err.to_string(),
        }
    }
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SizeTooLarge => write!(f, "fetched data size exceeds size limit"),
            Self::Source { message, .. } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for FetchError {}

pub(crate) fn extract_err(e: FoyerError) -> Error {
    if let Some(e) = e.downcast_ref::<Error>() {
        return Error::new(e.kind(), e.to_string());
    }
    if let Some(e) = e.downcast_ref::<FetchError>() {
        return match e {
            FetchError::SizeTooLarge => Error::new(
                ErrorKind::Unexpected,
                "fetched data size exceeds size limit",
            ),
            FetchError::Source { kind, message } => Error::new(*kind, message.clone()),
        };
    }
    Error::new(ErrorKind::Unexpected, "foyer operation failed").set_source(e)
}
