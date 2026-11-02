---
title: Getting started
sidebar_label: Getting started
description: Build the OpenDAL Lua binding, then write, read, and inspect files with a real example.
---

# Getting started

## Prerequisites

Build the native library and install it for your Lua version before running any
of the examples below. See [Overview](./01-overview.md) for the build steps.

## Your first program

This program creates an operator against the in-memory service, writes a file,
reads it back, and checks its metadata. No credentials needed — paste it into a
fresh project and run it as-is.

```lua file=bindings/lua/example/getting-started.lua region=quickstart
```

## Error handling

All methods raise a Lua error on failure (like `error()`). They do **not**
return an `(result, error)` tuple. Use `pcall` or `xpcall` to catch errors:

```lua
local ok, result = pcall(function() return op:read("missing.txt") end)
if not ok then
    print("error:", result)   -- result holds the error message
end
```

On success, each method returns its value directly:
- `read` → `string`
- `write`, `delete`, `create_dir`, `rename` → nothing (void)
- `stat` → metadata table
- `is_exist` → `bool`
- `content_length` → `number`
- `is_file`, `is_dir` → `bool`

## Use a different service

Only the constructor changes; every method is identical across services.

```lua
local opendal = require("opendal")

-- In-memory storage — useful for tests; no credentials or paths needed.
local ok, op = pcall(opendal.operator.new, "memory", {})
if not ok then
    print(op)
    return
end
op:write("key", "value")
print(op:read("key"))   -- "value"
```

For S3, GCS, Azblob, and every other backend, pass the service scheme and its
configuration keys. See [/services](/services) for the full list and the
exact key names each service accepts.

## Available operations

Errors are raised (not returned). Wrap calls in `pcall` when you need to handle failures.

| Method | Signature | Returns on success |
|--------|-----------|---------|
| `op:read(path)` | `(string)` | `string` |
| `op:write(path, bytes)` | `(string, string)` | _(nothing)_ |
| `op:delete(path)` | `(string)` | _(nothing)_ |
| `op:stat(path)` | `(string)` | `metadata` |
| `op:is_exist(path)` | `(string)` | `bool` |
| `op:rename(src, dst)` | `(string, string)` | _(nothing)_ |
| `op:create_dir(path)` | `(string)` | _(nothing)_ |

`metadata` returned by `stat` exposes three methods:

| Method | Returns on success |
|--------|---------|
| `meta:content_length()` | `number` |
| `meta:is_file()` | `bool` |
| `meta:is_dir()` | `bool` |

Paths are relative to the operator's `root`. A trailing `/` denotes a
directory (required for `create_dir`).
