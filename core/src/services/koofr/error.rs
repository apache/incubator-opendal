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

use bytes::Bytes;

use crate::raw::*;
use crate::Error;
use crate::ErrorKind;
use crate::Result;

/// Parse error response into Error.
pub fn parse_error(parts: http::response::Parts, bs: Bytes) -> Result<Error> {
    let (kind, retryable) = match parts.status.as_u16() {
        403 => (ErrorKind::PermissionDenied, false),
        404 => (ErrorKind::NotFound, false),
        304 | 412 => (ErrorKind::ConditionNotMatch, false),
        // Service like Koofr could return 499 error with a message like:
        // Client Disconnect, we should retry it.
        499 => (ErrorKind::Unexpected, true),
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = String::from_utf8_lossy(&bs).into_owned();

    let mut err = Error::new(kind, &message);

    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    Ok(err)
}

#[cfg(test)]
mod test {
    use http::Response;
    use http::StatusCode;

    use super::*;

    #[tokio::test]
    async fn test_parse_error() {
        let err_res = vec![(r#""#, ErrorKind::NotFound, StatusCode::NOT_FOUND)];

        for res in err_res {
            let bs = bytes::Bytes::from(res.0);
            let (parts, bs) = Response::builder()
                .status(res.2)
                .body(bs)
                .unwrap()
                .into_parts();

            let err = parse_error(parts, bs);

            assert!(err.is_ok());
            assert_eq!(err.unwrap().kind(), res.1);
        }
    }
}
