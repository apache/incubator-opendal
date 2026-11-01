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

cmake_minimum_required(VERSION 3.10)
project(opendal-cpp-examples)

set(CMAKE_CXX_STANDARD 17)

# use local bindings/cpp directory
add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../../bindings/cpp opendal_cpp_build)

add_executable(basic-example basic.cpp)
target_link_libraries(basic-example opendal_cpp)
include_directories(basic-example opendal_cpp)

add_executable(getting-started getting_started.cpp)
target_link_libraries(getting-started opendal_cpp)
include_directories(getting-started opendal_cpp)

if(APPLE)
    target_link_libraries(basic-example "-framework CoreFoundation -framework Security")
    target_link_libraries(getting-started "-framework CoreFoundation -framework Security")
endif()