// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Errors that returned by OpenDAL
//!
//! # Examples
//!
//! ```
//! # use anyhow::Result;
//! # use opendal::ObjectMode;
//! # use opendal::Operator;
//! # use opendal::Scheme;
//! use std::io::ErrorKind;
//! # use opendal::services::fs;
//! # #[tokio::main]
//! # async fn main() -> Result<()> {
//! let op = Operator::from_env(Scheme::Fs)?;
//! if let Err(e) = op.object("test_file").metadata().await {
//!     if e.kind() == ErrorKind::NotFound {
//!         println!("object not exist")
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use std::fmt::Debug;
use std::fmt::{Display, Formatter};
use std::{fmt, io};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// unexpected.
    ///
    /// OpenDAL don't know what happened here, and no actions other than just
    /// returning it back. For example, s3 returns an internal servie error.
    Unexpected,
    Unsupported,

    BackendConfigInvalid,

    /// object is not found.
    ObjectNotFound,
    ObjectPermissionDenied,
    ObjectIsADirectory,
    ObjectNotADirectory,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Unexpected => write!(f, "Unexpected"),
            ErrorKind::Unsupported => write!(f, "Unsupported"),
            ErrorKind::BackendConfigInvalid => write!(f, "BackendConfigInvalid"),
            ErrorKind::ObjectNotFound => write!(f, "ObjectNotFound"),
            ErrorKind::ObjectPermissionDenied => write!(f, "ObjectPermissionDenied"),
            ErrorKind::ObjectIsADirectory => write!(f, "ObjectIsADirectory"),
            ErrorKind::ObjectNotADirectory => write!(f, "ObjectNotADirectory"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorStatus {
    Permenent,
    Temporary,
    Persistent,
}

impl Display for ErrorStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ErrorStatus::Permenent => write!(f, "permenent"),
            ErrorStatus::Temporary => write!(f, "temporary"),
            ErrorStatus::Persistent => write!(f, "persistent"),
        }
    }
}

pub struct Error {
    kind: ErrorKind,
    message: String,

    status: ErrorStatus,
    operation: &'static str,
    context: Vec<(&'static str, String)>,
    source: Option<anyhow::Error>,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}) at {}", self.kind, self.status, self.operation)?;

        if !self.context.is_empty() {
            write!(f, ", context: {{ ")?;
            write!(
                f,
                "{}",
                self.context
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
            write!(f, " }}")?;
        }

        write!(f, " => {}", self.message)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} ({}) at {} => {}",
            self.kind, self.status, self.operation, self.message
        )?;
        if !self.context.is_empty() {
            writeln!(f)?;
            writeln!(f, "Context:")?;
            for (k, v) in self.context.iter() {
                writeln!(f, "    {k}: {v}")?;
            }
        }
        if let Some(source) = &self.source {
            writeln!(f)?;
            writeln!(f, "Source: {:?}", source)?;
        }

        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|v| v.as_ref())
    }
}

impl Error {
    pub fn new(kind: ErrorKind, message: &str) -> Self {
        Self {
            kind,
            message: message.to_string(),

            status: ErrorStatus::Permenent,
            operation: "",
            context: Vec::default(),
            source: None,
        }
    }

    pub fn with_operation(mut self, operation: &'static str) -> Self {
        if !self.operation.is_empty() {
            self.context.push(("called", self.operation.to_string()));
        }

        self.operation = operation;
        self
    }

    pub fn with_context(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.context.push((key, value.into()));
        self
    }

    pub fn with_source(mut self, src: impl Into<anyhow::Error>) -> Self {
        debug_assert!(self.source.is_none());

        self.source = Some(src.into());
        self
    }

    pub fn set_permenent(mut self) -> Self {
        self.status = ErrorStatus::Permenent;
        self
    }
    pub fn set_temporary(mut self) -> Self {
        self.status = ErrorStatus::Temporary;
        self
    }
    pub fn set_persistent(mut self) -> Self {
        self.status = ErrorStatus::Persistent;
        self
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn is_temporary(&self) -> bool {
        self.status == ErrorStatus::Temporary
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        let kind = match err.kind() {
            ErrorKind::ObjectNotFound => io::ErrorKind::NotFound,
            ErrorKind::ObjectPermissionDenied => io::ErrorKind::PermissionDenied,
            _ => io::ErrorKind::Other,
        };

        io::Error::new(kind, err)
    }
}
