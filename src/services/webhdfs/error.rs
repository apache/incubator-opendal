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

use http::response::Parts;
use http::Response;
use http::StatusCode;
use serde::Deserialize;

use crate::raw::*;
use crate::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WebHdfsErrorWrapper {
    pub remote_exception: WebHdfsError,
}

/// WebHdfsError is the error message returned by WebHdfs service
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct WebHdfsError {
    exception: String,
    message: String,
    java_class_name: String,
}

pub(super) async fn parse_error(resp: Response<IncomingAsyncBody>) -> Result<Error> {
    let (parts, body) = resp.into_parts();
    let bs = body.bytes().await?;
    let s = String::from_utf8_lossy(&bs);
    parse_error_msg(parts, &s)
}

fn parse_error_msg(parts: Parts, body: &str) -> Result<Error> {
    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::ObjectNotFound, false),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            (ErrorKind::ObjectPermissionDenied, false)
        }
        // passing invalid arguments will return BAD_REQUEST
        // should be unretriable
        StatusCode::BAD_REQUEST => (ErrorKind::Unexpected, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = match serde_json::from_str::<WebHdfsErrorWrapper>(body) {
        Ok(wh_error) => format!("{:?}", wh_error.remote_exception),
        Err(_) => body.to_owned(),
    };

    let mut err = Error::new(kind, &message).with_context("response", format!("{parts:?}"));

    if retryable {
        err = err.set_temporary();
    }

    Ok(err)
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use futures::stream;
    use serde_json::from_reader;

    use super::*;
    use crate::raw::input::Stream;

    /// Error response example from https://hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/WebHDFS.html#Error%20Responses
    #[tokio::test]
    async fn test_parse_error() -> Result<()> {
        let ill_args = bytes::Bytes::from(
            r#"
{
  "RemoteException":
  {
    "exception"    : "IllegalArgumentException",
    "javaClassName": "java.lang.IllegalArgumentException",
    "message"      : "Invalid value for webhdfs parameter \"permission\": ..."
  }
}
    "#,
        );
        let body = IncomingAsyncBody::new(
            Box::new(stream::iter(vec![Ok(ill_args.clone())])) as Box<dyn Stream>,
            None,
        );
        let resp = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(body)
            .unwrap();

        let err = parse_error(resp).await?;
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(!err.is_temporary());

        let err_msg: WebHdfsError = from_reader::<_, WebHdfsErrorWrapper>(ill_args.reader())
            .expect("must success")
            .remote_exception;
        assert_eq!(err_msg.exception, "IllegalArgumentException");
        assert_eq!(
            err_msg.java_class_name,
            "java.lang.IllegalArgumentException"
        );
        assert_eq!(
            err_msg.message,
            "Invalid value for webhdfs parameter \"permission\": ..."
        );

        Ok(())
    }
}
