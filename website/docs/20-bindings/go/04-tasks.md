---
title: Common tasks
sidebar_label: Common tasks
description: Task-oriented recipes for OpenDAL in Go — read, write, stream, list, delete, copy, rename, and presign.
---

# Common tasks

Recipes for the things you actually do with storage. Each works on every
service. They assume an `op` built as in
[Getting started](./02-getting-started.md). For full signatures, see the
[API reference](https://pkg.go.dev/github.com/apache/opendal/bindings/go).

A few conventions used throughout:

- Paths are relative to the operator's root; a trailing `/` means a directory
  (`logs/app/` is a directory, `logs/app` is a file).
- `Read` returns `[]byte`; `Write` takes `[]byte`.
- Every operation returns an `error` as its last value — always check it.
- Many verbs accept functional options (`opendal.ReadWithRange(...)`,
  `opendal.WriteWithContentType(...)`, …) as trailing arguments.

## Read a whole file

```go
data, err := op.Read("path/to/file")
if err != nil {
	log.Fatal(err)
}
text := string(data)
```

## Read part of a file

`ReadWithRange(offset, length)` reads `length` bytes starting at `offset`:

```go
data, err := op.Read("path/to/file", opendal.ReadWithRange(0, 1024))
```

Use `opendal.ReadWithRangeFrom(offset)` to read from `offset` to the end.

## Stream a large file

Don't load gigabytes into memory — use a `Reader`, which implements
`io.ReadSeekCloser`, and read in chunks:

```go
r, err := op.Reader("big.bin")
if err != nil {
	log.Fatal(err)
}
defer r.Close()

buf := make([]byte, 8192)
for {
	n, err := r.Read(buf)
	if n > 0 {
		// process buf[:n]
	}
	if err == io.EOF {
		break
	}
	if err != nil {
		log.Fatal(err)
	}
}
```

## Write a whole file

```go
err := op.Write("path/to/file", []byte("Hello, World!"))
```

## Stream a large upload

Use a `Writer` for data produced incrementally. It implements `io.WriteCloser`;
call `Write` repeatedly, then `Close` to commit. Each `Write` call accepts up to
256KB.

```go
w, err := op.Writer("big.bin")
if err != nil {
	log.Fatal(err)
}
if _, err := w.Write(firstChunk); err != nil {
	log.Fatal(err)
}
if _, err := w.Write(secondChunk); err != nil {
	log.Fatal(err)
}
// Close finishes the write; data may be lost if you skip it.
if err := w.Close(); err != nil {
	log.Fatal(err)
}
```

## Upload concurrently

For large objects on services with multipart support, upload parts in parallel:

```go
err := op.Write("big.bin", data, opendal.WriteWithConcurrent(8))
```

## Check existence and metadata

```go
exists, err := op.IsExist("path/to/file")
if err != nil {
	log.Fatal(err)
}
if exists {
	meta, err := op.Stat("path/to/file")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("%d bytes, dir = %v\n", meta.ContentLength(), meta.IsDir())
}
```

## List a directory

`List` returns a `Lister` you iterate with `Next` and `Entry`. Always check
`Error` after the loop:

```go
lister, err := op.List("dir/")
if err != nil {
	log.Fatal(err)
}
defer lister.Close()

for lister.Next() {
	entry := lister.Entry()
	fmt.Println(entry.Path())
}
if err := lister.Error(); err != nil {
	log.Fatal(err)
}
```

## Walk a tree recursively

```go
lister, err := op.List("dir/", opendal.ListWithRecursive(true))
```

The iteration is identical; only the option changes.

## Delete a file or a whole tree

```go
err := op.Delete("path/to/file")                              // single path
err = op.Delete("dir/", opendal.DeleteWithRecursive(true))    // path and everything under it
```

`Delete` succeeds even if the path does not exist.

## Create a directory

```go
err := op.CreateDir("path/to/dir/") // the trailing slash is required
```

Creating a directory that already exists succeeds, and creation is recursive
(like `mkdir -p`).

## Copy and rename

```go
err := op.Copy("from.txt", "to.txt")
err = op.Rename("old.txt", "new.txt")
```

Both operate within a single operator and require a service that supports them;
see [capability checks](./05-production.md#capability-checks).

## Generate a presigned URL

Hand a time-limited URL to a third party so they can access an object without
your credentials. The presign methods return a standard `*net/http.Request`:

```go
import "time"

req, err := op.PresignRead("path/to/file", time.Hour)
if err != nil {
	log.Fatal(err)
}
fmt.Println(req.Method, req.URL)
fmt.Println(req.Header)
```

`PresignWrite`, `PresignStat`, and `PresignDelete` work the same way for the
other operations.
