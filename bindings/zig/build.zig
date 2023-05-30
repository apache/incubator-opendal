const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // This function creates a module and adds it to the package's module set, making
    // it available to other packages which depend on this one.
    _ = b.addModule("opendal", .{
        .source_file = .{
            .path = "src/opendal.zig",
        },
        .dependencies = &.{},
    });
    // Creates a step for unit testing. This only builds the test executable
    // but does not run it.
    const unit_tests = b.addTest(.{
        .root_source_file = .{
            .path = "src/opendal.zig",
        },
        .target = target,
        .optimize = optimize,
    });
    unit_tests.addIncludePath("../c/include");
    if (optimize == .Debug)
        unit_tests.addLibraryPath("../../target/debug")
    else
        unit_tests.addLibraryPath("../../target/release");
    unit_tests.linkSystemLibrary("opendal_c");
    unit_tests.linkLibC();

    const opendal_c = buildOpendalC(b);
    const make_opendal_c = b.step("opendal_c", "Build opendal_c library");
    make_opendal_c.dependOn(&opendal_c.step);
    const run_unit_tests = b.addRunArtifact(unit_tests);
    const test_step = b.step("test", "Run opendal tests");
    test_step.dependOn(&run_unit_tests.step);
}
fn buildOpendalC(b: *std.Build) *std.Build.Step.Run {
    const rootdir = (comptime std.fs.path.dirname(@src().file) orelse null) ++ "/";
    const opendalCdir = rootdir ++ "../c";
    return b.addSystemCommand(&[_][]const u8{
        "make",
        "-C",
        opendalCdir,
        "build",
    });
}
