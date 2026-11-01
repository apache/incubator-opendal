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

# Compiler and flags
CXX = g++
CXXFLAGS = -std=c++17 -Wall -Wextra -O2 -g
INCLUDES = -I../include -I.
LIBS = -lopendal_c -lcurl
ifneq ($(shell uname), Darwin)
LIBS += -luuid
endif

# Source files
FRAMEWORK_SOURCES = test_framework.cpp
SUITE_SOURCES = test_suites_basic.cpp test_suites_list.cpp test_suites_reader_writer.cpp test_suites_presign.cpp
RUNNER_SOURCES = test_runner.cpp
ALL_SOURCES = $(FRAMEWORK_SOURCES) $(SUITE_SOURCES) $(RUNNER_SOURCES)

# Object files
OBJECTS = $(ALL_SOURCES:.cpp=.o)

# Target executable
TARGET = opendal_test_runner
EXAMPLE_TARGET = example_test

# Library path (adjust as needed)
LIBPATH = -L../target/debug -L../build

.PHONY: all clean test help example behavior_test

all: $(TARGET)

$(TARGET): $(OBJECTS)
	$(CXX) $(CXXFLAGS) -o $@ $^ $(LIBPATH) $(LIBS)

$(EXAMPLE_TARGET): example_test.o test_framework.o
	$(CXX) $(CXXFLAGS) -o $@ $^ $(LIBPATH) $(LIBS)

%.o: %.cpp
	$(CXX) $(CXXFLAGS) $(INCLUDES) -c $< -o $@

example: $(EXAMPLE_TARGET)
	@echo "Running example test..."
	./$(EXAMPLE_TARGET)

behavior_test: $(TARGET)
	@echo "Running OpenDAL C binding behavior tests..."
	LD_LIBRARY_PATH=../target/debug:$$LD_LIBRARY_PATH ./$(TARGET)

test: behavior_test

test-memory: $(TARGET)
	@echo "Running tests with memory service..."
	LD_LIBRARY_PATH=../target/debug:$$LD_LIBRARY_PATH OPENDAL_TEST=memory ./$(TARGET)

test-fs: $(TARGET)
	@echo "Running tests with filesystem service..."
	@mkdir -p /tmp/opendal_test
	LD_LIBRARY_PATH=../target/debug:$$LD_LIBRARY_PATH OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/opendal_test ./$(TARGET)
	@rm -rf /tmp/opendal_test

list-suites: $(TARGET)
	LD_LIBRARY_PATH=../target/debug:$$LD_LIBRARY_PATH ./$(TARGET) --list-suites

clean:
	rm -f $(OBJECTS) $(TARGET) $(EXAMPLE_TARGET) example_test.o

help:
	@echo "Available targets:"
	@echo "  all          - Build the test runner"
	@echo "  behavior_test - Run behavior tests (main target)"
	@echo "  test         - Alias for behavior_test"
	@echo "  example      - Build and run example test"
	@echo "  test-memory  - Run tests with memory service"
	@echo "  test-fs      - Run tests with filesystem service"
	@echo "  list-suites  - List all available test suites"
	@echo "  clean        - Remove compiled files"
	@echo "  help         - Show this help message"
	@echo ""
	@echo "Usage examples:"
	@echo "  OPENDAL_TEST=memory make behavior_test"
	@echo "  OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/test make behavior_test"
	@echo ""
	@echo "Environment variables:"
	@echo "  OPENDAL_TEST                 - Service to test (memory, fs, s3, etc.)"
	@echo "  OPENDAL_DISABLE_RANDOM_ROOT  - Set to 'true' to disable random root"
	@echo "  OPENDAL_<SERVICE>_<CONFIG>   - Service-specific configuration"

# Dependencies
test_framework.o: test_framework.h
test_suites_basic.o: test_framework.h
test_suites_list.o: test_framework.h
test_suites_reader_writer.o: test_framework.h
test_runner.o: test_framework.h
