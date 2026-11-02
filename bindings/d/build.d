#!/usr/bin/env dub
/+ dub.sdl:
    name "build"
    dependency "std" version="*"
+/

/**
 * OpenDAL D Bindings Build Script
 *
 * This script builds the OpenDAL C library with specified features
 * and copies the necessary files for D bindings.
 *
 * This script is automatically executed during:
 *   dub build
 *   dub test
 *   dub run
 *
 * For manual execution with custom options:
 *   OPENDAL_TEST=memory dmd -run build.d
 *   OPENDAL_TEST=fs OPENDAL_VERBOSE=1 dmd -run build.d release
 */
module build;

import std.stdio : writeln, stderr;
import std.path : buildPath;
import std.process : spawnShell, wait, environment;
import std.exception : enforce;
import std.file : copy, mkdir, exists;
import std.conv : to;
import std.string : startsWith, split, strip, toLower;
import std.array : join;


version (Windows)
    enum staticlib = "opendal_c.lib";
else
    enum staticlib = "libopendal_c.a";

struct BuildConfig
{
    bool isRelease;
    string[] services;
    bool verbose;
    bool help;
}

void printHelp()
{
    writeln("OpenDAL D Bindings Build Script");
    writeln();
    writeln("This script is automatically executed during:");
    writeln("  dub build    - Build the D bindings");
    writeln("  dub test     - Run unit tests");
    writeln("  dub run      - Build and run examples");
    writeln();
    writeln("Environment Variables (for use with dub commands):");
    writeln("  OPENDAL_TEST         The service to test (e.g., fs, s3, memory, etc.)");
    writeln("  OPENDAL_{SERVICE}_*  Service-specific configuration (e.g., OPENDAL_FS_ROOT)");
    writeln("  OPENDAL_VERBOSE      Set to '1' or 'true' for verbose output");
    writeln();
    writeln("Examples with environment variables:");
    writeln("  OPENDAL_TEST=memory dub build");
    writeln("  OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/test dub test");
    writeln("  OPENDAL_TEST=memory OPENDAL_VERBOSE=1 dub build -b release");
    writeln();
    writeln("For manual execution with command line options:");
    writeln("  dmd -run build.d [options]");
    writeln();
    writeln("Command line options:");
    writeln("  release              Build in release mode (default: debug)");
    writeln("  --verbose            Enable verbose output");
    writeln("  --help               Show this help message");
    writeln();
    writeln("Manual execution examples:");
    writeln("  OPENDAL_TEST=memory dmd -run build.d");
    writeln("  OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/test dmd -run build.d release");
    writeln("  OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp OPENDAL_VERBOSE=1 dmd -run build.d release");
}

BuildConfig parseArgs(string[] args)
{
    BuildConfig config;
    config.services = []; // No default services - user must specify

    // Check environment variables first
    auto envTest = environment.get("OPENDAL_TEST", "");
    if (envTest.length > 0)
    {
        config.services = [envTest];
    }

    auto envVerbose = environment.get("OPENDAL_VERBOSE", "");
    if (envVerbose == "1" || envVerbose.toLower() == "true")
    {
        config.verbose = true;
    }

    foreach (arg; args[1..$])
    {
        if (arg == "release")
        {
            config.isRelease = true;
        }
        else if (arg == "--verbose")
        {
            config.verbose = true;
        }
        else if (arg == "--help" || arg == "-h")
        {
            config.help = true;
        }

        else
        {
            stderr.writeln("Unknown argument: ", arg);
            stderr.writeln("Use --help for usage information");
        }
    }

    return config;
}

void buildCargoLibrary(BuildConfig config)
{
    // Check if services are specified
    if (config.services.length == 0)
    {
        stderr.writeln("Warning: No OpenDAL service specified for testing!");
        stderr.writeln("Use OPENDAL_TEST environment variable to specify the service to test.");
        stderr.writeln("Example: OPENDAL_TEST=memory dub build");
        stderr.writeln("Available services: memory, fs, s3, etc. (see OpenDAL documentation)");
        stderr.writeln();
        stderr.writeln("Building with only core functionality (no storage backends)...");
    }

    // Build features list
    string[] features = ["opendal/blocking"];
    foreach (service; config.services)
    {
        features ~= "opendal/services-" ~ service.strip();
    }

    // Construct cargo command
    auto cargoCmd = "cargo build --manifest-path " ~ buildPath("..", "c", "Cargo.toml");

    if (config.isRelease)
        cargoCmd ~= " --release";

    cargoCmd ~= " --features \"" ~ features.join(",") ~ "\"";

    if (config.verbose)
        cargoCmd ~= " --verbose";

    writeln("Building OpenDAL C library...");
    writeln("Features: ", features.join(", "));
    if (config.verbose)
        writeln("Command: ", cargoCmd);

    auto status = wait(spawnShell(cargoCmd));
    enforce(status == 0, "Cargo build failed with status: " ~ status.to!string);

    writeln("✓ Cargo build completed successfully");
}

void copyArtifacts(BuildConfig config)
{
    string buildType = config.isRelease ? "release" : "debug";

    writeln("Copying build artifacts...");

    // Copy header file
    auto headerSrc = buildPath("..", "c", "include", "opendal.h");
    auto headerDst = buildPath("source", "opendal", "opendal.h");

    if (config.verbose)
        writeln("Copying header: ", headerSrc, " -> ", headerDst);

    copy(headerSrc, headerDst);

    // Ensure lib directory exists
    if (!exists("lib"))
    {
        if (config.verbose)
            writeln("Creating lib directory");
        mkdir("lib");
    }

    // Copy static library
    auto libSrc = buildPath("..", "c", "target", buildType, staticlib);
    auto libDst = buildPath("lib", staticlib);

    if (config.verbose)
        writeln("Copying library: ", libSrc, " -> ", libDst);

    copy(libSrc, libDst);

    writeln("✓ Build artifacts copied successfully");
}

void main(string[] args)
{
    try
    {
        auto config = parseArgs(args);

        if (config.help)
        {
            printHelp();
            return;
        }

        writeln("OpenDAL D Bindings Build");
        writeln("========================");
        writeln("Build mode: ", config.isRelease ? "release" : "debug");
        writeln("Services: ", config.services.join(", "));
        writeln();

        buildCargoLibrary(config);
        copyArtifacts(config);

        writeln();
        writeln("✓ Build completed successfully!");
        writeln("Ready for: dub build, dub test, or dub run");
    }
    catch (Exception e)
    {
        stderr.writeln("❌ Build failed: ", e.msg);
        import core.stdc.stdlib : exit;
        exit(1);
    }
}
