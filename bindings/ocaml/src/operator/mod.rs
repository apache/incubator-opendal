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

mod _type;
mod capability;
mod entry;
mod lister;
mod metadata;
mod operator_info;
mod reader;
mod writer;

use _type::*;

use super::*;

#[ocaml::func]
#[ocaml::sig("string -> (string * string) list -> (operator, string) Result.t ")]
pub fn operator(
    scheme_str: String,
    map: BTreeMap<String, String>,
) -> Result<ocaml::Pointer<Operator>, String> {
    let op = map_res_error(new_operator(scheme_str, map))?;

    let handle = RUNTIME.handle();
    let _enter = handle.enter();
    let blocking_op = od::blocking::Operator::new(op.clone()).unwrap();

    Ok(Operator(blocking_op).into())
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (entry array, string) Result.t ")]
pub fn blocking_list(
    operator: &mut Operator,
    path: String,
) -> Result<Vec<ocaml::Pointer<Entry>>, String> {
    map_res_error(
        operator
            .0
            .list(path.as_str())
            .map(|m| m.into_iter().map(|it| Entry(it).into()).collect()),
    )
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (lister, string) Result.t ")]
pub fn blocking_lister(
    operator: &mut Operator,
    path: String,
) -> Result<ocaml::Pointer<Lister>, String> {
    map_res_error(operator.0.lister(path.as_str())).map(|op| Lister(op).into())
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (metadata, string) Result.t ")]
pub fn blocking_stat(
    operator: &mut Operator,
    path: String,
) -> Result<ocaml::Pointer<Metadata>, String> {
    map_res_error(operator.0.stat(path.as_str()).map(|m| Metadata(m).into()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (bool, string) Result.t ")]
pub fn blocking_is_exist(operator: &mut Operator, path: String) -> Result<bool, String> {
    map_res_error(operator.0.exists(path.as_str()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (bool, string) Result.t ")]
pub fn blocking_create_dir(operator: &mut Operator, path: String) -> Result<(), String> {
    map_res_error(operator.0.create_dir(path.as_str()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (char array, string) Result.t ")]
pub fn blocking_read(operator: &mut Operator, path: String) -> Result<Vec<u8>, String> {
    map_res_error(operator.0.read(path.as_str()).map(|v| v.to_vec()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (reader, string) Result.t ")]
pub fn blocking_reader(
    operator: &mut Operator,
    path: String,
) -> Result<ocaml::Pointer<Reader>, String> {
    map_res_error(operator.0.reader(path.as_str())).map(|op| Reader(op).into())
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> bytes -> (unit, string) Result.t ")]
pub fn blocking_write(
    operator: &mut Operator,
    path: String,
    bs: &'static [u8],
) -> Result<(), String> {
    map_res_error(operator.0.write(path.as_str(), bs).map(|_| ()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (writer, string) Result.t ")]
pub fn blocking_writer(
    operator: &mut Operator,
    path: String,
) -> Result<ocaml::Pointer<Writer>, String> {
    map_res_error(operator.0.writer(path.as_str())).map(|op| Writer(op).into())
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> string -> (unit, string) Result.t ")]
pub fn blocking_copy(operator: &mut Operator, from: String, to: String) -> Result<(), String> {
    map_res_error(operator.0.copy(from.as_str(), to.as_str())).map(|_| ())
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> string -> (unit, string) Result.t ")]
pub fn blocking_rename(operator: &mut Operator, from: String, to: String) -> Result<(), String> {
    map_res_error(operator.0.rename(from.as_str(), to.as_str()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (unit, string) Result.t ")]
pub fn blocking_delete(operator: &mut Operator, path: String) -> Result<(), String> {
    map_res_error(operator.0.delete(path.as_str()))
}

#[ocaml::func]
#[ocaml::sig("operator -> string array -> (unit, string) Result.t ")]
pub fn blocking_remove(operator: &mut Operator, path: Vec<String>) -> Result<(), String> {
    map_res_error(operator.0.delete_iter(path))
}

#[ocaml::func]
#[ocaml::sig("operator -> string -> (unit, string) Result.t ")]
pub fn blocking_remove_all(operator: &mut Operator, path: String) -> Result<(), String> {
    use opendal::options::DeleteOptions;
    map_res_error(operator.0.delete_options(
        path.as_str(),
        DeleteOptions {
            recursive: true,
            ..Default::default()
        },
    ))
}

#[ocaml::func]
#[ocaml::sig("operator -> (unit, string) Result.t ")]
pub fn blocking_check(operator: &mut Operator) -> Result<(), String> {
    map_res_error(operator.0.check())
}

#[ocaml::func]
#[ocaml::sig("operator -> operator_info ")]
pub fn operator_info(operator: &mut Operator) -> ocaml::Pointer<OperatorInfo> {
    OperatorInfo(operator.0.info()).into()
}

#[ocaml::func]
#[ocaml::sig("operator_info -> capability ")]
pub fn operator_info_capability(info: &mut OperatorInfo) -> ocaml::Pointer<Capability> {
    Capability(info.0.capability()).into()
}
