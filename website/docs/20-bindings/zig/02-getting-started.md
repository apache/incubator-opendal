---
title: Getting started
sidebar_label: Getting started
description: Build an operator, write and read data, inspect metadata, and list a directory with the OpenDAL Zig binding.
---

# Getting started

Before running anything, make sure you have built the binding:

```shell
# From bindings/zig/
zig build libopendal_c
```

## Your first program

The example below initializes an operator backed by the in-memory service, writes a
file, reads it back, checks its metadata, and deletes it. It needs no credentials and
no external service.

```zig file=bindings/zig/examples/getting_started.zig region=quickstart
```

`Operator.init` takes the service scheme as a string and an optional pointer to an
`opendal_operator_options` struct for service-specific configuration. Passing `null`
uses defaults.

## Listing a directory

`list` returns a `Lister`. Call `next` in a loop until it returns `null`.

```zig
var op = try opendal.Operator.init("memory", null);
defer op.deinit();

try op.createDir("/testdir/");
try op.write("/testdir/a.txt", "aaa");
try op.write("/testdir/b.txt", "bbb");

var lister = try op.list("/testdir/");
defer lister.deinit();

while (try lister.next()) |entry| {
    defer entry.deinit(); // Entry owns the underlying C allocation; free it after use.
    std.debug.print("name={s}  path={s}\n", .{ entry.name(), entry.path() });
}
```

## Passing service options

Service-specific configuration is set through the C API's `opendal_operator_options`.
The Zig binding re-exports the raw C namespace as `opendal.c`:

```zig
const c = opendal.c;

const options = c.opendal_operator_options_new();
defer c.opendal_operator_options_free(options);
c.opendal_operator_options_set(options, "root", "/myroot");

// opendal_operator_options_new() returns [*c]..., but Operator.init expects ?*.
// @ptrCast converts the many-item C pointer to a single-item optional pointer.
var op = try opendal.Operator.init("memory", @ptrCast(options));
defer op.deinit();
```

## Error handling

Every operation returns a Zig error union. The binding maps C error codes to named
Zig errors in the `OpendalError` set. The table below shows common variants;
the full set (including `error.IsDirectory`, `error.IsNotDirectory`, `error.RateLimited`,
`error.IsSameFile`, `error.ConditionNotMatch`, `error.RangeNotSatisfied`, and others)
is defined in `src/opendal.zig`.

| Zig error | Meaning |
|---|---|
| `error.NotFound` | Path does not exist |
| `error.PermissionDenied` | Access denied |
| `error.AlreadyExists` | Path already exists |
| `error.Unsupported` | Backend does not support the operation |
| `error.ConfigInvalid` | Operator configuration is invalid |
| `error.Unexpected` | Unclassified error from the backend |

Handle them with a standard `catch` or `try`:

```zig
const data = op.read("/missing.txt") catch |err| {
    if (err == error.NotFound) {
        std.debug.print("file not found\n", .{});
        return;
    }
    return err;
};
```

## Running the binding's own tests

```shell
zig build test --summary all
```

All tests run against the in-memory backend; no credentials are required.
