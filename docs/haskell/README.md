# Apache OpenDAL™ Haskell Binding (WIP)

![](https://img.shields.io/badge/status-unreleased-red)

![](https://github.com/apache/opendal/assets/5351546/87bbf6e5-f19e-449a-b368-3e283016c887)

## Example

Basic usage

```haskell
import OpenDAL

main :: IO ()
main = do
  Right op <- newOperator "memory"
  runOp op operation
  where
    operation = do
      writeOp op "key1" "value1"
      writeOp op "key2" "value2"
      value1 <- readOp op "key1"
      value2 <- readOp op "key2"
```

Use logger

```haskell
import OpenDAL
import Colog (simpleMessageAction)

main :: IO ()
main = do
  Right op <- newOperator "memory" {ocLogAction = Just simpleMessageAction}
  return ()
```

## Build

```bash
cabal build
```

## Test

```bash
cabal test
```

## Doc

To generate the documentation:

```bash
cabal haddock
```

If your `cabal` version is greater than `3.8`, you can use `cabal haddock --open` to open the documentation in your browser. Otherwise, you can visit the documentation from `dist-newstyle/build/$ARCH/ghc-$VERSION/opendal-$VERSION/doc/html/opendal/index.html`.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
