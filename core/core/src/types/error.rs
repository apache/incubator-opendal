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

//! OpenDAL error types.
//!
//! # Examples
//!
//! ```
//! # use anyhow::Result;
//! # use opendal_core::EntryMode;
//! # use opendal_core::Operator;
//! use opendal_core::ErrorKind;
//! # async fn test(op: Operator) -> Result<()> {
//! if let Err(e) = op.stat("test_file").await {
//!     if e.kind() == ErrorKind::NotFound {
//!         println!("entry not exist")
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use std::backtrace::Backtrace;
use std::backtrace::BacktraceStatus;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;

/// A specialized [`std::result::Result`] whose default error type is [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// OpenDAL error categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// OpenDAL don't know what happened here, and no actions other than just
    /// returning it back. For example, s3 returns an internal service error.
    Unexpected,
    /// Underlying service doesn't support this operation.
    Unsupported,

    /// The config for this service is invalid.
    ///
    /// OpenDAL also returns this kind for an invalid option value or
    /// combination, for example `if_not_changed` metadata that contains no
    /// usable version or ETag, or a restore `if_not_exists` without a
    /// restore version.
    ConfigInvalid,
    /// The given path is not found.
    NotFound,
    /// The given path doesn't have enough permission for this operation
    PermissionDenied,
    /// The given path is a directory.
    IsADirectory,
    /// The given path is not a directory.
    NotADirectory,
    /// The given path already exists thus we failed to the specified operation on it.
    AlreadyExists,
    /// Requests that sent to this path is over the limit, please slow down.
    RateLimited,
    /// The given file paths are same.
    IsSameFile,
    /// A condition supplied through the OpenDAL operation evaluated to false.
    ///
    /// Options such as `if_match`, `if_not_exists`, and `if_not_changed`
    /// produce this error. It is returned only for conditions the caller
    /// supplied: a service-side conflict unrelated to such a condition
    /// returns [`ErrorKind::Conflict`], and observing a missing file with
    /// `stat` or `read` returns [`ErrorKind::NotFound`] regardless of
    /// conditions.
    /// See the [conditional operation specification][crate::docs::specs::conditional_operations]
    /// for its portable error semantics.
    ConditionNotMatch,
    /// The range of the content is not satisfied.
    ///
    /// OpenDAL returns this error to indicate that the range of the read request is not satisfied.
    RangeNotSatisfied,
    /// The operation conflicts with the current or transitional state of the resource.
    ///
    /// This covers conflicts the service reports on its own; a false
    /// condition supplied through OpenDAL options returns
    /// [`ErrorKind::ConditionNotMatch`] instead.
    ///
    /// This error kind does not indicate whether retrying the same operation is safe.
    /// Inspect the error's retry status for that decision.
    Conflict,
}

impl ErrorKind {
    /// Convert self into static str.
    pub fn into_static(self) -> &'static str {
        self.into()
    }

    /// Capturing a backtrace can be a quite expensive runtime operation.
    /// For some kinds of errors, backtrace is not useful and we can skip it (e.g., check if a file exists).
    ///
    /// See <https://github.com/apache/opendal/discussions/5569>
    fn enable_backtrace(&self) -> bool {
        matches!(self, ErrorKind::Unexpected)
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.into_static())
    }
}

impl From<ErrorKind> for &'static str {
    fn from(v: ErrorKind) -> &'static str {
        match v {
            ErrorKind::Unexpected => "Unexpected",
            ErrorKind::Unsupported => "Unsupported",
            ErrorKind::ConfigInvalid => "ConfigInvalid",
            ErrorKind::NotFound => "NotFound",
            ErrorKind::PermissionDenied => "PermissionDenied",
            ErrorKind::IsADirectory => "IsADirectory",
            ErrorKind::NotADirectory => "NotADirectory",
            ErrorKind::AlreadyExists => "AlreadyExists",
            ErrorKind::RateLimited => "RateLimited",
            ErrorKind::IsSameFile => "IsSameFile",
            ErrorKind::ConditionNotMatch => "ConditionNotMatch",
            ErrorKind::RangeNotSatisfied => "RangeNotSatisfied",
            ErrorKind::Conflict => "Conflict",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorStatus {
    /// The error is stable and will not change without external changes.
    ///
    /// For example, a service returns a not found error.
    ///
    /// Users MUST never retry this operation.
    Permanent,

    /// The error is temporary.
    ///
    /// For example:
    ///
    /// - A service rate-limits a request.
    /// - A service is temporarily unavailable.
    ///
    /// Users MAY retry an operation to resolve this error.
    Temporary,

    /// The error was temporary but persisted after retries.
    ///
    /// For example, a service kept returning network errors.
    ///
    /// Users SHOULD NOT retry this operation, since it's highly possible to error again.
    Persistent,
}

impl Display for ErrorStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ErrorStatus::Permanent => write!(f, "permanent"),
            ErrorStatus::Temporary => write!(f, "temporary"),
            ErrorStatus::Persistent => write!(f, "persistent"),
        }
    }
}

