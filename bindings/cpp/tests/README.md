# OpenDAL C++ Test Framework

This directory contains a comprehensive test framework for the OpenDAL C++ bindings, designed to match the sophistication and organization of test frameworks in other language bindings (Python, Go, Node.js).

## Structure

```
tests/
├── framework/              # Test framework utilities and base classes
│   └── test_framework.hpp  # Core framework header
├── behavior/               # Behavior-based tests (similar to Go tests)
│   ├── read_test.cpp       # Read operation tests
│   ├── write_test.cpp      # Write operation tests
│   ├── delete_test.cpp     # Delete operation tests
│   ├── list_test.cpp       # List operation tests
│   └── async_read_test.cpp # Async read tests (if OPENDAL_ENABLE_ASYNC)
├── benchmark/              # Performance benchmarks
│   └── benchmark_test.cpp  # Comprehensive benchmarks
├── test_main.cpp           # Main test runner
├── basic_test.cpp          # Legacy basic tests (for compatibility)
├── async_test.cpp          # Legacy async tests (for compatibility)
└── README.md               # This file
```

## Features

### 🚀 Comprehensive Test Coverage
- **Behavior-based testing**: Tests organized by functionality (read, write, delete, list, etc.)
- **Async testing**: Full support for async operations when enabled
- **Performance benchmarks**: Detailed performance measurement and analysis
- **Error handling**: Comprehensive error condition testing
- **Concurrency testing**: Multi-threaded operation validation

### 🔧 Test Framework Features
- **Environment configuration**: Automatic test environment setup from environment variables
- **Random data generation**: Utilities for generating test data
- **Performance measurement**: Built-in timing and benchmarking utilities
- **Service abstraction**: Tests work with any OpenDAL service (memory, fs, s3, etc.)
- **Capability-based testing**: Skip tests based on service capabilities

### 📊 Test Categories

#### Behavior Tests
- **Read operations**: File reading, error handling, edge cases
- **Write operations**: File writing, overwriting, binary data
- **Delete operations**: File/directory deletion, recursive operations
- **List operations**: Directory listing, metadata validation
- **Async operations**: All above operations in async mode

#### Benchmark Tests
- **Performance measurement**: Latency and throughput testing
- **Memory usage**: Memory allocation pattern analysis
- **Concurrent operations**: Multi-threaded performance testing
- **Different file sizes**: Performance across various data sizes

## Usage

### Building and Running Tests

```bash
# Configure with testing enabled
cmake -DOPENDAL_ENABLE_TESTING=ON -DOPENDAL_DEV=ON ..

# Build tests
make opendal_cpp_test

# Run all tests
./opendal_cpp_test

# Run specific test suites
./opendal_cpp_test --gtest_filter="ReadBehaviorTest.*"
./opendal_cpp_test --gtest_filter="*Benchmark*"
```

### Environment Configuration

The test framework supports configuration through environment variables:

```bash
# Set the service to test (default: memory)
export OPENDAL_TEST=memory

# Service-specific configuration
export OPENDAL_MEMORY_ROOT=/tmp/opendal_test

# Disable random root (for deterministic paths)
export OPENDAL_DISABLE_RANDOM_ROOT=true

# For other services (example with filesystem)
export OPENDAL_TEST=fs
export OPENDAL_FS_ROOT=/tmp/opendal_fs_test
```

### Async Testing

When built with async support (`-DOPENDAL_ENABLE_ASYNC=ON`):

```bash
# Build with async support
cmake -DOPENDAL_ENABLE_TESTING=ON -DOPENDAL_ENABLE_ASYNC=ON -DOPENDAL_DEV=ON ..
make opendal_cpp_test

# Run async tests
./opendal_cpp_test --gtest_filter="Async*"
```

## Writing New Tests

### Basic Test Structure

