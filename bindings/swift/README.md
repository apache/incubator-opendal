# Apache OpenDAL™ Swift Binding (WIP)

[![status: unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/swift)

A Swift binding for OpenDAL. Access storage services through one consistent
API backed by the Rust core. **Experimental — not suitable for production use.**

We release the OpenDAL Swift binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Swift binding version instead of the `opendal` crate
version.

## Useful Links

- **User guide**: https://opendal.apache.org/docs/bindings/swift
- **Services & configuration**: https://opendal.apache.org/services

## Build

The Swift binding wraps the C binding. Build the C library before using the package:

```shell
cd bindings/swift
make build-c
```

To verify everything works:

```shell
make test
```

## Installation

The package is not published to any registry. Reference it by local path in
your project's `Package.swift`:

```swift
// swift-tools-version:5.7
import PackageDescription

let package = Package(
    name: "MyTool",
    dependencies: [
        .package(path: "/path/to/opendal/bindings/swift/OpenDAL"),
    ],
    targets: [
        .target(name: "MyTool", dependencies: [
            .product(name: "OpenDAL", package: "OpenDAL"),
        ]),
    ]
)
```

## Quickstart

```swift
import OpenDAL

// Create an operator backed by the in-memory service.
let op = try Operator(scheme: "memory")

// Write bytes to a path.
var data = Data([1, 2, 3, 4])
try op.blockingWrite(&data, to: "/demo")

// Read them back.
let result = try op.blockingRead("/demo")
print(result)
```

Both `blockingWrite` and `blockingRead` throw `OperatorError` on failure.

For a real backend, pass the service scheme and options:

```swift
let op = try Operator(
    scheme: "s3",
    options: [
        "bucket": "my-bucket",
        "region": "us-east-1",
    ]
)
```

See the [user guide](https://opendal.apache.org/docs/bindings/swift) for more
examples, and [Services](https://opendal.apache.org/services) for the full list
of backends and their configuration keys.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to set up the development
environment and submit patches.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or
trademarks of the Apache Software Foundation.
