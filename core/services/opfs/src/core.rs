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

use super::OPFS_SCHEME;
use opendal_core::raw::*;
use opendal_core::*;

#[derive(Debug, Clone)]
pub struct OpfsCore {
    pub root: String,
    pub info: ServiceInfo,
    pub capability: Capability,
}

impl OpfsCore {
    pub fn new(root: String) -> Self {
        let info = ServiceInfo::new(OPFS_SCHEME, &root, "opfs");
        let capability = Capability {
            stat: true,

            read: true,

            list: true,

            create_dir: true,

            write: true,
            write_can_empty: true,
            write_can_multi: true,

            delete: true,

            shared: true,

            ..Default::default()
        };

        Self {
            root,
            info,
            capability,
        }
    }
}

use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::DomException;

use opendal_core::Error;
use opendal_core::ErrorKind;

pub(crate) fn parse_js_error(value: JsValue) -> Error {
    if let Some(exc) = value.dyn_ref::<DomException>() {
        let kind = match exc.name().as_str() {
            "NotFoundError" => ErrorKind::NotFound,
            "TypeMismatchError" => ErrorKind::NotFound,
            "NotAllowedError" => ErrorKind::PermissionDenied,
            "QuotaExceededError" => ErrorKind::Unexpected,
            e => {
                console_debug!("Got unhandled DOM exception {e:?}");
                ErrorKind::Unexpected
            }
        };
        return Error::new(kind, exc.message());
    }

    // FIXME: how to map `TypeError`` from `getDirectoryHandle`:
    // "name specified is not a valid string or contains characters that
    // would interfere with the native file system"
    if let Some(err) = value.dyn_ref::<js_sys::Error>() {
        return Error::new(ErrorKind::Unexpected, String::from(err.message()));
    }

    Error::new(
        ErrorKind::Unexpected,
        value
            .as_string()
            .unwrap_or_else(|| "unknown JS error".to_string()),
    )
}

mod utils {
    use opendal_core::Result;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::FileSystemDirectoryHandle;
    use web_sys::FileSystemFileHandle;
    use web_sys::FileSystemGetDirectoryOptions;
    use web_sys::FileSystemGetFileOptions;
    use web_sys::StorageManager;
    use web_sys::Window;
    use web_sys::WorkerGlobalScope;

    use crate::core::*;

    /// Get the storage manager from the current global scope.
    ///
    /// OPFS is available both on the main thread (`Window`) and in workers
    /// (`WorkerGlobalScope`), so `web_sys::window()` cannot be used: it is
    /// `None` inside a worker.
    fn storage_manager() -> Result<StorageManager> {
        let global = js_sys::global();
        if let Some(window) = global.dyn_ref::<Window>() {
            Ok(window.navigator().storage())
        } else if let Some(worker) = global.dyn_ref::<WorkerGlobalScope>() {
            Ok(worker.navigator().storage())
        } else {
            Err(Error::new(
                ErrorKind::Unsupported,
                "OPFS requires a window or worker global scope",
            ))
        }
    }

    /// Get the OPFS root directory handle.
    pub(crate) async fn get_root_directory_handle() -> Result<FileSystemDirectoryHandle> {
        let storage_manager = storage_manager()?;
        // This may fail if not secure (not: HTTPS or localhost)
        JsFuture::from(storage_manager.get_directory())
            .await
            .and_then(JsCast::dyn_into)
            .map_err(parse_js_error)
    }

    /// Navigate to a directory handle by path.
    ///
    /// When `create` is true, intermediate directories are created as needed.
    pub(crate) async fn get_directory_handle(
        path: &str,
        create: bool,
    ) -> Result<FileSystemDirectoryHandle> {
        let opt = FileSystemGetDirectoryOptions::new();
        opt.set_create(create);

        let trimmed = path.trim_matches('/');
        let mut handle = get_root_directory_handle().await?;
        if trimmed.is_empty() {
            return Ok(handle);
        }
        for dir in trimmed.split('/') {
            handle = JsFuture::from(handle.get_directory_handle_with_options(dir, &opt))
                .await
                .and_then(JsCast::dyn_into)
                .map_err(parse_js_error)?;
        }

        Ok(handle)
    }

    /// Split a file path into its parent directory handle and filename.
    ///
    /// When `create` is true, intermediate directories are created as needed.
    pub(crate) async fn get_parent_dir_and_name<'a>(
        path: &'a str,
        create: bool,
    ) -> Result<(FileSystemDirectoryHandle, &'a str)> {
        let trimmed = path.trim_matches('/');
        match trimmed.rsplit_once('/') {
            Some((parent, name)) => {
                let dir = get_directory_handle(parent, create).await?;
                Ok((dir, name))
            }
            None => {
                let root = get_root_directory_handle().await?;
                Ok((root, trimmed))
            }
        }
    }

    /// Get a file handle by its full path.
    ///
    /// When `create` is true, intermediate directories and the file itself are created as needed.
    pub(crate) async fn get_file_handle(path: &str, create: bool) -> Result<FileSystemFileHandle> {
        let (dir, name) = get_parent_dir_and_name(path, create).await?;

        let opt = FileSystemGetFileOptions::new();
        opt.set_create(create);

        JsFuture::from(dir.get_file_handle_with_options(name, &opt))
            .await
            .and_then(JsCast::dyn_into)
            .map_err(parse_js_error)
    }
}

pub(super) use utils::*;
