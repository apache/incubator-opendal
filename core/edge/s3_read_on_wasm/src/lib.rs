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

use opendal::Operator;
use opendal::services::S3;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn hello_world() -> String {
    opendal::install_default();

    let cfg = S3::default()
        .endpoint("http://127.0.0.1:9000")
        .access_key_id("minioadmin")
        .secret_access_key("minioadmin")
        .bucket("test")
        .region("us-east-1");

    let op = Operator::new(cfg).unwrap();
    op.write(
        "test",
        "Hello, WASM! We are from OpenDAL at rust side!"
            .as_bytes()
            .to_vec(),
    )
    .await
    .unwrap();
    let bs = op.read("test").await.unwrap().to_bytes();
    String::from_utf8_lossy(&bs).to_string()
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;
    use wasm_bindgen_test::wasm_bindgen_test_configure;

    use super::*;

    wasm_bindgen_test_configure!(run_in_browser);

    /// This test is executed in the wasm target but rustc reports `dead_code` in wrong.
    #[allow(dead_code)]
    #[wasm_bindgen_test]
    async fn test_hello_world() {
        let s = hello_world().await;
        assert_eq!(s, "Hello, WASM! We are from OpenDAL at rust side!")
    }
}
