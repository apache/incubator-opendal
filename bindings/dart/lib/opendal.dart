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

import 'dart:io' as io;
import 'dart:typed_data';
import 'package:system_info2/system_info2.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart';
import 'src/rust/frb_generated.dart';
import 'src/rust/api/opendal_api.dart';
export 'src/rust/frb_generated.dart';
export 'src/rust/api/opendal_api.dart';

class Storage {
  final Operator _operator;

  Storage._(this._operator);

  static Future<Storage> init({
    required String schemeStr,
    required Map<String, String> map,
  }) async {
    if (!RustLib.instance.initialized) {
      var path = "rust/target/release/";  // default path
      final name = io.Platform.operatingSystem;
      final arch = SysInfo.kernelArchitecture;
      // Set appropriate binary path based on OS and architecture
      final nameLower = name.toLowerCase();
      final archLower = arch.toString().toLowerCase();
      
      if (nameLower == "linux") {
        if (archLower == "x86_64") {
          path = "rust/target/x86_64-unknown-linux-gnu/release/";
        } else if (archLower == "aarch64" || archLower == "arm64") {
          path = "rust/target/aarch64-unknown-linux-gnu/release/";
        } else if (archLower == "arm") {
          path = "rust/target/armv7-unknown-linux-gnueabihf/release/";
        }
      } else if (nameLower == "windows") {
        if (archLower == "x86_64" || archLower == "amd64") {
          path = "rust/target/x86_64-pc-windows-msvc/release/";
        } else if (archLower == "x86" || archLower == "i386") {
          path = "rust/target/i686-pc-windows-msvc/release/";
        } else if (archLower == "aarch64" || archLower == "arm64") {
          path = "rust/target/aarch64-pc-windows-msvc/release/";
        }
      } else if (nameLower == "macos") {
        if (archLower == "x86_64") {
          path = "rust/target/x86_64-apple-darwin/release/";
        } else if (archLower == "aarch64" || archLower == "arm64") {
          path = "rust/target/aarch64-apple-darwin/release/";
        }
      } else if (nameLower == "android") {
        if (archLower == "aarch64" || archLower == "arm64") {
          path = "rust/target/aarch64-linux-android/release/";
        } else if (archLower == "x86_64") {
          path = "rust/target/x86_64-linux-android/release/";
        } else if (archLower == "x86" || archLower == "i386") {
          path = "rust/target/i686-linux-android/release/";
        } else if (archLower == "arm") {
          path = "rust/target/armv7-linux-androideabi/release/";
        }
      } else if (nameLower == "ios") {
        if (archLower == "aarch64" || archLower == "arm64") {
          path = "rust/target/aarch64-apple-ios/release/";
        } else if (archLower == "x86_64") {
          path = "rust/target/x86_64-apple-ios/release/";
        }
      }
      // Fallback to default release dir when cross-compilation target dir
      // does not exist (e.g. native builds on macOS arm64).
      final libName = nameLower == "windows"
          ? "opendal_dart.dll"
          : nameLower == "linux"
              ? "libopendal_dart.so"
              : "libopendal_dart.dylib";
      if (!io.File("$path$libName").existsSync()) {
        path = "rust/target/release/";
      }
      var config = ExternalLibraryLoaderConfig(
        stem: 'opendal_dart',
        ioDirectory: path,
        webPrefix: 'pkg/',
      );
      await RustLib.init(externalLibrary: await loadExternalLibrary(config));
    }
    return Storage._(Operator(scheme: schemeStr, map: map));
  }

  /// Creates a factory function for creating File objects
  Function(String) initFile() {
    return (String path) => File._(path: path, operator: _operator);
  }

  /// Creates a factory function for creating Directory objects
  Function(String) initDir() {
    return (String path) => Directory._(path: path, operator: _operator);
  }
}

class File {
  final String path;
  final Operator _operator;

  File._({required this.path, required Operator operator})
      : _operator = operator;

  Future<bool> exists() {
    return _operator.exists(path: path);
  }

  bool existsSync() {
    return _operator.existsSync(path: path);
  }

  Future<Metadata> stat() {
    return _operator.stat(path: path);
  }

  Metadata statSync() {
    return _operator.statSync(path: path);
  }

  Future<void> delete() {
    return _operator.delete(path: path);
  }

  Future<void> rename(String newPath) {
    return _operator.rename(from: path, to: newPath);
  }

  void renameSync(String newPath) {
    _operator.renameSync(from: path, to: newPath);
  }

  void deleteSync() {
    _operator.deleteSync(path: path);
  }

  Future<Uint8List> read() {
    return _operator.read(path: path);
  }

  Uint8List readSync() {
    return _operator.readSync(path: path);
  }

  Future<void> write(Uint8List data) {
    return _operator.write(path: path, data: data);
  }

  void writeSync(Uint8List data) {
    _operator.writeSync(path: path, data: data);
  }
}

class Directory {
  final String path;
  final Operator _operator;

  Directory._({required this.path, required Operator operator})
      : _operator = operator;

  Future<void> create() {
    return _operator.createDir(path: path);
  }

  void createSync() {
    _operator.createDirSync(path: path);
  }

  Future<bool> exists() {
    return _operator.exists(path: path);
  }

  bool existsSync() {
    return _operator.existsSync(path: path);
  }

  Future<void> rename(String newPath) {
    return _operator.rename(from: path, to: newPath);
  }

  void renameSync(String newPath) {
    _operator.renameSync(from: path, to: newPath);
  }

  Future<Metadata> stat() {
    return _operator.stat(path: path);
  }

  Metadata statSync() {
    return _operator.statSync(path: path);
  }

  Future<void> delete() {
    return _operator.delete(path: path);
  }

  void deleteSync() {
    _operator.deleteSync(path: path);
  }
}
