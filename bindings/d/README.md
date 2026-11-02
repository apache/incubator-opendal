# Apache OpenDAL™ D Binding

[![Status: unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/d)

An early-stage D binding for [Apache OpenDAL](https://opendal.apache.org), built
on top of the C binding. Access the local filesystem, memory, S3, and
[50+ other storage services](/services) through one API.

**User guide**: https://opendal.apache.org/docs/bindings/d

## Build

Requirements:

- A D compiler: [dmd, ldc, or gdc](https://dlang.org/download)
- Rust and Cargo (to build the underlying C library)

```bash
# From bindings/d/

# Build libopendal_c and the D library (release mode)
OPENDAL_TEST=memory dub build -b release

# Build and run unit tests against the memory backend
OPENDAL_TEST=memory dub test
```

`OPENDAL_TEST` is read by the build script at build time to compile the named
service into the C library. Omitting it builds only core functionality with no
storage backends. To test against a different service, set `OPENDAL_TEST` and
the matching `OPENDAL_{SERVICE}_*` environment variables:

```bash
OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/test dub test
```

See the [user guide](https://opendal.apache.org/docs/bindings/d) for a
full example and API reference.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, build, and test instructions.
All contributions are welcome — this binding is at an early stage and there is
plenty of room to help.

## License and Trademarks

Licensed under the Apache License, Version 2.0:
http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or
trademarks of the Apache Software Foundation.