/// Error is the error struct returned by all opendal functions.
///
/// ## Display
///
/// Error can be displayed in two ways:
///
/// - Via `Display`: like `err.to_string()` or `format!("{err}")`
///
/// Error will be printed in a single line:
///
/// ```shell
/// Unexpected, context: { path: /path/to/file, called: send_async } => something wrong happened, source: networking error"
/// ```
///
/// - Via `Debug`: like `format!("{err:?}")`
///
/// Error will be printed in multi lines with more details and backtraces (if captured):
///
/// ```shell
/// Unexpected => something wrong happened
///
/// Context:
///    path: /path/to/file
///    called: send_async
///
/// Source: networking error
///
/// Backtrace:
///    0: opendal::error::Error::new
///              at ./src/error.rs:197:24
///    1: opendal::error::tests::generate_error
///              at ./src/error.rs:241:9
///    2: opendal::error::tests::test_error_debug_with_backtrace::{{closure}}
///              at ./src/error.rs:305:41
///    ...
/// ```
///
/// - For conventional struct-style Debug representation, like `format!("{err:#?}")`:
///
/// ```shell
/// Error {
///     kind: Unexpected,
///     message: "something wrong happened",
///     status: Permanent,
///     operation: "Read",
///     context: [
///         (
///             "path",
///             "/path/to/file",
///         ),
///         (
///             "called",
///             "send_async",
///         ),
///     ],
///     source: Some(
///         "networking error",
///     ),
/// }
/// ```
pub struct Error {
    kind: ErrorKind,
    message: String,

    status: ErrorStatus,
    operation: &'static str,
    context: Vec<(&'static str, String)>,

    source: Option<anyhow::Error>,
    backtrace: Option<Box<Backtrace>>,
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

        if !self.message.is_empty() {
            write!(f, " => {}", self.message)?;
        }

        if let Some(source) = &self.source {
            write!(f, ", source: {source}")?;
        }

        Ok(())
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // If alternate has been specified, we will print like Debug.
        if f.alternate() {
            let mut de = f.debug_struct("Error");
            de.field("kind", &self.kind);
            de.field("message", &self.message);
            de.field("status", &self.status);
            de.field("operation", &self.operation);
            de.field("context", &self.context);
            de.field("source", &self.source);
            return de.finish();
        }

        write!(f, "{} ({}) at {}", self.kind, self.status, self.operation)?;
        if !self.message.is_empty() {
            write!(f, " => {}", self.message)?;
        }
        writeln!(f)?;

        if !self.context.is_empty() {
            writeln!(f)?;
            writeln!(f, "Context:")?;
            for (k, v) in self.context.iter() {
                writeln!(f, "   {k}: {v}")?;
            }
        }
        if let Some(source) = &self.source {
            writeln!(f)?;
            writeln!(f, "Source:")?;
            writeln!(f, "   {source:#}")?;
        }

        if let Some(backtrace) = &self.backtrace {
            writeln!(f)?;
            writeln!(f, "Backtrace:")?;
            writeln!(f, "{backtrace}")?;
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
    /// Create a new Error with error kind and message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),

            status: ErrorStatus::Permanent,
            operation: "",
            context: Vec::default(),
            source: None,

            backtrace: kind
                .enable_backtrace()
                // `Backtrace::capture()` will check if backtrace has
                // been enabled internally. It's zero cost if backtrace
                // is disabled.
                .then(Backtrace::capture)
                // We only keep captured backtrace to avoid an extra box.
                .filter(|bt| bt.status() == BacktraceStatus::Captured)
                .map(Box::new),
        }
    }

    /// Update error's operation.
    ///
    /// # Notes
    ///
    /// If the error already carries an operation, we will push a new context
    /// `(called, operation)`.
    pub fn with_operation(mut self, operation: impl Into<&'static str>) -> Self {
        if !self.operation.is_empty() {
            self.context.push(("called", self.operation.to_string()));
        }

        self.operation = operation.into();
        self
    }

    /// Add more context in error.
    pub fn with_context(mut self, key: &'static str, value: impl ToString) -> Self {
        self.context.push((key, value.to_string()));
        self
    }

    /// Set source for error.
    ///
    /// # Notes
    ///
    /// If the source has been set, we will raise a panic here.
    pub fn set_source(mut self, src: impl Into<anyhow::Error>) -> Self {
        debug_assert!(self.source.is_none(), "the source error has been set");

        self.source = Some(src.into());
        self
    }

    /// Set permanent status for error.
    ///
    /// By set permanent, we indicate the retry must be stopped.
    pub fn set_permanent(mut self) -> Self {
        self.status = ErrorStatus::Permanent;
        self
    }

