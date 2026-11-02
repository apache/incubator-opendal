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

use opendal::Error;
use opendal::ErrorKind;

pub(crate) fn parse_error(err: object_store::Error) -> Error {
    match err {
        object_store::Error::NotFound { .. } => {
            Error::new(ErrorKind::NotFound, "path not found").set_source(err)
        }

        object_store::Error::AlreadyExists { .. } => {
            Error::new(ErrorKind::AlreadyExists, "path already exists").set_source(err)
        }

        object_store::Error::PermissionDenied { .. }
        | object_store::Error::Unauthenticated { .. } => {
            Error::new(ErrorKind::PermissionDenied, "permission denied").set_source(err)
        }

        object_store::Error::InvalidPath { .. } => {
            Error::new(ErrorKind::NotFound, "invalid path").set_source(err)
        }

        object_store::Error::NotSupported { .. } => {
            Error::new(ErrorKind::Unsupported, "operation not supported").set_source(err)
        }

        object_store::Error::Precondition { .. } => {
            Error::new(ErrorKind::ConditionNotMatch, "precondition not met").set_source(err)
        }

        object_store::Error::Generic { store, .. } => {
            Error::new(ErrorKind::Unexpected, format!("{store} operation failed")).set_source(err)
        }

        _ => Error::new(ErrorKind::Unexpected, "unknown error").set_source(err),
    }
}
