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

// expose the opendal rust core as `core`.
// We will use `ocore::Xxx` to represents all types from opendal rust core.
pub use ::opendal as ocore;
use pyo3::prelude::*;

mod capability;
pub use capability::*;
mod layers;
pub use layers::*;
mod lister;
pub use lister::*;
mod metadata;
pub use metadata::*;
mod operator;
pub use operator::*;
mod file;
pub use file::*;
mod utils;
pub use utils::*;
mod errors;
pub use errors::*;
mod options;
pub use options::*;
mod services;
pub use services::*;

// The `opendal` Python package is a mixed Python/native layout: the public
// package is the hand-written `python/opendal/` (with its own `__init__.py`),
// and this native module is built as the private submodule `opendal._opendal`
// (`module-name` in pyproject.toml). The `#[pymodule] mod` name must therefore
// be `_opendal` to match the compiled `opendal/_opendal.*.so` (its `PyInit`
// symbol); `__init__.py` imports and re-exports from it as the public API.
#[pymodule(gil_used = false)]
mod _opendal {
    use pyo3::prelude::*;

    #[pymodule]
    mod operator {
        #[pymodule_export]
        use crate::{AsyncOperator, Operator};
    }

    // Pickle reconstructors for operators built via `from_uri`. They must live
    // on the top-level `_opendal` module so `__reduce__` can reference them by
    // their qualified name during unpickling.
    #[pymodule_export]
    use crate::{_reconstruct_async_operator, _reconstruct_operator};

    #[pymodule]
    mod file {
        #[pymodule_export]
        use crate::{AsyncFile, File};
    }

    #[pymodule]
    mod capability {
        #[pymodule_export]
        use crate::Capability;
    }

    #[pymodule]
    mod services {
        #[pymodule_export]
        use crate::Scheme;
    }

    #[pymodule]
    mod layers {
        #[pymodule_export]
        use crate::{
            CapabilityOverrideLayer, ConcurrentLimitLayer, Layer, MimeGuessLayer, RetryLayer,
            TimeoutLayer,
        };
    }

    #[pymodule]
    mod types {
        #[pymodule_export]
        use crate::{Entry, EntryMode, Metadata, PresignedRequest};
    }

    // Exceptions are `PyErr` subtypes, not `#[pyclass]`es, so they are added
    // procedurally rather than via `#[pymodule_export]`.
    #[pymodule]
    mod exceptions {
        #[pymodule_init]
        fn init(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
            use pyo3::prelude::*;

            use crate::{
                AlreadyExists, ConditionNotMatch, ConfigInvalid, Conflict, Error, IsADirectory,
                IsSameFile, NotADirectory, NotFound, PermissionDenied, RangeNotSatisfied,
                RateLimited, Unexpected, Unsupported, add_exceptions,
            };

            add_exceptions!(
                m,
                [
                    Error,
                    Unexpected,
                    Unsupported,
                    ConfigInvalid,
                    NotFound,
                    PermissionDenied,
                    IsADirectory,
                    NotADirectory,
                    AlreadyExists,
                    IsSameFile,
                    ConditionNotMatch,
                    RateLimited,
                    RangeNotSatisfied,
                    Conflict,
                ]
            )
        }
    }

    // Option types live directly on the top-level `opendal` module.
    #[pymodule_export]
    use crate::{DeleteOptions, ListOptions, ReadOptions, StatOptions, WriteOptions};

    #[pymodule_export]
    #[allow(non_upper_case_globals)]
    pub const __version__: &str = env!("CARGO_PKG_VERSION");

    // Make the submodules importable as `opendal.<name>`.
    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        crate::register_submodules(m, "opendal")
    }
}
