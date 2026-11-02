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

use crate::*;

/// Parse std io error into opendal::Error.
///
/// # TODO
///
/// Add `NotADirectory` and `IsADirectory` once they are stable.
///
/// ref: <https://github.com/rust-lang/rust/issues/86442>
pub fn new_std_io_error(err: io::Error) -> Error {
    use std::io::ErrorKind::*;

    let (kind, retryable) = match err.kind() {
        NotFound => (ErrorKind::NotFound, false),
        PermissionDenied => (ErrorKind::PermissionDenied, false),
        AlreadyExists => (ErrorKind::AlreadyExists, false),
        Unsupported => (ErrorKind::Unsupported, false),

        Interrupted | UnexpectedEof | TimedOut | WouldBlock => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, true),
    };

    let mut err = Error::new(kind, err.kind().to_string()).set_source(err);

    if retryable {
        err = err.set_temporary();
    }

    err
}

/// helper functions to format `Error` into `io::Error`.
///
/// This function is added privately by design and only valid in current
/// context (i.e. `raw` mod). We don't want to expose this function to
/// users.
#[inline]
pub(crate) fn format_std_io_error(err: Error) -> io::Error {
    let kind = match err.kind() {
        ErrorKind::NotFound => io::ErrorKind::NotFound,
        ErrorKind::PermissionDenied => io::ErrorKind::PermissionDenied,
        _ => io::ErrorKind::Other,
    };

    io::Error::new(kind, err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_to_end_stops_on_persistent_error() {
        struct ErrorThenEof {
            calls: usize,
        }

        impl io::Read for ErrorThenEof {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls == 1 {
                    return Err(format_std_io_error(
                        Error::new(ErrorKind::Unexpected, "retry exhausted").set_persistent(),
                    ));
                }
                Ok(0)
            }
        }

        let mut reader = ErrorThenEof { calls: 0 };
        let mut buf = Vec::new();
        assert!(
            io::Read::read_to_end(&mut reader, &mut buf).is_err(),
            "calls: {}",
            reader.calls
        );
    }
}
