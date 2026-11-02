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

fn main() {
    println!("cargo:rerun-if-env-changed=FDB_CLIENT_LIB_PATH");

    #[cfg(target_os = "macos")]
    {
        let lib_dir = std::env::var_os("FDB_CLIENT_LIB_PATH")
            .map(std::path::PathBuf::from)
            .or_else(find_fdb_client_lib_dir_macos);

        if let Some(path) = lib_dir {
            println!("cargo:rustc-link-search=native={}", path.display());
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", path.display());
        }
    }
}

#[cfg(target_os = "macos")]
fn find_fdb_client_lib_dir_macos() -> Option<std::path::PathBuf> {
    const CANDIDATES: &[&str] = &[
        "/opt/homebrew/opt/foundationdb/lib",
        "/opt/homebrew/lib",
        "/usr/local/opt/foundationdb/lib",
        "/usr/local/lib",
    ];

    for dir in CANDIDATES {
        let dir = std::path::Path::new(dir);
        if dir.join("libfdb_c.dylib").exists() {
            return Some(dir.to_path_buf());
        }
    }

    None
}
