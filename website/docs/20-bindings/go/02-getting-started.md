---
title: Getting started
sidebar_label: Getting started
description: Run your first OpenDAL program in Go, then point it at a real storage backend.
---

# Getting started

## Your first program

This program builds an operator, writes a file, reads it back, inspects its
metadata, and deletes it. It runs against the in-memory service with no
credentials, so you can run it right after installing.

The binding requires **libffi** to be installed. Then add the binding and the
scheme package for the service you want:

```shell
go get github.com/apache/opendal/bindings/go@latest
go get github.com/apache/opendal-go-services/memory
```

```go file=bindings/go/examples/guide/getting_started.go region=quickstart
```

Note that `Write` takes `[]byte` and `Read` returns `[]byte`. Every method
returns an `error` as its last value — check it instead of expecting a panic.

## Point it at a real backend

Only the service changes; the operations stay identical. To use S3, import its
scheme package and pass configuration through `OperatorOptions`, a
`map[string]string` of configuration keys:

```shell
go get github.com/apache/opendal-go-services/s3
```

```go
import (
	"github.com/apache/opendal-go-services/s3"
	opendal "github.com/apache/opendal/bindings/go"
)

op, err := opendal.NewOperator(s3.Scheme, opendal.OperatorOptions{
	"bucket": "my-bucket",
	"region": "us-east-1",
})
if err != nil {
	log.Fatal(err)
}
defer op.Close()

if err := op.Write("hello.txt", []byte("Hello from S3!")); err != nil {
	log.Fatal(err)
}
```

Each service is a separate `opendal-go-services` module, so add the ones you
use. The next page, [Connecting to your storage](./03-connecting.md), covers
configuration and credentials in depth; [Services](/services) lists every
backend and its configuration keys.
