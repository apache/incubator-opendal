#!/bin/bash
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

# OpenDAL C++ Test Runner
# This script helps run the C++ test suite with different configurations

set -e

# Default values
BUILD_DIR="build"
SERVICE="memory"
ASYNC_MODE=""
TEST_FILTER=""
VERBOSE=""
BENCHMARK_ONLY=""
DEV_MODE="ON"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  -s, --service SERVICE    Set test service (default: memory)"
    echo "  -a, --async             Enable async tests"
    echo "  -f, --filter PATTERN    Run tests matching pattern"
    echo "  -b, --benchmark         Run only benchmark tests"
    echo "  -v, --verbose           Verbose test output"
    echo "  -c, --clean             Clean build directory before building"
    echo "  -d, --build-dir DIR     Build directory (default: build)"
    echo "  -h, --help              Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                      # Run all tests with memory service"
    echo "  $0 -a                   # Run all tests including async"
    echo "  $0 -f 'ReadBehavior*'   # Run only read behavior tests"
    echo "  $0 -b                   # Run only benchmark tests"
    echo "  $0 -s fs                # Run tests with filesystem service"
    echo "  $0 -a -b -v             # Run async benchmark tests with verbose output"
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -s|--service)
            SERVICE="$2"
            shift 2
            ;;
        -a|--async)
            ASYNC_MODE="-DOPENDAL_ENABLE_ASYNC=ON"
            shift
            ;;
        -f|--filter)
            TEST_FILTER="$2"
            shift 2
            ;;
        -b|--benchmark)
            BENCHMARK_ONLY="YES"
            shift
            ;;
        -v|--verbose)
            VERBOSE="--gtest_verbose"
            shift
            ;;
        -c|--clean)
            log_info "Cleaning build directory..."
            rm -rf "$BUILD_DIR"
            shift
            ;;
        -d|--build-dir)
            BUILD_DIR="$2"
            shift 2
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# Set up environment variables for the service
export OPENDAL_TEST="$SERVICE"

case $SERVICE in
    memory)
        log_info "Using memory service"
        ;;
    fs)
        log_info "Using filesystem service"
        export OPENDAL_FS_ROOT="/tmp/opendal_cpp_test"
        mkdir -p "$OPENDAL_FS_ROOT"
        ;;
    *)
        log_warning "Using service: $SERVICE (make sure it's properly configured)"
        ;;
esac

# Create build directory if it doesn't exist
mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

# Configure the project
log_info "Configuring project..."
CMAKE_FLAGS="-DOPENDAL_ENABLE_TESTING=ON -DOPENDAL_DEV=$DEV_MODE"

if [[ -n "$ASYNC_MODE" ]]; then
    CMAKE_FLAGS="$CMAKE_FLAGS $ASYNC_MODE"
    log_info "Async mode enabled"
fi

cmake $CMAKE_FLAGS ..

# Build the tests
log_info "Building tests..."
make opendal_cpp_test

# Prepare test arguments
TEST_ARGS=""

if [[ -n "$TEST_FILTER" ]]; then
    if [[ -n "$BENCHMARK_ONLY" ]]; then
        TEST_ARGS="--gtest_filter=*Benchmark*:$TEST_FILTER"
    else
        TEST_ARGS="--gtest_filter=$TEST_FILTER"
    fi
elif [[ -n "$BENCHMARK_ONLY" ]]; then
    TEST_ARGS="--gtest_filter=*Benchmark*"
fi

if [[ -n "$VERBOSE" ]]; then
    TEST_ARGS="$TEST_ARGS $VERBOSE"
fi

# Run the tests
log_info "Running tests with service: $SERVICE"
if [[ -n "$TEST_ARGS" ]]; then
    log_info "Test arguments: $TEST_ARGS"
fi

if ./opendal_cpp_test $TEST_ARGS; then
    log_success "All tests passed!"
    exit 0
else
    log_error "Some tests failed!"
    exit 1
fi 