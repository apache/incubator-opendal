# Apache OpenDAL™ Zig Binding

[![Status: Unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/zig)

A Zig binding for [Apache OpenDAL](https://opendal.apache.org) — blocking access to
S3, GCS, Azure Blob, the local filesystem, and 50+ more storage services through one
API, backed by the Rust core.

> **Early-stage / Experimental.** The API is a work in progress and may change
> between releases. There are no pre-built packages; you must build from source.

## User Guide

Full documentation is at
**<https://opendal.apache.org/docs/bindings/zig>**

The guide covers installation, building, operator construction, read/write/stat/delete,
listing directories, and error handling.

## Build

Requirements: Zig 0.14.0 or higher, a C/C++ compiler, CMake, and a Rust toolchain.

```shell
# From bindings/zig/

# Build the underlying C library
zig build libopendal_c

# Build and run the tests
zig build test --summary all
```

The build pulls the C binding header from `../c/include/opendal.h` and links against
`libopendal_c` produced by the C binding's CMake build.

## Quick example

```zig
const opendal = @import("opendal");

var op = try opendal.Operator.init("memory", null);
defer op.deinit();

try op.write("/hello.txt", "Hello, World!");
const data = try op.read("/hello.txt");
```

See the [User Guide](https://opendal.apache.org/docs/bindings/zig) for more.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for local setup, build steps, and how to run
the test suite.

## License and Trademarks

Licensed under the Apache License, Version 2.0:
<http://www.apache.org/licenses/LICENSE-2.0>

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of
the Apache Software Foundation.
