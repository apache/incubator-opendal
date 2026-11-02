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

pub fn convert_error(opendal_error: opendal_core::Error) -> dav_server::fs::FsError {
    match opendal_error.kind() {
        opendal_core::ErrorKind::Unsupported => dav_server::fs::FsError::NotImplemented,
        opendal_core::ErrorKind::NotFound => dav_server::fs::FsError::NotFound,
        opendal_core::ErrorKind::PermissionDenied
        | opendal_core::ErrorKind::IsADirectory
        | opendal_core::ErrorKind::NotADirectory => dav_server::fs::FsError::Forbidden,
        opendal_core::ErrorKind::AlreadyExists
        | opendal_core::ErrorKind::IsSameFile
        | opendal_core::ErrorKind::ConditionNotMatch => dav_server::fs::FsError::Exists,
        _ => dav_server::fs::FsError::GeneralFailure,
    }
}
