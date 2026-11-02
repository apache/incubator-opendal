# Apache OpenDAL™ Lua Binding (WIP)

[![status: unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/lua)

A Lua binding for [Apache OpenDAL](https://opendal.apache.org/): access the
local filesystem, in-memory storage, S3, and 50+ services through one API.

We release the OpenDAL Lua binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Lua binding version instead of the `opendal` crate
version.

## Useful Links

- **User guide**: https://opendal.apache.org/docs/bindings/lua
- **Services & configuration**: https://opendal.apache.org/docs/category/services/
- **Source**: https://github.com/apache/opendal/tree/main/bindings/lua

## Build and install

### Build from source

```bash
# Currently only Lua 5.2 is supported (the lua52 Cargo feature).
cd bindings/lua
cargo build --package opendal-lua --release

# Copy the native library to your Lua shared-library directory.
cp ../../target/release/libopendal_lua.so /usr/lib/lua/5.2/opendal.so
```

### Install with LuaRocks

```bash
cd bindings/lua
luarocks make
```

## Quickstart

```lua
local opendal = require("opendal")

-- Errors are raised as Lua errors; use pcall to catch them.
local ok, op = pcall(opendal.operator.new, "fs", { root = "/tmp" })
if not ok then
    print(op)
    return
end

op:write("test.txt", "hello world")
print("read:", op:read("test.txt"))   -- "hello world"
```

Run the bundled example:

```bash
lua5.2 example/fs.lua
```

For more examples and full API documentation, see the
[User guide](https://opendal.apache.org/docs/bindings/lua).

## Contributing

Contributions are welcome. Run tests with
[busted](https://lunarmodules.github.io/busted/):

```bash
busted -o gtest test/opendal_test.lua
```

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or
trademarks of the Apache Software Foundation.
