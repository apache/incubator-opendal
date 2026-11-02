# Apache OpenDAL™ C Binding (WIP)

[![](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/bindings/c/)

A C binding for OpenDAL, providing blocking access to S3, GCS, Azure Blob, the
local filesystem, and 50+ more storage backends through a single C API.

We release the OpenDAL C binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the C binding version instead of the `opendal` crate
version.

**User guide**: https://opendal.apache.org/docs/bindings/c

## Building

Requirements: C11/C++14 compiler, CMake, Rust toolchain.

```shell
mkdir -p build && cd build

# Default build (memory + fs services)
cmake ..
make

# Enable additional services
cmake .. -DFEATURES="opendal/services-s3,opendal/services-gcs"
make
```

The header is at `include/opendal.h`. The shared library is written to
`../../target/debug/` after building.

## Example

```c
#include <assert.h>
#include <stdio.h>
#include "opendal.h"

int main(void)
{
    /* Build an operator for the "memory" service. */
    opendal_result_operator_new result = opendal_operator_new("memory", NULL);
    assert(result.op != NULL && result.error == NULL);
    opendal_operator *op = result.op;

    /* Write bytes. */
    const char *msg = "Hello, OpenDAL!";
    opendal_bytes data = { .data = (uint8_t *)msg, .len = 15 };
    opendal_error *err = opendal_operator_write(op, "/hello.txt", &data);
    assert(err == NULL);

    /* Read them back. */
    opendal_result_read r = opendal_operator_read(op, "/hello.txt");
    assert(r.error == NULL);
    printf("%.*s\n", (int)r.data.len, r.data.data);
    opendal_bytes_free(&r.data);

    opendal_operator_free(op);
    return 0;
}
```

See `examples/` for more, including error handling patterns.

Full documentation is at https://opendal.apache.org/docs/bindings/c.

## Contributing

See [`CONTRIBUTING.md`](../../CONTRIBUTING.md) at the repository root. To run
tests you need Valgrind and GTest:

```shell
cd build && make tests && ./tests
```

Format C sources with `make format` (requires `clang-format`). Do not manually
format `include/opendal.h`.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
