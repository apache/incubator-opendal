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
#[ocaml::sig("capability -> bool ")]
pub fn capability_stat(cap: &Capability) -> bool {
    cap.0.stat
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_read(cap: &Capability) -> bool {
    cap.0.read
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_write(cap: &Capability) -> bool {
    cap.0.write
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_create_dir(cap: &Capability) -> bool {
    cap.0.create_dir
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_delete(cap: &Capability) -> bool {
    cap.0.delete
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_copy(cap: &Capability) -> bool {
    cap.0.copy
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_rename(cap: &Capability) -> bool {
    cap.0.rename
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_list(cap: &Capability) -> bool {
    cap.0.list
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_list_with_limit(cap: &Capability) -> bool {
    cap.0.list_with_limit
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_list_with_start_after(cap: &Capability) -> bool {
    cap.0.list_with_start_after
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_list_with_recursive(cap: &Capability) -> bool {
    cap.0.list_with_recursive
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_presign(cap: &Capability) -> bool {
    cap.0.presign
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_presign_read(cap: &Capability) -> bool {
    cap.0.presign_read
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_presign_stat(cap: &Capability) -> bool {
    cap.0.presign_stat
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_presign_write(cap: &Capability) -> bool {
    cap.0.presign_write
}

#[ocaml::func]
#[ocaml::sig("capability -> bool ")]
pub fn capability_shared(cap: &Capability) -> bool {
    cap.0.shared
}
