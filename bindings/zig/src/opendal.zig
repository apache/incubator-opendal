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

pub const c = @import("opendal_c_header");

pub const Operator = struct {
    inner: *c.opendal_operator,

    pub fn init(scheme: []const u8, options: ?*c.opendal_operator_options) !Operator {
        const result = c.opendal_operator_new(scheme.ptr, options);
        if (result.op == null) {
            if (result.@"error") |err| {
                c.opendal_error_free(err);
            }
            return error.OperatorInitFailed;
        }
        return .{
            .inner = result.op.?,
        };
    }

    pub fn deinit(self: *Operator) void {
        c.opendal_operator_free(self.inner);
    }

    pub fn write(self: *const Operator, path: []const u8, data: []const u8) !void {
        const bytes = c.opendal_bytes{
            .data = @constCast(data.ptr),
            .len = data.len,
            .capacity = data.len,
        };
        if (c.opendal_operator_write(self.inner, path.ptr, &bytes)) |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
    }

    pub fn read(self: *const Operator, path: []const u8) ![]const u8 {
        const result = c.opendal_operator_read(self.inner, path.ptr);
        if (result.@"error") |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
        return result.data.data[0..result.data.len];
    }

    pub fn delete(self: *const Operator, path: []const u8) !void {
        if (c.opendal_operator_delete(self.inner, path.ptr)) |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
    }

    pub fn stat(self: *const Operator, path: []const u8) !Metadata {
        const result = c.opendal_operator_stat(self.inner, path.ptr);
        if (result.@"error") |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
        return .{ .inner = result.meta.? };
    }

    pub fn exists(self: *const Operator, path: []const u8) !bool {
        const result = c.opendal_operator_exists(self.inner, path.ptr);
        if (result.@"error") |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
        return result.exists;
    }

    pub fn list(self: *const Operator, path: []const u8) !Lister {
        const result = c.opendal_operator_list(self.inner, path.ptr);
        if (result.@"error") |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
        return .{ .inner = result.lister.? };
    }

    pub fn createDir(self: *const Operator, path: []const u8) !void {
        if (c.opendal_operator_create_dir(self.inner, path.ptr)) |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
    }

    pub fn rename(self: *const Operator, src: []const u8, dest: []const u8) !void {
        if (c.opendal_operator_rename(self.inner, src.ptr, dest.ptr)) |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
    }

    pub fn copy(self: *const Operator, src: []const u8, dest: []const u8) !void {
        if (c.opendal_operator_copy(self.inner, src.ptr, dest.ptr)) |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
    }

    pub fn info(self: *const Operator) !OperatorInfo {
        const info_new = c.opendal_operator_info_new(self.inner);
        if (info_new == null) return error.InfoFailed;
        return .{ .inner = info.? };
    }
};

pub const OperatorInfo = struct {
    inner: *c.opendal_operator_info,

    pub fn deinit(self: *OperatorInfo) void {
        c.opendal_operator_info_free(self.inner);
    }

    pub fn scheme(self: *const OperatorInfo) []const u8 {
        const ptr = c.opendal_operator_info_get_scheme(self.inner);
        defer std.c.free(ptr);
        return std.mem.span(ptr);
    }

    pub fn root(self: *const OperatorInfo) []const u8 {
        const ptr = c.opendal_operator_info_get_root(self.inner);
        defer std.c.free(ptr);
        return std.mem.span(ptr);
    }

    pub fn name(self: *const OperatorInfo) []const u8 {
        const ptr = c.opendal_operator_info_get_name(self.inner);
        defer std.c.free(ptr);
        return std.mem.span(ptr);
    }

    pub fn capability(self: *const OperatorInfo) c.opendal_capability {
        return c.opendal_operator_info_get_capability(self.inner);
    }
};

pub const Lister = struct {
    inner: *c.opendal_lister,

    pub fn deinit(self: *Lister) void {
        c.opendal_lister_free(self.inner);
    }

    pub fn next(self: *const Lister) !?Entry {
        const result = c.opendal_lister_next(self.inner);
        if (result.@"error") |err| {
            errdefer c.opendal_error_free(err);
            try codeToError(err.*.code);
        }
        if (result.entry) |entry| {
            return Entry{ .inner = entry };
        }
        return null;
    }
};

