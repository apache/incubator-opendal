---
title: Getting started
sidebar_label: Getting started
description: Build your first OpenDAL program in Haskell — create an operator, read and write, and handle errors.
---

# Getting started

## Build and install

The binding is not yet published to Hackage. Clone the repository and work from
`bindings/haskell/`:

```shell
git clone https://github.com/apache/opendal.git
cd opendal/bindings/haskell
cabal build
```

A working Rust toolchain (`cargo`) is required alongside GHC and `cabal`; the
custom build step compiles the Rust FFI library automatically.

## Your first program

The snippet below creates an in-memory operator (no credentials needed), writes
two keys, reads them back, checks existence, inspects metadata, and deletes a
key.

```haskell file=bindings/haskell/examples/GettingStarted.hs region=quickstart
```

`runOp` executes the `OperatorT` block against the operator and returns
`IO (Either OpenDALError a)`. Inside the block, operations use `MonadOperation`
methods and errors abort the block early.

## Using the raw IO API

Every operation is also available as a standalone `IO` function returning
`Either OpenDALError a`. Use these when you do not want the monad transformer:

```haskell
import OpenDAL

main :: IO ()
main = do
  Right op <- newOperator "memory"

  writeOpRaw op "key1" "value1" >>= \case
    Left err -> putStrLn $ "write failed: " ++ show err
    Right () -> return ()

  readOpRaw op "key1" >>= \case
    Left err    -> putStrLn $ "read failed: " ++ show err
    Right bytes -> print bytes    -- "value1"

  isExistOpRaw op "key1" >>= \case
    Left err        -> putStrLn $ "isExist failed: " ++ show err
    Right doesExist -> print doesExist    -- True
```

## Handling errors

All errors carry an `ErrorCode` and a human-readable message:

```haskell
import OpenDAL

main :: IO ()
main = do
  Right op <- newOperator "memory"
  result <- runOp op $ readOp "does-not-exist"
  case result of
    Left (OpenDALError code msg) -> do
      putStrLn $ "Code: " ++ show code    -- NotFound
      putStrLn $ "Message: " ++ msg
    Right _ -> putStrLn "unexpected success"
```

See the `ErrorCode` type for the complete set of error codes. Common ones include
`NotFound`, `PermissionDenied`, `AlreadyExists`, `Unsupported`, `ConfigInvalid`,
`RateLimited`, `IsADirectory`, `NotADirectory`, `IsSameFile`, `FFIError`, and
`Unexpected`.

## Point it at a real backend

Pass a `HashMap` of service-specific config keys to `OperatorConfig`. The
`OverloadedStrings` extension (enabled by default in the cabal file) lets you
write the scheme as a string literal:

```haskell
import Data.HashMap.Strict qualified as HashMap
import OpenDAL

main :: IO ()
main = do
  let cfg = "s3"
        { ocConfig = HashMap.fromList
            [ ("bucket", "my-bucket")
            , ("region", "us-east-1")
            ]
        }
  result <- newOperator cfg
  case result of
    Left err -> putStrLn $ "operator creation failed: " ++ show err
    Right op -> do
      writeOpRaw op "hello.txt" "Hello from S3!" >>= print
```

Config keys match the Rust core; see [Services](/services) for the full list of
keys for each backend.

## Streaming writes

For writing in chunks, use the `Writer` API instead of `writeOpRaw`:

```haskell
import OpenDAL

main :: IO ()
main = do
  Right op <- newOperator "memory"
  Right writer <- writerOpRaw op "output.bin" defaultWriterOption
  writerWrite writer "chunk one "   -- returns Either OpenDALError ()
  writerWrite writer "chunk two"
  writerClose writer >>= \case      -- returns Either OpenDALError Metadata
    Left err   -> putStrLn $ "close failed: " ++ show err
    Right meta -> print (mContentLength meta)   -- 19
```

Use `appendWriterOption` instead of `defaultWriterOption` to append to an
existing file (service-dependent; `Unsupported` is returned if the backend does
not support it).

## Listing entries

`listOpRaw` returns a `Lister`; pull entries one at a time with `nextLister`
until it returns `Right Nothing`:

```haskell
import OpenDAL

collectAll :: Lister -> IO [String]
collectAll lister = go []
  where
    go acc = nextLister lister >>= \case
      Right (Just path) -> go (path : acc)
      Right Nothing     -> return (reverse acc)
      Left err          -> fail $ "lister error: " ++ show err

main :: IO ()
main = do
  Right op <- newOperator "memory"
  writeOpRaw op "dir/a.txt" "a" >>= print
  writeOpRaw op "dir/b.txt" "b" >>= print
  Right lister <- listOpRaw op "dir/"   -- path must end with /
  paths <- collectAll lister
  mapM_ putStrLn paths
```

`scanOpRaw` works the same way but lists recursively (flat traversal of a
prefix).

## Enable logging

Pass a `co-log` action in `ocLogAction` to see per-request log messages:

```haskell
import Colog (simpleMessageAction)
import OpenDAL

main :: IO ()
main = do
  Right op <- newOperator "memory" { ocLogAction = Just simpleMessageAction }
  writeOpRaw op "k" "v" >>= print
```
