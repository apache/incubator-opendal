# Apache OpenDAL™ C++ Binding

[![status: unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/cpp)

A C++ binding for OpenDAL, providing access to S3, GCS, Azure Blob, HDFS, the
local filesystem, and 50+ more services through one API.

> **Note**: This binding is experimental/WIP. Only Clang and AppleClang are
> currently supported. The API may change without notice.

We release the OpenDAL C++ binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the C++ binding version instead of the `opendal` crate
version.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/cpp](https://opendal.apache.org/docs/bindings/cpp)
- **Services & configuration**: [opendal.apache.org/services](https://opendal.apache.org/services)
- **Source**: [`bindings/cpp/`](https://github.com/apache/opendal/tree/main/bindings/cpp)
- **Examples**: [`examples/cpp/`](https://github.com/apache/opendal/tree/main/examples/cpp)

## Integration via CMake

### FetchContent

```cmake
include(FetchContent)

FetchContent_Declare(
  opendal-cpp
  GIT_REPOSITORY https://github.com/apache/opendal.git
  GIT_TAG        v0.40.0
  SOURCE_SUBDIR  bindings/cpp
)
FetchContent_MakeAvailable(opendal-cpp)

target_link_libraries(your_target opendal_cpp)
```

### Vendored source

```shell
mkdir third_party && cd third_party
git clone https://github.com/apache/opendal.git
cd opendal && git checkout v0.40.0
```

```cmake
add_subdirectory(third_party/opendal/bindings/cpp)
target_link_libraries(your_target opendal_cpp)
```

### Prerequisites

- CMake ≥ 3.22
- Clang or AppleClang with C++17 support (C++20 required for async)

### Build

```shell
mkdir build && cd build
cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ ..
make
```

### CMake options

| Option | Default | Description |
|---|---|---|
| `OPENDAL_DEV` | `OFF` | Enable all development options at once |
| `OPENDAL_FEATURES` | `""` | Comma-separated list of services, e.g. `"opendal/services-s3"` |
| `OPENDAL_ENABLE_ASYNC` | `OFF` | Enable async API (requires C++20) |
| `OPENDAL_ENABLE_TESTING` | `OFF` | Build and enable tests |
| `OPENDAL_ENABLE_DOCUMENTATION` | `OFF` | Build Doxygen docs |
| `OPENDAL_ENABLE_ADDRESS_SANITIZER` | `OFF` | Enable address sanitizer |

## Example

```cpp
#include "opendal.hpp"
#include <iostream>

int main() {
    // Construct an operator. The second argument is a config map.
    opendal::Operator op("memory");

    // Write, read, stat.
    op.Write("hello.txt", "Hello, OpenDAL!");
    std::string data = op.Read("hello.txt");
    std::cout << data << "\n";

    opendal::Metadata meta = op.Stat("hello.txt");
    std::cout << "size=" << meta.ContentLength() << "\n";
}
```

See the [user guide](https://opendal.apache.org/docs/bindings/cpp) for more
examples including streaming reads, directory listing, real backends, and the
async API.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) at the repository root.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