    /// Set temporary status for error.
    ///
    /// A temporary error indicates that the receiving OpenDAL layer can safely replay
    /// the same operation method with the state it retains. It does not imply that a
    /// broader workflow can be retried without refreshing or rebuilding its state.
    pub fn set_temporary(mut self) -> Self {
        self.status = ErrorStatus::Temporary;
        self
    }

    /// Set persistent status for error.
    ///
    /// By setting persistent, we indicate the retry should be stopped.
    pub fn set_persistent(mut self) -> Self {
        self.status = ErrorStatus::Persistent;
        self
    }

    /// Set permanent status for error by given permanent.
    ///
    /// By set permanent, we indicate the retry must be stopped.
    pub fn with_permanent(mut self, permanent: bool) -> Self {
        if permanent {
            self.status = ErrorStatus::Permanent;
        }
        self
    }

    /// Set temporary status for error by given temporary.
    ///
    /// By set temporary, we indicate this error is retryable.
    pub fn with_temporary(mut self, temporary: bool) -> Self {
        if temporary {
            self.status = ErrorStatus::Temporary;
        }
        self
    }

    /// Set persistent status for error by given persistent.
    ///
    /// By set persistent, we indicate the retry should be stopped.
    pub fn with_persistent(mut self, persistent: bool) -> Self {
        if persistent {
            self.status = ErrorStatus::Persistent;
        }
        self
    }

    /// Return error's kind.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Check if this error is permanent.
    pub fn is_permanent(&self) -> bool {
        self.status == ErrorStatus::Permanent
    }

    /// Check if the receiving OpenDAL layer can safely replay the same operation method.
    pub fn is_temporary(&self) -> bool {
        self.status == ErrorStatus::Temporary
    }

    /// Check if this error is persistent.
    pub fn is_persistent(&self) -> bool {
        self.status == ErrorStatus::Persistent
    }

    /// Return error's message.
    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    /// Return error's backtrace.
    ///
    /// Note: the standard way of exposing backtrace is the unstable feature [`error_generic_member_access`](https://github.com/rust-lang/rust/issues/99301).
    /// We don't provide it as it requires nightly rust.
    ///
    /// If you just want to print error with backtrace, use `Debug`, like `format!("{err:?}")`.
    ///
    /// If you use nightly rust, and want to access `opendal::Error`'s backtrace in the standard way, you can
    /// implement a newtype like this:
    ///
    /// ```ignore
    /// // assume you already have `#![feature(error_generic_member_access)]` on the top of your crate
    ///
    /// #[derive(::std::fmt::Debug)]
    /// pub struct OpendalError(opendal::Error);
    ///
    /// impl std::fmt::Display for OpendalError {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         self.0.fmt(f)
    ///     }
    /// }
    ///
    /// impl std::error::Error for OpendalError {
    ///     fn provide<'a>(&'a self, request: &mut std::error::Request<'a>) {
    ///         if let Some(bt) = self.0.backtrace() {
    ///             request.provide_ref::<std::backtrace::Backtrace>(bt);
    ///         }
    ///     }
    ///
    ///     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    ///         self.0.source()
    ///     }
    /// }
    /// ```
    ///
    /// Additionally, you can add a clippy lint to prevent usage of the original `opendal::Error` type.
    /// ```toml
    /// disallowed-types = [
    ///     { path = "opendal::Error", reason = "Please use `my_crate::OpendalError` instead." },
    /// ]
    /// ```
    pub fn backtrace(&self) -> Option<&Backtrace> {
        self.backtrace.as_ref().map(|bt| bt.as_ref())
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> Self {
        let kind = match err.kind() {
            ErrorKind::NotFound => io::ErrorKind::NotFound,
            ErrorKind::PermissionDenied => io::ErrorKind::PermissionDenied,
            _ => io::ErrorKind::Other,
        };

        io::Error::new(kind, err)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use anyhow::anyhow;
    use pretty_assertions::assert_eq;

    use super::*;

    static TEST_ERROR: LazyLock<Error> = LazyLock::new(|| Error {
        kind: ErrorKind::Unexpected,
        message: "something wrong happened".to_string(),
        status: ErrorStatus::Permanent,
        operation: "Read",
        context: vec![
            ("path", "/path/to/file".to_string()),
            ("called", "send_async".to_string()),
        ],
        source: Some(anyhow!("networking error")),
        backtrace: None,
    });

    #[test]
    fn test_error_display() {
        let s = format!("{}", LazyLock::force(&TEST_ERROR));
        assert_eq!(
            s,
            r#"Unexpected (permanent) at Read, context: { path: /path/to/file, called: send_async } => something wrong happened, source: networking error"#
        );
        println!("{:#?}", LazyLock::force(&TEST_ERROR));
    }

    #[test]
    fn test_error_debug() {
        let s = format!("{:?}", LazyLock::force(&TEST_ERROR));
        assert_eq!(
            s,
            r#"Unexpected (permanent) at Read => something wrong happened

Context:
   path: /path/to/file
   called: send_async

Source:
   networking error
"#
        )
    }
}
