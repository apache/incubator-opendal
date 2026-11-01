# Apache OpenDAL™ Dart Binding (WIP)

A Dart binding for [Apache OpenDAL](https://opendal.apache.org/): access S3,
GCS, Azure Blob, the local filesystem, and 50+ more services through one API.
Built on the Rust core via [flutter_rust_bridge](https://github.com/fzyzcjy/flutter_rust_bridge).

We release the OpenDAL Dart binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Dart binding version instead of the `opendal` crate
version.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/dart](https://opendal.apache.org/docs/bindings/dart)
- **Examples**: [`examples/`](./examples)
- **Services & configuration**: [opendal.apache.org/services](https://opendal.apache.org/services)
- **Source**: [`bindings/dart/`](https://github.com/apache/opendal/tree/main/bindings/dart)

## Installation

This binding is not yet published to pub.dev. To use it, clone the repository
and add it as a path dependency, or work directly from `bindings/dart/`.

You must compile the Rust native library first:

```shell
cd rust
cargo build -r
cd ..
dart pub get
```

The loader first looks for the library in the platform-specific target-triple
directory (e.g. `rust/target/aarch64-apple-darwin/release/`), then falls back
to `rust/target/release/`. For cross-compiled targets, build with
`--target <triple>` so the library is placed in the matching target directory.

## Quickstart

```dart
import 'dart:typed_data';
import 'package:opendal/opendal.dart';

void main() async {
  final storage = await Storage.init(schemeStr: "memory", map: {"root": "/"});
  final File = storage.initFile();

  final file = File("hello.txt");
  await file.write(Uint8List.fromList("Hello, OpenDAL!".codeUnits));

  final data = await file.read();
  print(String.fromCharCodes(data)); // Hello, OpenDAL!

  await file.delete();
}
```

The API mirrors `dart:io` — `Storage.initFile()` and `Storage.initDir()` return
factory functions that behave like `dart:io`'s `File` and `Directory`.

Every method has a blocking `*Sync` variant alongside the async `Future<T>` form.

See the [user guide](https://opendal.apache.org/docs/bindings/dart) for more
examples and a full operations reference.

## Contributing

Run tests after building the native library:

```shell
dart test
```

Update generated code when upgrading `flutter_rust_bridge`:

1. Install the matching `flutter_rust_bridge_codegen` version.
2. Update `flutter_rust_bridge` in both `pubspec.yaml` and `rust/Cargo.toml`.
3. Run `flutter_rust_bridge_codegen generate`.
4. Run `cargo build -r` from `rust/`.
5. Run `dart test` to verify.

The codegen version recorded in generated files, `lib/src/rust/frb_generated.dart`,
and `rust/src/frb_generated.rs` must all match. Version drift causes startup
or test failures.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or
trademarks of the Apache Software Foundation.
