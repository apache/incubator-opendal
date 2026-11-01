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

// For ocaml-rs, the build order in the same build group is the order of the file names.
// In order to use the type in the function in the generated ocaml file, it must be defined on the first generated file.
use super::*;

#[ocaml::sig]
pub struct Operator(pub(crate) od::blocking::Operator);
ocaml::custom!(Operator);

#[ocaml::sig]
pub struct Reader(pub(crate) od::blocking::Reader);
ocaml::custom!(Reader);

#[ocaml::sig]
pub struct Writer(pub(crate) od::blocking::Writer);
ocaml::custom!(Writer);

#[ocaml::sig]
pub struct Lister(pub(crate) od::blocking::Lister);
ocaml::custom!(Lister);

#[ocaml::sig]
pub struct Metadata(pub(crate) od::Metadata);
ocaml::custom!(Metadata);

#[ocaml::sig]
pub struct Entry(pub(crate) od::Entry);
ocaml::custom!(Entry);

#[ocaml::sig]
pub struct OperatorInfo(pub(crate) od::OperatorInfo);
ocaml::custom!(OperatorInfo);

#[ocaml::sig]
pub struct Capability(pub(crate) od::Capability);
ocaml::custom!(Capability);

#[ocaml::sig]
#[allow(dead_code)]
pub struct PresignedRequest(pub(crate) od::raw::PresignedRequest);
ocaml::custom!(PresignedRequest);
