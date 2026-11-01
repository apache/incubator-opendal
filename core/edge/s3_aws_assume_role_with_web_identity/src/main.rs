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

use logforth::append::Testing;
use logforth::filter::rustlog::RustLogFilterBuilder;
use logforth::layout::TextLayout;
use opendal::tests::init_test_service;

#[tokio::main]
async fn main() {
    let _ = logforth::starter_log::builder()
        .dispatch(|d| {
            d.filter(RustLogFilterBuilder::from_default_env().build())
                .append(Testing::default().with_layout(TextLayout::default()))
        })
        .try_apply();

    let op = init_test_service()
        .unwrap_or_else(|err| panic!("failed to init test service: {:?}", err))
        .expect("service must be init");
    assert_eq!(op.info().scheme(), opendal::services::S3_SCHEME);

    let result = op
        .exists(&uuid::Uuid::new_v4().to_string())
        .await
        .expect("this operation should never return error");
    assert!(!result, "the file must be not exist");
}
