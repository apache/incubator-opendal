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

use std::fmt::Debug;

use bytes::Bytes;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

use opendal_core::*;

/// response data from d1
#[derive(Deserialize, Debug)]
pub struct D1Response {
    pub result: Vec<D1Result>,
    pub success: bool,
    pub errors: Vec<D1Error>,
}

impl D1Response {
    pub fn parse(bs: &Bytes) -> Result<D1Response, Error> {
        let response: D1Response = serde_json::from_slice(bs).map_err(|e| {
            Error::new(
                opendal_core::ErrorKind::Unexpected,
                format!("failed to parse error response: {e}"),
            )
        })?;

        if !response.success {
            return Err(Error::new(
                opendal_core::ErrorKind::Unexpected,
                String::from_utf8_lossy(bs),
            ));
        }
        Ok(response)
    }

    pub fn get_result(&self, key: &str) -> Option<Buffer> {
        if self.result.is_empty() || self.result[0].results.is_empty() {
            return None;
        }
        let result = &self.result[0].results[0];
        let value = result.get(key);

        match value {
            Some(Value::Array(s)) => {
                let mut v = Vec::new();
                for i in s {
                    if let Value::Number(n) = i {
                        v.push(n.as_u64().unwrap() as u8);
                    }
                }
                Some(Buffer::from(v))
            }
            _ => None,
        }
    }

    pub fn get_usize_result(&self, key: &str) -> Result<Option<usize>, Error> {
        if self.result.is_empty() || self.result[0].results.is_empty() {
            return Ok(None);
        }
        let result = &self.result[0].results[0];
        let Some(value) = result.get(key) else {
            return Ok(None);
        };

        match value {
            Value::Number(n) => {
                let value = n.as_u64().ok_or_else(|| {
                    Error::new(
                        opendal_core::ErrorKind::Unexpected,
                        "d1 value length is invalid",
                    )
                })?;
                value.try_into().map(Some).map_err(|err| {
                    Error::new(
                        opendal_core::ErrorKind::Unexpected,
                        "d1 value length is invalid",
                    )
                    .set_source(err)
                })
            }
            _ => Err(Error::new(
                opendal_core::ErrorKind::Unexpected,
                "d1 value length is invalid",
            )),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct D1Result {
    pub results: Vec<Map<String, Value>>,
}

#[derive(Clone, Deserialize, Debug, Serialize)]
pub struct D1Error {
    pub message: String,
    pub code: i32,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deserialize_get_object_json_response() {
        let data = r#"
        {
            "result": [
                {
                    "results": [
                        {
                            "CustomerId": "4",
                            "CompanyName": "Around the Horn",
                            "ContactName": "Thomas Hardy"
                        }
                    ],
                    "success": true,
                    "meta": {
                        "served_by": "v3-prod",
                        "duration": 0.2147,
                        "changes": 0,
                        "last_row_id": 0,
                        "changed_db": false,
                        "size_after": 2162688,
                        "rows_read": 3,
                        "rows_written": 2
                    }
                }
            ],
            "success": true,
            "errors": [],
            "messages": []
        }"#;
        let response: D1Response = serde_json::from_str(data).unwrap();
        println!("{:?}", response.result[0].results[0]);
    }

    #[test]
    fn test_get_usize_result() {
        let data = r#"
        {
            "result": [
                {
                    "results": [
                        {
                            "content_length": 6
                        }
                    ],
                    "success": true,
                    "meta": {}
                }
            ],
            "success": true,
            "errors": [],
            "messages": []
        }"#;

        let response: D1Response = serde_json::from_str(data).unwrap();
        assert_eq!(
            response.get_usize_result("content_length").unwrap(),
            Some(6)
        );
        assert_eq!(response.get_usize_result("missing").unwrap(), None);
    }
}
