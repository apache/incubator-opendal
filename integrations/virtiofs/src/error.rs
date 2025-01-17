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

use std::io;

use anyhow::Error as AnyError;
use opendal::ErrorKind;
use snafu::prelude::Snafu;

/// Error is a error struct returned by all ovfs functions.
#[derive(Debug, Snafu)]
#[non_exhaustive]
pub enum Error {
    #[snafu(display("IO error: {:?}", source))]
    IOError {
        #[snafu(source(false))]
        source: io::Error,
    },
    #[snafu(display("Unexpected error: {}, source: {:?}", message, source))]
    Unexpected {
        message: String,
        #[snafu(source(false))]
        source: Option<AnyError>,
    },
}

impl From<libc::c_int> for Error {
    fn from(errno: libc::c_int) -> Error {
        Error::IOError {
            source: io::Error::from_raw_os_error(errno),
        }
    }
}

impl From<opendal::Error> for Error {
    fn from(error: opendal::Error) -> Error {
        match error.kind() {
            ErrorKind::Unsupported => libc::ENOTSUP.into(),
            ErrorKind::IsADirectory => libc::EISDIR.into(),
            ErrorKind::NotFound => libc::ENOENT.into(),
            ErrorKind::PermissionDenied => libc::EPERM.into(),
            ErrorKind::AlreadyExists => libc::EEXIST.into(),
            ErrorKind::NotADirectory => libc::ENOTDIR.into(),
            ErrorKind::RangeNotSatisfied => libc::ERANGE.into(),
            ErrorKind::RateLimited => libc::EAGAIN.into(),
            _ => libc::EIO.into(),
        }
    }
}

impl From<Error> for libc::c_int {
    fn from(error: Error) -> libc::c_int {
        match error {
            Error::IOError { source } => source.raw_os_error().unwrap_or(libc::EIO),
            Error::Unexpected { .. } => libc::EIO,
        }
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> io::Error {
        match error {
            Error::IOError { source } => source,
            Error::Unexpected { message, source } => {
                let message = format!("Unexpected error: {}", message);
                match source {
                    Some(source) => io::Error::new(
                        io::ErrorKind::Other,
                        format!("{}, source: {:?}", message, source),
                    ),
                    None => io::Error::new(io::ErrorKind::Other, message),
                }
            }
        }
    }
}

/// Result is a result wrapper in ovfs.
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub fn new_unexpected_error(message: &str, source: Option<AnyError>) -> Error {
    Error::Unexpected {
        message: message.to_string(),
        source,
    }
}
