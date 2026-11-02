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

module opendal;

public import opendal.operator;

version (unittest)
{
	@("Test basic Operator creation")
	@safe unittest
	{
		import std.process: environment;

		// Get test service from environment variable
		auto testService = environment.get("OPENDAL_TEST", "memory");

		/* Initialize operator with automatic configuration */
		auto options = new OperatorOptions();

		// Auto-configure based on environment variables with prefix OPENDAL_{SERVICE}_
		import std.string: toUpper, startsWith, toLower;
		auto prefix = "OPENDAL_" ~ testService.toUpper() ~ "_";
		foreach (key, value; environment.toAA())
		{
			if (key.startsWith(prefix))
			{
				auto configKey = key[prefix.length .. $].toLower();
				options.set(configKey, value);
			}
		}

		Operator op = Operator(testService, options);

		/* Prepare some data to be written */
		string data = "this_string_length_is_24";

		/* Write this into path - use UUID-like path for uniqueness */
		import std.uuid: randomUUID;
		string testPath = randomUUID().toString();

		/* Write this into path */
		op.write(testPath, cast(ubyte[])data.dup);

		/* We can read it out, make sure the data is the same */
		auto read_bytes = op.read(testPath);
		assert(read_bytes.length == 24);
		assert(cast(string)read_bytes.idup == data);

		/* Clean up */
		if (op.exists(testPath))
		{
			op.remove(testPath);
		}
	}

	@("Benchmark parallel and normal functions")
	@safe unittest
	{
		import std.exception: assertNotThrown;
		import std.file: tempDir;
		import std.path: buildPath;
		import std.datetime.stopwatch: StopWatch;
		import std.stdio: writeln;
		import std.process: environment;

		// Get test service from environment variable
		auto testService = environment.get("OPENDAL_TEST", "memory");

		/* Initialize operator with automatic configuration */
		auto options = new OperatorOptions();

		// Auto-configure based on environment variables with prefix OPENDAL_{SERVICE}_
		import std.string: toUpper, startsWith, toLower;
		auto prefix = "OPENDAL_" ~ testService.toUpper() ~ "_";
		foreach (key, value; environment.toAA())
		{
			if (key.startsWith(prefix))
			{
				auto configKey = key[prefix.length .. $].toLower();
				options.set(configKey, value);
			}
		}

		auto op = Operator(testService, options, true);

		/* Use UUID for unique test path */
		import std.uuid: randomUUID;
		string testPath = randomUUID().toString();
		auto testData = cast(ubyte[])"Benchmarking OpenDAL async and normal functions".dup;

		// Benchmark write operations
		StopWatch sw;

		sw.start();
		assertNotThrown(op.write(testPath, testData));
		sw.stop();
		auto normalWriteTime = sw.peek();

		sw.reset();
		sw.start();
		assertNotThrown(op.writeParallel(testPath, testData));
		sw.stop();
		auto parallelWriteTime = sw.peek();

		// Benchmark read operations
		sw.reset();
		sw.start();
		auto normalReadData = op.read(testPath);
		sw.stop();
		auto normalReadTime = sw.peek();

		sw.reset();
		sw.start();
		auto parallelReadData = op.readParallel(testPath);
		sw.stop();
		auto parallelReadTime = sw.peek();

		// Benchmark list operations - create a test directory first
		import std.uuid: randomUUID;
		string dirPath = randomUUID().toString() ~ "/";

		// Create directory and a file in it for meaningful list operation
		op.createDir(dirPath);
		string fileInDir = dirPath ~ randomUUID().toString();
		op.write(fileInDir, cast(ubyte[])"test file in directory".dup);

		sw.reset();
		sw.start();
		op.list(dirPath);
		sw.stop();
		auto normalListTime = sw.peek();

		sw.reset();
		sw.start();
		op.listParallel(dirPath);
		sw.stop();
		auto parallelListTime = sw.peek();

		// Clean up the directory and file
		if (op.exists(fileInDir))
		{
			op.remove(fileInDir);
		}

		// Print benchmark results
		writeln("Write benchmark:");
		writeln("  Normal:   ", normalWriteTime);
		writeln("  Parallel: ", parallelWriteTime);

		writeln("Read benchmark:");
		writeln("  Normal:   ", normalReadTime);
		writeln("  Parallel: ", parallelReadTime);

		writeln("List benchmark:");
		writeln("  Normal:   ", normalListTime);
		writeln("  Parallel: ", parallelListTime);

		// Verify data integrity
		assert(normalReadData == testData);
		assert(parallelReadData == testData);

		// Clean up
		if (op.exists(testPath))
		{
			op.remove(testPath);
			assert(!op.exists(testPath));
		}
	}

}
