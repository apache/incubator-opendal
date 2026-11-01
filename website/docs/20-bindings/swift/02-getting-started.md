---
title: Getting started
sidebar_label: Getting started
description: Create an OpenDAL Swift operator, write data, and read it back.
---

# Getting started

:::warning Experimental
The Swift binding is unreleased and experimental. Build from source before
following this guide — see [Overview](./01-overview.md) for build steps.
:::

## Your first program

This program creates an in-memory operator, writes bytes to a path, and reads
them back. No credentials or network access needed.

```swift file=bindings/swift/OpenDAL/Sources/GettingStarted/main.swift region=quickstart
```

`Operator.init(scheme:options:)` throws `OperatorError` on failure. Both
`blockingWrite` and `blockingRead` also throw `OperatorError`; wrap calls in
`do/catch` for production code.

## Passing options

Service configuration is passed as a `[String: String]` dictionary. For
example, to pin a root path on the memory service:

```swift
import OpenDAL

let op = try Operator(
    scheme: "memory",
    options: ["root": "/myroot"]
)

var data = Data("hello".utf8)
try op.blockingWrite(&data, to: "file.txt")

let bytes = try op.blockingRead("file.txt")
print(String(data: bytes, encoding: .utf8)!) // hello
```

## Pointing at a real backend

Only the scheme and options change; the write/read calls are identical. The
keys for each service are listed on [/services](/services).

```swift
import OpenDAL

// S3 example — fill in real values.
let op = try Operator(
    scheme: "s3",
    options: [
        "bucket": "my-bucket",
        "region": "us-east-1",
        "access_key_id": "...",
        "secret_access_key": "...",
    ]
)

var payload = Data("hello from S3".utf8)
try op.blockingWrite(&payload, to: "hello.txt")
```

## Error handling

Both `Operator.init` and the blocking operations throw `OperatorError`:

```swift
import OpenDAL

do {
    let op = try Operator(scheme: "memory")
    let _ = try op.blockingRead("/does-not-exist")
} catch let err as OperatorError {
    print("OpenDAL operation failed: \(err)")
} catch {
    print("unexpected error: \(error)")
}
```

> **Note:** `OperatorError` has `code` (UInt32) and `message` (Data) fields
> internally, but they are not declared `public` and cannot be accessed from
> consuming packages. Until they are exposed, use the error's string
> representation via `\(err)` or `err.localizedDescription`.
