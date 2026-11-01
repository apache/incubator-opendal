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

// Getting Started example for the Zig binding, embedded into the website
// guide. It runs against the in-memory service with no credentials, so CI can
// execute it directly.

// ANCHOR: quickstart
const std = @import("std");
const opendal = @import("opendal");

pub fn main() !void {
    // Initialize an operator for the "memory" backend with no options.
    var op = try opendal.Operator.init("memory", null);
    defer op.deinit();

    // Write bytes to a path.
    try op.write("/hello.txt", "Hello, World!");

    // Read them back.
    const data = try op.read("/hello.txt");
    std.debug.print("read: {s}\n", .{data});

    // Inspect metadata.
    var meta = try op.stat("/hello.txt");
    defer meta.deinit();
    std.debug.print("size: {d} bytes\n", .{meta.contentLength()});
    std.debug.print("is file: {}\n", .{meta.isFile()});

    // Delete.
    try op.delete("/hello.txt");
    std.debug.print("done\n", .{});
}
// ANCHOR_END: quickstart
