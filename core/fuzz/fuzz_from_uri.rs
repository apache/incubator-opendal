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

#![no_main]

use libfuzzer_sys::arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use opendal::Operator;
use opendal::OperatorUri;

#[derive(Debug, Clone, Arbitrary)]
struct FuzzInput {
    uri: String,
    options: Vec<(String, String)>,
}

fuzz_target!(|input: FuzzInput| {
    let _ = logforth::starter_log::stderr().try_apply();

    let _ = OperatorUri::new(&input.uri, input.options.clone());
    let _ = Operator::from_uri(input.uri.as_str());
    let _ = Operator::via_iter(&input.uri, input.options);
});
