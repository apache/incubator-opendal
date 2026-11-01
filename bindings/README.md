# OpenDAL Bindings

This folder contains the bindings for OpenDAL. Currently, we support the following bindings:

### Released Bindings

* [Java](java/README.md)
* [Node.js](nodejs/README.md)
* [Python](python/README.md)

### Unreleased Bindings

* [C](c/README.md)
* [C++](cpp/README.md)
* [C#](dotnet/README.md)
* [D](d/README.md)
* [Dart](dart/README.md)
* [.NET](dotnet/README.md)
* [Go](go/README.md)
* [Haskell](haskell/README.md)
* [Lua](lua/README.md)
* [OCaml](ocaml/README.md)
* [PHP](php/README.md)
* [Ruby](ruby/README.md)
* [Swift](swift/README.md)
* [Zig](zig/README.md)

## Versioning

We release each binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). This allows
bindings to follow their own release cycles and compatibility requirements.

For example, while the `opendal` crate might be at version `0.55.0`, a binding
might be at version `0.47.0` or `0.49.2`. For updates and compatibility, use
the specific binding version instead of the `opendal` crate version.

## Getting Started

Every binding should provide a `README.md` file to help users get started.
The `README.md` file should contain the following sections:

* **Installation**: how to install the binding.
* **Usage**: how to use the binding.
* **Development**: how to develop the binding.
* **Testing**: how to test the binding.

You can find the `README.md` file for each binding in the corresponding folder.

Please refer to the bindings listed above or which are already released for more details.

## Contributing

We welcome contributions to OpenDAL. Please refer to [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributing guidelines.
