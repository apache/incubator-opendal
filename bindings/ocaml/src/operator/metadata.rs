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

use super::*;

#[ocaml::func]
#[ocaml::sig("metadata -> bool ")]
pub fn metadata_is_file(metadata: &mut Metadata) -> bool {
    metadata.0.is_file()
}

#[ocaml::func]
#[ocaml::sig("metadata -> bool ")]
pub fn metadata_is_dir(metadata: &mut Metadata) -> bool {
    metadata.0.is_dir()
}

#[ocaml::func]
#[ocaml::sig("metadata -> int64 ")]
pub fn metadata_content_length(metadata: &mut Metadata) -> u64 {
    metadata.0.content_length()
}

#[ocaml::func]
#[ocaml::sig("metadata -> string option ")]
pub fn metadata_content_md5(metadata: &mut Metadata) -> Option<String> {
    metadata.0.content_md5().map(String::from)
}

#[ocaml::func]
#[ocaml::sig("metadata -> string option ")]
pub fn metadata_content_type(metadata: &mut Metadata) -> Option<String> {
    metadata.0.content_type().map(String::from)
}

#[ocaml::func]
#[ocaml::sig("metadata -> string option ")]
pub fn metadata_content_disposition(metadata: &mut Metadata) -> Option<String> {
    metadata.0.content_disposition().map(String::from)
}

#[ocaml::func]
#[ocaml::sig("metadata -> string option ")]
pub fn metadata_etag(metadata: &mut Metadata) -> Option<String> {
    metadata.0.etag().map(String::from)
}

#[ocaml::func]
#[ocaml::sig("metadata -> int64 option ")]
pub fn metadata_last_modified(metadata: &mut Metadata) -> Option<i64> {
    metadata
        .0
        .last_modified()
        .map(|t| t.into_inner().as_second())
}