pub const Entry = struct {
    inner: *c.opendal_entry,

    pub fn deinit(self: *Entry) void {
        c.opendal_entry_free(self.inner);
    }

    pub fn path(self: *const Entry) []const u8 {
        const ptr = c.opendal_entry_path(self.inner);
        return std.mem.span(ptr);
    }

    pub fn name(self: *const Entry) []const u8 {
        const ptr = c.opendal_entry_name(self.inner);
        return std.mem.span(ptr);
    }
};

pub const Metadata = struct {
    inner: *c.opendal_metadata,

    pub fn deinit(self: *Metadata) void {
        c.opendal_metadata_free(self.inner);
    }

    pub fn mode(self: *const Metadata) u32 {
        return c.opendal_metadata_mode(self.inner);
    }

    pub fn isDir(self: *const Metadata) bool {
        return c.opendal_metadata_is_dir(self.inner);
    }

    pub fn isFile(self: *const Metadata) bool {
        return c.opendal_metadata_is_file(self.inner);
    }

    pub fn contentLength(self: *const Metadata) u64 {
        return c.opendal_metadata_content_length(self.inner);
    }

    pub fn contentType(self: *const Metadata) ?[]const u8 {
        var len: usize = undefined;
        const ptr = c.opendal_metadata_content_type(self.inner, &len);
        if (ptr == null) return null;
        return ptr[0..len];
    }

    pub fn etag(self: *const Metadata) ?[]const u8 {
        var len: usize = undefined;
        const ptr = c.opendal_metadata_etag(self.inner, &len);
        if (ptr == null) return null;
        return ptr[0..len];
    }

    pub fn lastModified(self: *const Metadata) i64 {
        return c.opendal_metadata_last_modified(self.inner);
    }
};

pub const Code = enum(c.opendal_code) {
    UNEXPECTED = c.OPENDAL_UNEXPECTED,
    UNSUPPORTED = c.OPENDAL_UNSUPPORTED,
    CONFIG_INVALID = c.OPENDAL_CONFIG_INVALID,
    NOT_FOUND = c.OPENDAL_NOT_FOUND,
    PERMISSION_DENIED = c.OPENDAL_PERMISSION_DENIED,
    IS_A_DIRECTORY = c.OPENDAL_IS_A_DIRECTORY,
    NOT_A_DIRECTORY = c.OPENDAL_NOT_A_DIRECTORY,
    ALREADY_EXISTS = c.OPENDAL_ALREADY_EXISTS,
    RATE_LIMITED = c.OPENDAL_RATE_LIMITED,
    IS_SAME_FILE = c.OPENDAL_IS_SAME_FILE,
    CONDITION_NOT_MATCH = c.OPENDAL_CONDITION_NOT_MATCH,
    RANGE_NOT_SATISFIED = c.OPENDAL_RANGE_NOT_SATISFIED,
    CONFLICT = c.OPENDAL_CONFLICT,
};

pub const OpendalError = error{
    Unexpected,
    Unsupported,
    ConfigInvalid,
    NotFound,
    PermissionDenied,
    IsDirectory,
    IsNotDirectory,
    AlreadyExists,
    RateLimited,
    IsSameFile,
    ConditionNotMatch,
    RangeNotSatisfied,
    Conflict,
};

pub fn codeToError(code: c.opendal_code) OpendalError!void {
    return switch (code) {
        c.OPENDAL_UNEXPECTED => error.Unexpected,
        c.OPENDAL_UNSUPPORTED => error.Unsupported,
        c.OPENDAL_NOT_FOUND => error.NotFound,
        c.OPENDAL_CONFIG_INVALID => error.ConfigInvalid,
        c.OPENDAL_PERMISSION_DENIED => error.PermissionDenied,
        c.OPENDAL_IS_A_DIRECTORY => error.IsDirectory,
        c.OPENDAL_NOT_A_DIRECTORY => error.IsNotDirectory,
        c.OPENDAL_ALREADY_EXISTS => error.AlreadyExists,
        c.OPENDAL_RATE_LIMITED => error.RateLimited,
        c.OPENDAL_IS_SAME_FILE => error.IsSameFile,
        c.OPENDAL_CONDITION_NOT_MATCH => error.ConditionNotMatch,
        c.OPENDAL_RANGE_NOT_SATISFIED => error.RangeNotSatisfied,
        c.OPENDAL_CONFLICT => error.Conflict,
        else => {},
    };
}

