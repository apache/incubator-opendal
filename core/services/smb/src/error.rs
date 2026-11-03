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

use opendal_core::Error;
use opendal_core::ErrorKind;
use opendal_core::raw::new_std_io_error;
use smb::Status;

pub(super) fn parse_smb_error(error: smb::Error) -> Error {
    if let smb::Error::IoError(error) = error {
        return new_std_io_error(error);
    }
    let kind = match &error {
        smb::Error::UnexpectedMessageStatus(status)
        | smb::Error::ReceivedErrorMessage(status, _) => match *status {
            Status::U32_OBJECT_NAME_NOT_FOUND
            | Status::U32_OBJECT_PATH_NOT_FOUND
            // A recently deleted directory can remain pending while another
            // handle closes; treat it as absent for stat/list/delete.
            | 0xC0000056 => {
                ErrorKind::NotFound
            }
            Status::U32_ACCESS_DENIED
            | Status::U32_LOGON_FAILURE
            | Status::U32_USER_ACCOUNT_LOCKED_OUT => ErrorKind::PermissionDenied,
            Status::U32_OBJECT_NAME_COLLISION => ErrorKind::AlreadyExists,
            Status::U32_FILE_IS_A_DIRECTORY => ErrorKind::IsADirectory,
            // STATUS_NOT_A_DIRECTORY is not exposed by smb::Status.
            0xC0000103 => ErrorKind::NotADirectory,
            Status::U32_DIRECTORY_NOT_EMPTY | Status::U32_SHARING_VIOLATION => ErrorKind::Conflict,
            Status::U32_NOT_SUPPORTED | Status::U32_NOT_IMPLEMENTED => ErrorKind::Unsupported,
            _ => ErrorKind::Unexpected,
        },
        smb::Error::NotFound(_) => ErrorKind::NotFound,
        smb::Error::MissingPermissions(_) => ErrorKind::PermissionDenied,
        smb::Error::InvalidConfiguration(_) | smb::Error::InvalidArgument(_) => {
            ErrorKind::ConfigInvalid
        }
        smb::Error::UnsupportedOperation(_) => ErrorKind::Unsupported,
        _ => ErrorKind::Unexpected,
    };
    Error::new(kind, "SMB operation failed").set_source(error)
}
