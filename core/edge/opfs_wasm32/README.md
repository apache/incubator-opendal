# OPFS on WASM

This test verifies the OpenDAL OPFS service works in a browser environment.

## Install

```shell
cargo install wasm-pack
```

## Build

```shell
wasm-pack build
```

## Test

NOTE:

- You need to have Chrome installed.
- OPFS requires a browser context (no Node.js support).
- Headless Chrome may not work for OPFS tests.

Some code execution path differ whether it's running in the main thread or in a web worker, use the 2 following commands to test both cases:

### Main thread
```shell
wasm-pack test --chrome
```

### Web worker

To run the same tests in a dedicated web worker:

```shell
wasm-pack test --chrome -- --features worker
```