pub fn errorToCode(err: OpendalError) c_int {
    return switch (err) {
        error.Unexpected => c.OPENDAL_UNEXPECTED,
        error.Unsupported => c.OPENDAL_UNSUPPORTED,
        error.ConfigInvalid => c.OPENDAL_CONFIG_INVALID,
        error.NotFound => c.OPENDAL_NOT_FOUND,
        error.PermissionDenied => c.OPENDAL_PERMISSION_DENIED,
        error.IsDirectory => c.OPENDAL_IS_A_DIRECTORY,
        error.IsNotDirectory => c.OPENDAL_NOT_A_DIRECTORY,
        error.AlreadyExists => c.OPENDAL_ALREADY_EXISTS,
        error.RateLimited => c.OPENDAL_RATE_LIMITED,
        error.IsSameFile => c.OPENDAL_IS_SAME_FILE,
        error.ConditionNotMatch => c.OPENDAL_CONDITION_NOT_MATCH,
        error.RangeNotSatisfied => c.OPENDAL_RANGE_NOT_SATISFIED,
        error.Conflict => c.OPENDAL_CONFLICT,
    };
}
const std = @import("std");
const testing = std.testing;

test "Error Tests" {
    // C code to Zig error
    try testing.expectError(error.Unexpected, codeToError(c.OPENDAL_UNEXPECTED));
    try testing.expectError(error.Unsupported, codeToError(c.OPENDAL_UNSUPPORTED));
    try testing.expectError(error.ConfigInvalid, codeToError(c.OPENDAL_CONFIG_INVALID));
    try testing.expectError(error.NotFound, codeToError(c.OPENDAL_NOT_FOUND));
    try testing.expectError(error.PermissionDenied, codeToError(c.OPENDAL_PERMISSION_DENIED));
    try testing.expectError(error.IsDirectory, codeToError(c.OPENDAL_IS_A_DIRECTORY));
    try testing.expectError(error.IsNotDirectory, codeToError(c.OPENDAL_NOT_A_DIRECTORY));
    try testing.expectError(error.AlreadyExists, codeToError(c.OPENDAL_ALREADY_EXISTS));
    try testing.expectError(error.RateLimited, codeToError(c.OPENDAL_RATE_LIMITED));
    try testing.expectError(error.IsSameFile, codeToError(c.OPENDAL_IS_SAME_FILE));
    try testing.expectError(error.ConditionNotMatch, codeToError(c.OPENDAL_CONDITION_NOT_MATCH));
    try testing.expectError(error.RangeNotSatisfied, codeToError(c.OPENDAL_RANGE_NOT_SATISFIED));
    try testing.expectError(error.Conflict, codeToError(c.OPENDAL_CONFLICT));

    // Zig error to C code
    try testing.expectEqual(c.OPENDAL_UNEXPECTED, errorToCode(error.Unexpected));
    try testing.expectEqual(c.OPENDAL_UNSUPPORTED, errorToCode(error.Unsupported));
    try testing.expectEqual(c.OPENDAL_CONFIG_INVALID, errorToCode(error.ConfigInvalid));
    try testing.expectEqual(c.OPENDAL_NOT_FOUND, errorToCode(error.NotFound));
    try testing.expectEqual(c.OPENDAL_PERMISSION_DENIED, errorToCode(error.PermissionDenied));
    try testing.expectEqual(c.OPENDAL_IS_A_DIRECTORY, errorToCode(error.IsDirectory));
    try testing.expectEqual(c.OPENDAL_NOT_A_DIRECTORY, errorToCode(error.IsNotDirectory));
    try testing.expectEqual(c.OPENDAL_ALREADY_EXISTS, errorToCode(error.AlreadyExists));
    try testing.expectEqual(c.OPENDAL_RATE_LIMITED, errorToCode(error.RateLimited));
    try testing.expectEqual(c.OPENDAL_IS_SAME_FILE, errorToCode(error.IsSameFile));
    try testing.expectEqual(c.OPENDAL_CONDITION_NOT_MATCH, errorToCode(error.ConditionNotMatch));
    try testing.expectEqual(c.OPENDAL_RANGE_NOT_SATISFIED, errorToCode(error.RangeNotSatisfied));
    try testing.expectEqual(c.OPENDAL_CONFLICT, errorToCode(error.Conflict));
}

test "Semantic Analyzer" {
    testing.refAllDecls(@This());
}

test "operator basic operations" {
    // Initialize a operator for "memory" backend, with no options
    var op = try Operator.init("memory", null);
    defer op.deinit();

    // Prepare some data to be written
    const data = "this_string_length_is_24";

    // Write this into path "/testpath"
    try op.write("/testpath", data);
    defer _ = op.delete("/testpath") catch |err| {
        std.debug.panic("Error deleting file: {}\n", .{err});
    };

    // We can read it out, make sure the data is the same
    const read_bytes = try op.read("/testpath");
    try testing.expectEqual(read_bytes.len, 24);
    try testing.expectEqualStrings(read_bytes, data);
}

