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

use std::collections::HashMap;
use std::sync::LazyLock;

use ::opendal as od;
use mlua::UserData;
use mlua::prelude::*;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

#[derive(Clone, mlua::FromLua)]
struct ODOperator {
    operator: od::blocking::Operator,
}

impl UserData for ODOperator {}

#[derive(Clone, mlua::FromLua)]
struct ODMetadata {
    metadata: od::Metadata,
}

impl UserData for ODMetadata {}

fn operator_new(lua: &Lua, (scheme, option): (String, LuaTable)) -> LuaResult<LuaTable> {
    if scheme.is_empty() {
        return Err(LuaError::external("scheme is empty"));
    }

    let mut map = HashMap::<String, String>::default();
    for pair in option.pairs::<String, String>() {
        let (key, value) = pair?;
        map.insert(key, value);
    }

    let op = match od::Operator::via_iter(scheme, map) {
        Ok(o) => o,
        Err(e) => return Err(LuaError::external(e)),
    };

    let handle = RUNTIME.handle();
    let _guard = handle.enter();
    let op = od::blocking::Operator::new(op).map_err(LuaError::external)?;

    // this prevents the operator memory from being dropped by the Box
    let op = ODOperator { operator: op };

    let operator = lua.create_table()?;
    operator.set("_operator", op)?;
    operator.set("read", lua.create_function(operator_read)?)?;
    operator.set("write", lua.create_function(operator_write)?)?;
    operator.set("delete", lua.create_function(operator_delete)?)?;
    operator.set("is_exist", lua.create_function(operator_is_exist)?)?;
    operator.set("create_dir", lua.create_function(operator_create_dir)?)?;
    operator.set("rename", lua.create_function(operator_rename)?)?;
    operator.set("stat", lua.create_function(operator_stat)?)?;
    Ok(operator)
}

fn operator_is_exist(_: &Lua, (operator, path): (LuaTable, String)) -> LuaResult<bool> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    if path.is_empty() {
        return Err(LuaError::external("path is empty"));
    }

    let path = path.as_str();
    let res = op.exists(path);
    match res {
        Ok(exist) => Ok(exist),
        Err(e) => Err(LuaError::external(e)),
    }
}

fn operator_delete(_: &Lua, (operator, path): (LuaTable, String)) -> LuaResult<()> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    if path.is_empty() {
        return Err(LuaError::external("path is empty"));
    }

    let path = path.as_str();
    let res = op.delete(path);
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(LuaError::external(e)),
    }
}

fn operator_write(_: &Lua, (operator, path, bytes): (LuaTable, String, String)) -> LuaResult<()> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    if path.is_empty() {
        return Err(LuaError::external("path is empty"));
    }

    let path = path.as_str();
    let res = op.write(path, bytes);
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(LuaError::external(e)),
    }
}

fn operator_create_dir(_: &Lua, (operator, path): (LuaTable, String)) -> LuaResult<()> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    let res = op.create_dir(path.as_str());
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(LuaError::external(e)),
    }
}

fn operator_rename(_: &Lua, (operator, src, dst): (LuaTable, String, String)) -> LuaResult<()> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    let res = op.rename(&src, &dst);
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(LuaError::external(e)),
    }
}

fn operator_read(lua: &Lua, (operator, path): (LuaTable, String)) -> LuaResult<LuaString> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    if path.is_empty() {
        return Err(LuaError::external("path is empty"));
    }

    let path = path.as_str();
    let data = op.read(path);
    match data {
        Ok(data) => Ok(lua.create_string(data.to_vec())?),
        Err(e) => Err(LuaError::external(e)),
    }
}

fn metadata_content_length(_: &Lua, metadata: LuaTable) -> LuaResult<LuaNumber> {
    let metadata = metadata.get::<ODMetadata>("_meta")?;
    let metadata = metadata.metadata;

    Ok(metadata.content_length() as f64)
}

fn metadata_is_file(_: &Lua, metadata: LuaTable) -> LuaResult<bool> {
    let metadata = metadata.get::<ODMetadata>("_meta")?;
    let metadata = metadata.metadata;

    Ok(metadata.is_file())
}

fn metadata_is_dir(_: &Lua, metadata: LuaTable) -> LuaResult<bool> {
    let metadata = metadata.get::<ODMetadata>("_meta")?;
    let metadata = metadata.metadata;

    Ok(metadata.is_dir())
}

fn operator_stat(lua: &Lua, (operator, path): (LuaTable, String)) -> LuaResult<LuaTable> {
    let op = operator.get::<ODOperator>("_operator")?;
    let op = op.operator;

    if path.is_empty() {
        return Err(LuaError::external("path is empty"));
    }

    let path = path.as_str();
    let data = op.stat(path);
    match data {
        Ok(data) => {
            let metadata = lua.create_table()?;
            metadata.set("_meta", ODMetadata { metadata: data })?;
            metadata.set(
                "content_length",
                lua.create_function(metadata_content_length)?,
            )?;
            metadata.set("is_file", lua.create_function(metadata_is_file)?)?;
            metadata.set("is_dir", lua.create_function(metadata_is_dir)?)?;
            Ok(metadata)
        }
        Err(e) => Err(LuaError::external(e)),
    }
}

#[mlua::lua_module]
fn opendal(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    let operator = lua.create_table()?;
    operator.set("new", lua.create_function(operator_new)?)?;

    exports.set("operator", operator)?;

    Ok(exports)
}
