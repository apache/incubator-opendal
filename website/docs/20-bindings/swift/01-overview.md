---
title: Swift
sidebar_label: Overview
slug: /bindings/swift
description: The OpenDAL Swift binding — experimental status, what it supports, how to build it, and where to go next.
---

# Apache OpenDAL™ Swift Binding

A Swift binding for OpenDAL: access storage services through one API, backed
by the Rust core.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::warning Experimental / WIP
This binding is unreleased and under active development. The API surface is
minimal, breaking changes may happen without notice, and it is **not suitable
for production use**. Contributions welcome.
:::

## Status

**Experimental — unreleased.** Only blocking read and write are implemented.
The binding depends on the C binding and must be built from source; it is not
published to any package registry.

## Capabilities

| Capability | Status |
|---|---|
| Blocking read | Supported |
| Blocking write | Supported |
| Async read/write | Not yet |
| Stat / metadata | Not yet |
| List | Not yet |
| Delete | Not yet |
| Presigned URLs | Not yet |

The `Operator` class is the single entry point. It accepts a service scheme
string and a dictionary of string options, matching the same model as every
other OpenDAL binding. See [Services](/services) for the full list of backends
and their configuration keys.

## Installation

The package is **not published to any registry**. You must build the C
dependency and reference the package by local path.

### 1. Build the C dependency

```shell
cd bindings/swift
make build-c
```

### 2. Add a local path dependency

In your project's `Package.swift`:

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

Replace `/path/to/opendal` with the absolute path to your local checkout.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/swift](https://opendal.apache.org/docs/bindings/swift)
- **Services & configuration**: [/services](/services)
- **Source**: [`bindings/swift/`](https://github.com/apache/opendal/tree/main/bindings/swift)
- **Contributing**: [`bindings/swift/CONTRIBUTING.md`](https://github.com/apache/opendal/blob/main/bindings/swift/CONTRIBUTING.md)

## Next steps

1. [Getting started](./02-getting-started.md) — create an operator, write data, read it back.