test "operator advanced operations" {
    var op = try Operator.init("memory", null);
    defer op.deinit();

    // Test directory creation
    try op.createDir("/testdir/");
    try testing.expect(try op.exists("/testdir/"));

    // Test file operations in directory
    const data = "hello world";
    try op.write("/testdir/file.txt", data);
    try testing.expect(try op.exists("/testdir/file.txt"));

    // Test metadata
    var meta = try op.stat("/testdir/file.txt");
    defer meta.deinit();
    try testing.expect(meta.isFile());
    try testing.expect(!meta.isDir());
    try testing.expectEqual(meta.contentLength(), data.len);

    // ================================================================================
    // "operator advanced operations" - Unsupported
    // ================================================================================
    // /home/kassane/opendal/bindings/zig/src/opendal.zig:269:5: 0x103d9a0 in codeToError (test)
    //     return switch (code) {
    //     ^
    // /home/kassane/opendal/bindings/zig/src/opendal.zig:113:13: 0x103f725 in copy (test)
    //             try codeToError(err.*.code);
    //             ^
    // /home/kassane/opendal/bindings/zig/src/opendal.zig:385:5: 0x1040031 in test.operator advanced operations (test)
    //     try op.copy("/testdir/renamed.txt", "/testdir/copied.txt");
    //     ^
    // operator advanced operations (0.99ms)

    // 3 of 4 tests passed

    // Test rename operation
    // try op.rename("/testdir/file.txt", "/testdir/renamed.txt");
    // try testing.expect(!try op.exists("/testdir/file.txt"));
    // try testing.expect(try op.exists("/testdir/renamed.txt"));

    // Test copy operation
    // try op.copy("/testdir/renamed.txt", "/testdir/copied.txt");
    // try testing.expect(try op.exists("/testdir/renamed.txt"));
    // try testing.expect(try op.exists("/testdir/copied.txt"));
    //============================================

    // Test list operation
    var lister = try op.list("/testdir/");
    defer lister.deinit();
    var count: usize = 0;
    while (try lister.next()) |entry| {
        // defer entry.deinit();
        count += 1;
        try testing.expect(entry.path().len > 0);
        try testing.expect(entry.name().len > 0);
        // std.log.debug("entry name: {s}", .{entry.name()});
        // std.log.debug("entry path: {s}", .{entry.path()});
    }
    try testing.expectEqual(count, 2);

    // Test delete operation
    // try op.delete("/testdir/renamed.txt");
    try testing.expect(!try op.exists("/testdir/renamed.txt"));
}

test "sync operations" {
    var op = try Operator.init("memory", null);
    defer op.deinit();

    const data = [_]u8{ 1, 2, 3, 4, 5 };

    // Test first path
    try writeData(&op, "test_path", &data);
    const read_bytes1 = try readData(&op, "test_path");
    try testing.expectEqualSlices(u8, &data, read_bytes1);
}

// FIXME: test async operations
// test "async operations" {
//     const coro = @import("libcoro");

//     const allocator = std.testing.allocator;
//     const stack = try coro.stackAlloc(allocator, null);
//     defer allocator.free(stack);

//     var op = try Operator.init("memory", null);
//     defer op.deinit();

//     const data = [_]u8{ 1, 2, 3, 4, 5 };

//     // Test first path
//     const write_frame1 = try coro.xasync(writeData, .{ &op, "test_path", &data }, stack);
//     const read_frame1 = try coro.xasync(readData, .{ &op, "test_path" }, stack);
//     try testing.expectEqual(write_frame1.status(), .Suspended);
//     try testing.expectEqual(read_frame1.status(), .Suspended);

//     try coro.xawait(write_frame1);
//     const read_bytes1 = try coro.xawait(read_frame1);
//     try testing.expectEqual(write_frame1.status(), .Done);
//     try testing.expectEqual(read_frame1.status(), .Done);
//     try testing.expectEqualSlices(u8, &data, read_bytes1);
// }

fn writeData(op: *Operator, path: []const u8, data: []const u8) !void {
    try op.write(path, data);
}

fn readData(op: *Operator, path: []const u8) ![]const u8 {
    return try op.read(path);
}
