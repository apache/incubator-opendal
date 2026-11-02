---
title: Getting started
sidebar_label: Getting started
description: Build and run your first OpenDAL C++ program, then connect it to a real storage backend.
---

# Getting started

## Build the binding

Prerequisites: CMake ≥ 3.22, Clang or AppleClang, Rust toolchain (the binding
compiles the Rust core as part of the build).

```shell
cd bindings/cpp
mkdir build && cd build
cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ ..
make
```

For development (enables tests and docs):

```shell
cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
      -DOPENDAL_DEV=ON ..
make
```

## Your first program

This program constructs an operator against the in-memory service (no
credentials needed), writes data, reads it back, and inspects its metadata.

```cpp file=examples/cpp/getting_started.cpp region=quickstart
```

Compile and run (assuming the binding has been installed into your build tree):

```shell
clang++ -std=c++17 -I/path/to/opendal/bindings/cpp/include \
        -L/path/to/build -lopendal_cpp \
        hello.cpp -o hello
./hello
```

## Point it at a real backend

Only the scheme and config map change; every operation is identical. Pass
backend options as a `std::unordered_map<std::string, std::string>`:

```cpp
#include "opendal.hpp"
#include <iostream>

int main() {
    opendal::Operator op("s3", {
        {"bucket", "my-bucket"},
        {"region", "us-east-1"},
    });

    op.Write("hello.txt", "Hello from S3!");
    std::string data = op.Read("hello.txt");
    std::cout << data << "\n";
}
```

The configuration keys match the per-service docs at [/services](/services).
You must enable the service at CMake configure time via `OPENDAL_FEATURES`:

```cmake
cmake -DOPENDAL_FEATURES="opendal/services-s3" ...
```

## Streaming reads

For large objects, use `GetReader` to avoid an unnecessary copy into a
`std::string`. `ReaderStream` wraps the reader as a standard `std::istream`:

```cpp
#include "opendal.hpp"
#include <iostream>
#include <string>

int main() {
    opendal::Operator op("memory");
    op.Write("data.bin", "some large content");

    // Read in chunks via Reader.
    {
        auto reader = op.GetReader("data.bin");
        char buf[64];
        std::streamsize n = reader.Read(buf, sizeof(buf));
        std::cout.write(buf, n);
        std::cout << "\n";
    }

    // Or wrap as a standard istream.
    {
        opendal::ReaderStream stream(op.GetReader("data.bin"));
        std::string token;
        stream >> token;  // reads first whitespace-delimited token
        std::cout << token << "\n";
    }
}
```

## Listing a directory

`Operator::List` returns all entries at once. `GetLister` gives a lazy
iterator you can loop over:

```cpp
#include "opendal.hpp"
#include <iostream>

int main() {
    opendal::Operator op("memory");
    op.Write("dir/a.txt", "a");
    op.Write("dir/b.txt", "b");

    // Eager list — returns std::vector<opendal::Entry>.
    auto entries = op.List("dir/");
    for (const auto& entry : entries) {
        std::cout << entry.path << "\n";
    }

    // Lazy lister — useful for large directories.
    auto lister = op.GetLister("dir/");
    for (const auto& entry : lister) {
        std::cout << entry.path << "\n";
    }
}
```

## Async API

The async API is a separate opt-in header that requires C++20 and Clang/AppleClang.
Enable it at configure time:

```shell
cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
      -DOPENDAL_ENABLE_ASYNC=ON ..
```

Then use `opendal::async::Operator` from `opendal_async.hpp`. Operations return
Rust futures which you `.await` or block on via your async runtime:

```cpp
#include "opendal_async.hpp"
#include <cstdint>
#include <string_view>
#include <vector>

int main() {
    opendal::async::Operator op("memory");

    // Write requires a std::span<uint8_t> as the second argument.
    std::string_view content = "Hello, OpenDAL!";
    std::vector<uint8_t> buf(content.begin(), content.end());
    auto write_fut = op.Write("hello.txt", std::span<uint8_t>(buf));
    // ... await write_fut ...

    auto read_fut = op.Read("hello.txt");
    // ... await read_fut ...
}
```

The async API is more experimental than the sync one. Consult
[`bindings/cpp/include/opendal_async.hpp`](https://github.com/apache/opendal/blob/main/bindings/cpp/include/opendal_async.hpp)
for the current method signatures.
