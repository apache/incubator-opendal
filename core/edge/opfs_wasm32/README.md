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

```shell
wasm-pack test --chrome
```