```cpp
#include "../framework/test_framework.hpp"

namespace opendal::test {

class MyBehaviorTest : public OpenDALTest {
protected:
    void SetUp() override {
        OpenDALTest::SetUp();
        // Additional setup if needed
    }
};

OPENDAL_TEST_F(MyBehaviorTest, TestSomething) {
    auto path = random_path();
    auto content = random_string(100);
    
    // Your test logic here
    op_.write(path, content);
    auto result = op_.read(path);
    EXPECT_EQ(result, content);
}

} // namespace opendal::test
```

### Async Test Structure

```cpp
#ifdef OPENDAL_ENABLE_ASYNC

class MyAsyncBehaviorTest : public AsyncOpenDALTest {
    // Similar structure but using co_await
};

OPENDAL_TEST_F(MyAsyncBehaviorTest, TestAsyncSomething) {
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        auto path = random_path();
        auto content = random_bytes(100);
        
        co_await op_->write(path, content);
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        
        co_return;
    }());
}

#endif
```

### Benchmark Test Structure

```cpp
class MyBenchmarkTest : public BenchmarkTest {
    // Inherit from BenchmarkTest
};

OPENDAL_TEST_F(MyBenchmarkTest, BenchmarkMyOperation) {
    benchmark_operation(
        "My operation description",
        [&]() {
            // Operation to benchmark
            auto path = random_path();
            op_.write(path, random_string(1024));
        },
        100 // iterations
    );
}
```

## Test Framework API

### Base Classes
- `OpenDALTest`: Base class for sync tests
- `AsyncOpenDALTest`: Base class for async tests (when enabled)
- `BenchmarkTest`: Base class for performance tests

### Utility Functions
- `random_path()`: Generate random file path
- `random_dir_path()`: Generate random directory path
- `random_string(size)`: Generate random string data
- `random_bytes(size)`: Generate random binary data

### Performance Utilities
- `PerformanceTimer`: High-resolution timing
- `benchmark_operation()`: Automated benchmarking with statistics

### Environment
- `TestConfig::instance()`: Access test configuration
- `initialize_test_framework()`: Initialize framework (called by main)

## Integration with CI/CD

### GitHub Actions Example

```yaml
- name: Run C++ Tests
  run: |
    cd bindings/cpp
    mkdir build && cd build
    cmake -DOPENDAL_ENABLE_TESTING=ON -DOPENDAL_DEV=ON ..
    make opendal_cpp_test
    ./opendal_cpp_test --gtest_output=xml:test_results.xml
```

### Test Selection

```bash
# Run only basic functionality tests
./opendal_cpp_test --gtest_filter="*BehaviorTest*" --gtest_filter="-*Benchmark*"

# Run only benchmarks
./opendal_cpp_test --gtest_filter="*Benchmark*"

# Run only async tests
./opendal_cpp_test --gtest_filter="*Async*"

# Run with verbose output
./opendal_cpp_test --gtest_verbose
```

## Comparison with Other Language Bindings

This C++ test framework provides equivalent functionality to:

- **Python**: pytest-based testing with fixtures and parametrization
- **Go**: behavior-based testing with comprehensive coverage
- **Node.js**: Vitest-based testing with async/sync separation
- **All**: Environment-based configuration and capability testing

## Contributing

When adding new tests:

1. **Organize by behavior**: Group related tests in behavior files
2. **Include async variants**: When adding sync tests, consider async equivalents
3. **Add benchmarks**: For performance-critical operations
4. **Document thoroughly**: Include clear test descriptions
5. **Follow patterns**: Use existing test patterns and utilities

## Troubleshooting

### Common Issues

1. **Tests fail with "No service configured"**
   - Set `OPENDAL_TEST` environment variable
   - Ensure the service is properly configured

2. **Async tests don't compile**
   - Build with `-DOPENDAL_ENABLE_ASYNC=ON`
   - Ensure C++20 support and clang compiler

3. **Permission errors**
   - Check file system permissions
   - Ensure test directories are writable

4. **Memory issues**
   - Build with address sanitizer: `-DOPENDAL_ENABLE_ADDRESS_SANITIZER=ON`
   - Check for memory leaks in custom tests 