# Apache OpenDAL™ Haskell Binding (WIP)

[![](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/haskell)

We release the OpenDAL Haskell binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Haskell binding version instead of the `opendal`
crate version.

Access S3, GCS, Azure Blob, the local filesystem, and 50+ more services from
Haskell through a single, consistent API backed by the OpenDAL Rust core.

## Useful Links

- **User guide**: https://opendal.apache.org/docs/bindings/haskell
- **Services & configuration**: https://opendal.apache.org/services
- **Source**: [`bindings/haskell/`](https://github.com/apache/opendal/tree/main/bindings/haskell)

## Build

Requires a working Rust toolchain (`cargo`) in addition to GHC and `cabal`.
The custom build step compiles the Rust FFI library automatically.

```bash
cabal build
```

Run the test suite:

```bash
cabal test
```

Generate Haddock documentation:

```bash
cabal haddock
# cabal haddock --open   (cabal >= 3.8)
```

## Quickstart

```haskell
import OpenDAL

main :: IO ()
main = do
  Right op <- newOperator "memory"
  result <- runOp op $ do
    writeOp "hello.txt" "Hello, World!"
    content <- readOp "hello.txt"
    deleteOp "hello.txt"
    return content
  case result of
    Left err    -> putStrLn $ "error: " ++ show (errorCode err)
    Right bytes -> print bytes
```

See the [user guide](https://opendal.apache.org/docs/bindings/haskell) for a
full walkthrough, including real backends, error handling, streaming writes, and
directory listing.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for how to build, test, and submit
changes.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
