// Copyright 2022 Datafuse Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use futures::StreamExt;

use crate::raw::*;
use crate::*;

/// Operator is the entry for all public async APIs.
///
/// Read [`concepts`][docs::concepts] for know more about [`Operator`].
///
/// # Examples
///
/// Read more backend init examples in [`services`]
///
/// ```
/// # use anyhow::Result;
/// use opendal::services::Fs;
/// use opendal::Operator;
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create fs backend builder.
///     let mut builder = Fs::default();
///     // Set the root for fs, all operations will happen under this root.
///     //
///     // NOTE: the root must be absolute path.
///     builder.root("/tmp");
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::create(builder)?.finish();
///
///     // Create an object handle to start operation on object.
///     let _ = op.object("test_file");
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Operator {
    accessor: FusedAccessor,
}

impl Operator {
    pub(super) fn inner(&self) -> &FusedAccessor {
        &self.accessor
    }

    pub(super) fn from_inner(accessor: FusedAccessor) -> Self {
        Self { accessor }
    }

    pub(super) fn into_innter(self) -> FusedAccessor {
        self.accessor
    }

    /// Get metadata of underlying accessor.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal::Operator;
    ///
    /// # #[tokio::main]
    /// # async fn test(op: Operator) -> Result<()> {
    /// let meta = op.metadata();
    /// # Ok(())
    /// # }
    /// ```
    pub fn metadata(&self) -> OperatorMetadata {
        OperatorMetadata::new(self.accessor.metadata())
    }

    /// Create a new batch operator handle to take batch operations
    /// like `walk` and `remove`.
    pub fn batch(&self) -> BatchOperator {
        BatchOperator::new(self.clone())
    }

    /// Create a new [`Object`][crate::Object] handle to take operations.
    pub fn object(&self, path: &str) -> Object {
        Object::new(self.accessor.clone(), path)
    }

    /// Check if this operator can work correctly.
    ///
    /// We will send a `list` request to path and return any errors we met.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal::Operator;
    ///
    /// # #[tokio::main]
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.check().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check(&self) -> Result<()> {
        let mut ds = self.object("/").list().await?;

        match ds.next().await {
            Some(Err(e)) if e.kind() != ErrorKind::ObjectNotFound => Err(e),
            _ => Ok(()),
        }
    }
}
