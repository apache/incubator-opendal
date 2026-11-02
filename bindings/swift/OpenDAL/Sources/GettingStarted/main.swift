// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

// ANCHOR: quickstart
import OpenDAL

// Create an operator backed by the in-memory service.
let op = try Operator(scheme: "memory")

// Write bytes to a path.
var data = Data("Hello, World!".utf8)
try op.blockingWrite(&data, to: "hello.txt")

// Read them back.
let result = try op.blockingRead("hello.txt")
print("read \(result.count) bytes")
print(String(data: result, encoding: .utf8)!)
// ANCHOR_END: quickstart
