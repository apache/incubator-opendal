// Copyright 2022 Datafuse Labs.
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

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use futures::TryStreamExt;

use crate::layers::ErrorContextLayer;
use crate::layers::TypeEraseLayer;
use crate::object::ObjectLister;
use crate::raw::*;
use crate::*;

/// User-facing APIs for object and object streams.
#[derive(Clone, Debug)]
pub struct Operator {
    accessor: FusedAccessor,
}

impl From<FusedAccessor> for Operator {
    fn from(accessor: FusedAccessor) -> Self {
        Self { accessor }
    }
}

impl Operator {
    /// Create a new operator.
    ///
    /// # Examples
    ///
    /// Read more backend init examples in [examples](https://github.com/datafuselabs/opendal/tree/main/examples).
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// # use opendal::services::Fs;
    /// # use opendal::Object;
    /// # use opendal::Operator;
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
    ///     let _: Object = op.object("test_file");
    ///
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::new_ret_no_self)]
    pub fn new<A: Accessor>(acc: A) -> OperatorBuilder<impl Accessor> {
        OperatorBuilder::new(acc)
    }

    /// Create a new operator
    pub fn create<AB: AccessorBuilder>(mut ab: AB) -> Result<OperatorBuilder<impl Accessor>> {
        let acc = ab.build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// Create a new operator from env.
    pub fn from_map<AB: AccessorBuilder>(
        map: HashMap<String, String>,
    ) -> Result<OperatorBuilder<impl Accessor>> {
        let acc = AB::from_map(map).build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// Create a new operator from iter.
    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<AB: AccessorBuilder>(
        iter: impl Iterator<Item = (String, String)>,
    ) -> Result<OperatorBuilder<impl Accessor>> {
        let acc = AB::from_iter(iter).build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// Create a new operator from env.
    pub fn from_env<AB: AccessorBuilder>() -> Result<OperatorBuilder<impl Accessor>> {
        let acc = AB::from_env().build()?;
        Ok(OperatorBuilder::new(acc))
    }

    /// Get inner accessor.
    ///
    /// This function should only be used by developers to implement layers.
    pub fn inner(&self) -> FusedAccessor {
        self.accessor.clone()
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
        OperatorMetadata {
            acc: self.accessor.metadata(),
        }
    }

    /// Create a new batch operator handle to take batch operations
    /// like `walk` and `remove`.
    pub fn batch(&self) -> BatchOperator {
        BatchOperator::new(self.clone())
    }

    /// Create a new [`Object`][crate::Object] handle to take operations.
    pub fn object(&self, path: &str) -> Object {
        Object::new(self.clone(), path)
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

/// OperatorBuilder is a typed builder to builder an Operator.
///
/// # NOTES
///
/// It's required to call `finish` after the operator built.
pub struct OperatorBuilder<A: Accessor> {
    accessor: A,
}

impl<A: Accessor> OperatorBuilder<A> {
    /// Create a new operator builder.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(accessor: A) -> OperatorBuilder<impl Accessor> {
        // Make sure error context layer hass been attached.
        OperatorBuilder { accessor }.layer(ErrorContextLayer)
    }

    /// Create a new layer.
    ///
    /// # Examples
    ///
    /// This examples needs feature `retry` enabled.
    ///
    /// ```no_build
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// # use opendal::services::fs;
    /// # use opendal::services::fs::Builder;
    /// use opendal::Operator;
    /// use opendal::Layer;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let accessor = fs::Backend::build().finish().await?;
    /// let op = Operator::new(accessor).layer(new_layer);
    /// // All operations will go through the new_layer
    /// let _ = op.object("test_file").read();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn layer<L: Layer<A>>(self, layer: L) -> OperatorBuilder<L::LayeredAccessor> {
        OperatorBuilder {
            accessor: layer.layer(self.accessor),
        }
    }

    /// Finish the building to construct an Operator.
    pub fn finish(self) -> Operator {
        let ob = self.layer(TypeEraseLayer);

        Operator {
            accessor: Arc::new(ob.accessor),
        }
    }
}

/// BatchOperator is used to take batch operations like walk_dir and remove_all, should
/// be constructed by [`Operator::batch()`].
///
/// # TODO
///
/// We will support batch operators between two different operators like copy and move.
#[derive(Clone, Debug)]
pub struct BatchOperator {
    src: Operator,
}

impl BatchOperator {
    pub(crate) fn new(op: Operator) -> Self {
        BatchOperator { src: op }
    }

    /// Walk a dir in the best way that suitable for underlying storage.
    ///
    /// The returning order could be differ for different underlying storage.
    /// And could be changed at any time. Users MUST NOT relay on the order.
    pub fn walk(&self, path: &str) -> Result<ObjectLister> {
        // # TODO
        //
        // After https://github.com/datafuselabs/opendal/issues/353, we can
        // use prefix list for walk_bottom_up.
        self.walk_top_down(path)
    }

    /// Walk a dir in top down way: list current dir first and then list nested dir.
    ///
    /// Refer to [`TopDownWalker`] for more about the behavior details.
    pub fn walk_top_down(&self, path: &str) -> Result<ObjectLister> {
        Ok(ObjectLister::new(
            self.src.clone(),
            Box::new(TopDownWalker::new(self.src.inner(), path)),
        ))
    }

    /// Walk a dir in bottom up way: list nested dir first and then current dir.
    ///
    /// Refer to [`BottomUpWalker`] for more about the behavior details.
    pub fn walk_bottom_up(&self, path: &str) -> Result<ObjectLister> {
        Ok(ObjectLister::new(
            self.src.clone(),
            Box::new(BottomUpWalker::new(self.src.inner(), path)),
        ))
    }

    /// Remove the path and all nested dirs and files recursively.
    ///
    /// **Use this function in cautions to avoid unexpected data loss.**
    pub async fn remove_all(&self, path: &str) -> Result<()> {
        let parent = self.src.object(path);
        let meta = parent.metadata().await?;

        if meta.mode() != ObjectMode::DIR {
            return parent.delete().await;
        }

        let obs = self.walk_bottom_up(path)?;
        obs.try_for_each(|v| async move { v.delete().await }).await
    }
}

/// Metadata for operator, users can use this metadata to get information of operator.
#[derive(Clone, Debug, Default)]
pub struct OperatorMetadata {
    acc: AccessorMetadata,
}

impl OperatorMetadata {
    /// [`Scheme`] of operator.
    pub fn scheme(&self) -> Scheme {
        self.acc.scheme()
    }

    /// Root of operator, will be in format like `/path/to/dir/`
    pub fn root(&self) -> &str {
        self.acc.root()
    }

    /// Name of backend, could be empty if underlying backend doesn't have namespace concept.
    ///
    /// For example:
    ///
    /// - name for `s3` => bucket name
    /// - name for `azblob` => container name
    pub fn name(&self) -> &str {
        self.acc.name()
    }

    /// Check if current backend supports [`Accessor::read`] or not.
    pub fn can_read(&self) -> bool {
        self.acc.capabilities().contains(AccessorCapability::Read)
    }

    /// Check if current backend supports [`Accessor::write`] or not.
    pub fn can_write(&self) -> bool {
        self.acc.capabilities().contains(AccessorCapability::Write)
    }

    /// Check if current backend supports [`Accessor::list`] or not.
    pub fn can_list(&self) -> bool {
        self.acc.capabilities().contains(AccessorCapability::List)
    }

    /// Check if current backend supports [`Accessor::presign`] or not.
    pub fn can_presign(&self) -> bool {
        self.acc
            .capabilities()
            .contains(AccessorCapability::Presign)
    }

    /// Check if current backend supports multipart operations or not.
    pub fn can_multipart(&self) -> bool {
        self.acc
            .capabilities()
            .contains(AccessorCapability::Multipart)
    }

    /// Check if current backend supports blocking operations or not.
    pub fn can_blocking(&self) -> bool {
        self.acc
            .capabilities()
            .contains(AccessorCapability::Blocking)
    }
}
