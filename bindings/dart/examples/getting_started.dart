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

// The Getting Started example for the Dart binding, embedded into the website
// guide. It runs against the in-memory service with no credentials, so CI can
// execute it directly. The region between the ANCHOR markers is what the docs
// show — keep it copy-pasteable.

// ANCHOR: quickstart
import 'dart:typed_data';
import '../lib/opendal.dart';

void main() async {
  // Initialize storage with the memory service — no credentials needed.
  final storage = await Storage.init(schemeStr: "memory", map: {"root": "/"});

  // initFile() returns a factory function that mirrors dart:io File.
  final File = storage.initFile();

  final file = File("hello.txt");

  // Write bytes to the file.
  await file.write(Uint8List.fromList("Hello, OpenDAL!".codeUnits));

  // Read bytes back.
  final data = await file.read();
  print(String.fromCharCodes(data)); // Hello, OpenDAL!

  // Inspect metadata.
  final meta = await file.stat();
  print("isFile: ${meta.isFile}");
  print("contentLength: ${meta.contentLength}");

  // Delete the file.
  await file.delete();
}
// ANCHOR_END: quickstart
