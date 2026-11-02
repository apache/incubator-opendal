# Apache OpenDAL™ Go Binding

[![](https://img.shields.io/badge/status-released-blue)](https://pkg.go.dev/github.com/apache/opendal/bindings/go)
[![Go Reference](https://pkg.go.dev/badge/github.com/apache/opendal/bindings/go.svg)](https://pkg.go.dev/github.com/apache/opendal/bindings/go)

opendal-go is a **Native** Go binding for Apache OpenDAL. It is built on top of `opendal-c` without CGO enabled, using [purego](https://github.com/ebitengine/purego) and libffi.

We release the OpenDAL Go binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Go binding version instead of the `opendal` crate
version.

## Useful Links

- [User guide](https://opendal.apache.org/docs/bindings/go)
- [API reference](https://pkg.go.dev/github.com/apache/opendal/bindings/go)
- [Services & configuration](https://opendal.apache.org/services)

## Installation

opendal-go requires **libffi** to be installed.

```bash
go get github.com/apache/opendal/bindings/go@latest
```

Each service is a separate companion module. Add the schemes you use, for example the in-memory service:

```bash
go get github.com/apache/opendal-go-services/memory
```

## Quickstart

```go
package main

import (
	"fmt"
	"log"

	"github.com/apache/opendal-go-services/memory"
	opendal "github.com/apache/opendal/bindings/go"
)

func main() {
	op, err := opendal.NewOperator(memory.Scheme, opendal.OperatorOptions{})
	if err != nil {
		log.Fatal(err)
	}
	defer op.Close()

	if err := op.Write("hello.txt", []byte("Hello, OpenDAL!")); err != nil {
		log.Fatal(err)
	}

	data, err := op.Read("hello.txt")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("Read content: %s\n", data)
}
```

For the full guide — connecting to real backends, streaming, listing, presigning, retries, and error handling — see the [user guide](https://opendal.apache.org/docs/bindings/go).

## Contributing

This document is for users of the binding. If you want to build the binding from source, run the tests, or run benchmarks, see [CONTRIBUTING.md](CONTRIBUTING.md). Contributions are welcome; please also read the project-wide [contributing guide](https://github.com/apache/opendal/blob/main/CONTRIBUTING.md).

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
