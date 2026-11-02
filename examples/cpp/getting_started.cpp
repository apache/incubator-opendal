/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

// The Getting Started example for the C++ binding, embedded into the website
// guide. It runs against the in-memory service with no credentials, so CI can
// execute it directly. The region between the ANCHOR markers is what the docs
// show — keep it copy-pasteable.

// ANCHOR: quickstart
#include "opendal.hpp"
#include <iostream>
#include <string>

int main() {
    // Construct an operator for the "memory" service (no credentials needed).
    opendal::Operator op("memory");

    // Write a string.
    op.Write("hello.txt", "Hello, OpenDAL!");

    // Read it back.
    std::string data = op.Read("hello.txt");
    std::cout << data << "\n";  // Hello, OpenDAL!

    // Inspect metadata.
    opendal::Metadata meta = op.Stat("hello.txt");
    std::cout << "size=" << meta.ContentLength() << "\n";
    std::cout << "is_file=" << meta.IsFile() << "\n";

    // Remove.
    op.Remove("hello.txt");
    std::cout << "exists=" << op.Exists("hello.txt") << "\n";  // 0

    return 0;
}
// ANCHOR_END: quickstart
