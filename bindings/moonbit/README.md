# Apache OpenDAL™ MoonBit Binding

> [!WARNING]
> This binding is experimental and does not expose a public storage API yet.

This initial scaffold connects MoonBit's native target to a binding-local Rust
library through a small C adapter. Its smoke test constructs an OpenDAL memory
operator to verify the complete native link at runtime.

## Requirements

- Rust 1.91 or later.
- A C compiler supported by MoonBit.
- MoonBit compiler `0.10.6+80dc50f24`.

## Development

Build the Rust library from this directory:

```shell
cargo build --release
```

Expose the library to the native linker and loader, then run the smoke test:

```shell
export OPENDAL_MOONBIT_LIB_DIR="$(pwd)/target/release"
export LIBRARY_PATH="$OPENDAL_MOONBIT_LIB_DIR${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="$OPENDAL_MOONBIT_LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export DYLD_LIBRARY_PATH="$OPENDAL_MOONBIT_LIB_DIR${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
moon test --target native --release --deny-warn
```

Check the MoonBit and Rust sources separately:

```shell
moon check --target native --deny-warn
moon fmt --check
moon info --target native
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
```

The scaffold currently supports only the native target. Public operator APIs,
additional services, async operations, WebAssembly, packaging, and release
artifacts will be added separately.
