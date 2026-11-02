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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]
use std::fmt::Debug;
use std::fmt::Formatter;
use std::sync::Arc;

use globset::Glob;
use globset::GlobSet;
use globset::GlobSetBuilder;
use opendal_core::raw::*;
use opendal_core::*;

/// `RouteLayer` routes operations to different operators by matching paths
/// against glob patterns.
///
/// Routes are evaluated in insertion order, and the first matching route wins.
/// An unmatched path uses the operator wrapped by this layer. Copy and rename
/// operations select a route from the source path, so this layer does not move
/// data between two routed services. A batched delete routes each path
/// independently.
///
/// [`OperatorInfo::capability`] continues to describe the wrapped operator; it
/// does not aggregate the capabilities of route targets.
///
/// # Example
///
/// ```no_run
/// use opendal_core::services::Memory;
/// use opendal_core::{Operator, Result};
/// use opendal_layer_route::RouteLayer;
///
/// fn build_operator() -> Result<Operator> {
///     let archive = Operator::new(Memory::default())?;
///     let routes = RouteLayer::builder()
///         .route("archive/**", archive)
///         .build()?;
///
///     Ok(Operator::new(Memory::default())?.layer(routes))
/// }
/// ```
#[derive(Clone, Debug)]
pub struct RouteLayer {
    router: Arc<RouteRouter>,
}

impl RouteLayer {
    /// Create a builder for `RouteLayer`.
    pub fn builder() -> RouteLayerBuilder {
        RouteLayerBuilder::default()
    }
}

/// Builder for `RouteLayer`.
#[derive(Default)]
pub struct RouteLayerBuilder {
    routes: Vec<RouteEntry>,
}

impl RouteLayerBuilder {
    /// Add a route for paths that match `pattern`.
    ///
    /// If multiple patterns match, the route added first wins. The target
    /// operator contributes both its service and its operation context.
    pub fn route(mut self, pattern: impl AsRef<str>, op: Operator) -> Self {
        let (ctx, srv) = op.into_parts();
        self.routes.push(RouteEntry {
            pattern: pattern.as_ref().to_string(),
            target: RouteTarget { srv, ctx },
        });
        self
    }

    /// Build the route layer.
    ///
    /// This method returns [`ErrorKind::ConfigInvalid`] if a glob pattern is
    /// invalid.
    pub fn build(self) -> Result<RouteLayer> {
        let mut builder = GlobSetBuilder::new();
        let mut targets = Vec::with_capacity(self.routes.len());

        for entry in self.routes {
            let glob = Glob::new(&entry.pattern).map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "invalid route glob pattern")
                    .with_context("pattern", entry.pattern.clone())
                    .with_context("source", err.to_string())
            })?;
            builder.add(glob);
            targets.push(entry.target);
        }

        let glob = builder.build().map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "failed to build route glob set")
                .with_context("source", err.to_string())
        })?;

        Ok(RouteLayer {
            router: Arc::new(RouteRouter { glob, targets }),
        })
    }
}

struct RouteEntry {
    pattern: String,
    target: RouteTarget,
}

#[derive(Debug)]
struct RouteRouter {
    glob: GlobSet,
    targets: Vec<RouteTarget>,
}

#[derive(Clone, Debug)]
struct RouteTarget {
    srv: Servicer,
    ctx: OperationContext,
}

enum RouteSelected {
    Default(Servicer),
    Target(RouteTarget),
}

impl RouteRouter {
    fn match_index(&self, path: &str) -> Option<usize> {
        self.glob.matches(path).into_iter().min()
    }

    fn select(&self, path: &str, default: &Servicer) -> RouteSelected {
        self.match_index(path)
            .and_then(|idx| self.targets.get(idx).cloned())
            .map(RouteSelected::Target)
            .unwrap_or_else(|| RouteSelected::Default(default.clone()))
    }

    fn target(&self, idx: usize) -> Option<RouteTarget> {
        self.targets.get(idx).cloned()
    }
}

impl Layer for RouteLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl RouteLayer {
    fn layer(&self, inner: Servicer) -> RouteAccessor {
        RouteAccessor {
            inner: Arc::new(inner),
            router: self.router.clone(),
        }
    }
}

#[doc(hidden)]
pub struct RouteAccessor {
    inner: Servicer,
    router: Arc<RouteRouter>,
}

impl Debug for RouteAccessor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl RouteAccessor {
    fn select(&self, path: &str) -> RouteSelected {
        self.router.select(path, &self.inner)
    }
}

impl Service for RouteAccessor {
    type Reader = oio::Reader;
    type Writer = oio::Writer;
    type Lister = oio::Lister;
    type Deleter = oio::Deleter;
    type Copier = oio::Copier;
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        let mut capability = self.inner.capability();
        capability.write_can_copy_from = false;
        capability.compose = false;
        capability.compose_with_content_type = false;
        capability.compose_with_content_disposition = false;
        capability.compose_with_content_encoding = false;
        capability.compose_with_cache_control = false;
        capability.compose_with_user_metadata = false;
        capability.compose_with_if_match = false;
        capability.compose_with_if_none_match = false;
        capability.compose_with_if_version_match = false;
        capability.compose_with_if_version_not_match = false;
        capability.compose_with_if_not_exists = false;
        capability.compose_with_source_version = false;
        capability.compose_with_source_if_match = false;
        capability
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.create_dir(ctx, path, args).await,
            RouteSelected::Target(target) => target.srv.create_dir(&target.ctx, path, args).await,
        }
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<oio::Reader> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.read(ctx, path, args),
            RouteSelected::Target(target) => target.srv.read(&target.ctx, path, args),
        }
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<oio::Writer> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.write(ctx, path, args),
            RouteSelected::Target(target) => target.srv.write(&target.ctx, path, args),
        }
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<oio::Copier> {
        match self.select(from) {
            RouteSelected::Default(srv) => srv.copy(ctx, from, to, args),
            RouteSelected::Target(target) => target.srv.copy(&target.ctx, from, to, args),
        }
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        match self.select(from) {
            RouteSelected::Default(srv) => srv.rename(ctx, from, to, args).await,
            RouteSelected::Target(target) => target.srv.rename(&target.ctx, from, to, args).await,
        }
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.restore(ctx, path, args).await,
            RouteSelected::Target(target) => target.srv.restore(&target.ctx, path, args).await,
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.stat(ctx, path, args).await,
            RouteSelected::Target(target) => target.srv.stat(&target.ctx, path, args).await,
        }
    }

    fn delete(&self, ctx: &OperationContext) -> Result<oio::Deleter> {
        Ok(Box::new(RouteDeleter::new(
            self.inner.clone(),
            self.router.clone(),
            ctx.clone(),
        )) as oio::Deleter)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<oio::Lister> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.list(ctx, path, args),
            RouteSelected::Target(target) => target.srv.list(&target.ctx, path, args),
        }
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        match self.select(path) {
            RouteSelected::Default(srv) => srv.presign(ctx, path, args).await,
            RouteSelected::Target(target) => target.srv.presign(&target.ctx, path, args).await,
        }
    }
}

#[doc(hidden)]
pub struct RouteDeleter {
    default: Servicer,
    router: Arc<RouteRouter>,
    ctx: OperationContext,
    default_deleter: Option<oio::Deleter>,
    target_deleters: Vec<Option<oio::Deleter>>,
}

impl RouteDeleter {
    fn new(default: Servicer, router: Arc<RouteRouter>, ctx: OperationContext) -> Self {
        let mut target_deleters = Vec::with_capacity(router.targets.len());
        target_deleters.resize_with(router.targets.len(), || None);
        Self {
            default,
            router,
            ctx,
            default_deleter: None,
            target_deleters,
        }
    }

    async fn get_deleter(&mut self, key: RouteKey) -> Result<&mut oio::Deleter> {
        match key {
            RouteKey::Default => {
                if self.default_deleter.is_none() {
                    let deleter = self.default.delete(&self.ctx)?;
                    self.default_deleter = Some(deleter);
                }
                Ok(self
                    .default_deleter
                    .as_mut()
                    .expect("default deleter must exist"))
            }
            RouteKey::Target(idx) => {
                if idx >= self.target_deleters.len() {
                    if self.default_deleter.is_none() {
                        let deleter = self.default.delete(&self.ctx)?;
                        self.default_deleter = Some(deleter);
                    }
                    return Ok(self
                        .default_deleter
                        .as_mut()
                        .expect("default deleter must exist"));
                }
                if self.target_deleters[idx].is_none() {
                    let Some(target) = self.router.target(idx) else {
                        let deleter = self.default.delete(&self.ctx)?;
                        self.default_deleter = Some(deleter);
                        return Ok(self
                            .default_deleter
                            .as_mut()
                            .expect("default deleter must exist"));
                    };
                    let deleter = target.srv.delete(&target.ctx)?;
                    self.target_deleters[idx] = Some(deleter);
                }
                Ok(self.target_deleters[idx]
                    .as_mut()
                    .expect("target deleter must exist"))
            }
        }
    }
}

impl oio::Delete for RouteDeleter {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        let key = match self.router.match_index(path) {
            Some(idx) => RouteKey::Target(idx),
            None => RouteKey::Default,
        };
        let deleter = self.get_deleter(key).await?;
        deleter.delete(path, args).await
    }

    async fn close(&mut self) -> Result<()> {
        let mut first_err = None;
        if let Some(deleter) = self.default_deleter.as_mut()
            && let Err(err) = deleter.close().await
        {
            first_err = Some(err);
        }
        for deleter in self.target_deleters.iter_mut().flatten() {
            if let Err(err) = deleter.close().await
                && first_err.is_none()
            {
                first_err = Some(err);
            }
        }

        match first_err {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
enum RouteKey {
    Default,
    Target(usize),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use opendal_service_fs::Fs;

    use super::*;

    fn build_memory_operator() -> Result<Operator> {
        Operator::new(services::Memory::default())
    }

    fn build_mock_operator(name: &'static str) -> Operator {
        Operator::from_parts(
            OperationContext::default(),
            Arc::new(MockService {
                info: ServiceInfo::new("mock", "", name),
                paths: Arc::new(Mutex::new(HashSet::new())),
            }),
        )
    }

    async fn build_fs_operator(label: &str) -> Result<(Operator, PathBuf)> {
        let root = fs_root(label);
        tokio::fs::create_dir_all(&root)
            .await
            .map_err(new_std_io_error)?;
        let op = Operator::new(Fs::default().root(&root.to_string_lossy()))?;
        Ok((op, root))
    }

    fn fs_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "opendal-route-{label}-{nanos}-{}",
            std::process::id()
        ));
        root
    }

    async fn cleanup_fs_root(root: &Path) {
        let _ = tokio::fs::remove_dir_all(root).await;
    }

    async fn assert_missing(op: &Operator, path: &str) -> Result<()> {
        let err = op.stat(path).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        Ok(())
    }

    #[derive(Debug)]
    struct MockService {
        info: ServiceInfo,
        paths: Arc<Mutex<HashSet<String>>>,
    }

    impl Service for MockService {
        type Reader = ();
        type Writer = MockWriter;
        type Lister = ();
        type Deleter = ();
        type Copier = ();
        type Composer = ();

        fn info(&self) -> ServiceInfo {
            self.info.clone()
        }

        fn capability(&self) -> Capability {
            Capability {
                stat: true,
                write: true,
                copy: true,
                rename: true,
                ..Default::default()
            }
        }

        async fn create_dir(
            &self,
            _: &OperationContext,
            _: &str,
            _: OpCreateDir,
        ) -> Result<RpCreateDir> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        async fn stat(&self, _: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
            if self.paths.lock().unwrap().contains(path) {
                Ok(RpStat::new(MetadataBuilder::file(0).build()))
            } else {
                Err(Error::new(ErrorKind::NotFound, "path not found"))
            }
        }

        fn read(&self, _ctx: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn write(&self, _: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
            Ok(MockWriter {
                paths: self.paths.clone(),
                path: path.to_string(),
            })
        }

        fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn list(&self, _ctx: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn copy(
            &self,
            _: &OperationContext,
            from: &str,
            to: &str,
            _: OpCopy,
        ) -> Result<Self::Copier> {
            if !self.paths.lock().unwrap().contains(from) {
                return Err(Error::new(ErrorKind::NotFound, "source not found"));
            }
            self.paths.lock().unwrap().insert(to.to_string());
            Ok(())
        }

        async fn rename(
            &self,
            _: &OperationContext,
            from: &str,
            to: &str,
            _: OpRename,
        ) -> Result<RpRename> {
            let mut paths = self.paths.lock().unwrap();
            if !paths.remove(from) {
                return Err(Error::new(ErrorKind::NotFound, "source not found"));
            }
            paths.insert(to.to_string());
            Ok(RpRename::default())
        }

        async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }
    }

    struct MockWriter {
        paths: Arc<Mutex<HashSet<String>>>,
        path: String,
    }

    impl oio::Write for MockWriter {
        async fn write(&mut self, _: Buffer) -> Result<()> {
            Ok(())
        }

        async fn close(&mut self) -> Result<Metadata> {
            self.paths.lock().unwrap().insert(self.path.clone());
            Ok(MetadataBuilder::unknown().build())
        }

        async fn abort(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_first_match_wins() -> Result<()> {
        let default_op = build_memory_operator()?;
        let (fast_op, fast_root) = build_fs_operator("first-match-fast").await?;
        let slow_op = build_memory_operator()?;

        let routed = default_op.clone().layer(
            RouteLayer::builder()
                .route("data/*.txt", fast_op.clone())
                .route("data/**", slow_op.clone())
                .build()?,
        );

        routed.write("data/file.txt", "v").await?;

        assert!(fast_op.stat("data/file.txt").await.is_ok());
        assert_missing(&slow_op, "data/file.txt").await?;
        assert_missing(&default_op, "data/file.txt").await?;

        cleanup_fs_root(&fast_root).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_fallback_to_default() -> Result<()> {
        let default_op = build_memory_operator()?;
        let (hot_op, hot_root) = build_fs_operator("fallback-hot").await?;

        let routed = default_op.clone().layer(
            RouteLayer::builder()
                .route("hot/**", hot_op.clone())
                .build()?,
        );

        routed.write("cold/file.txt", "v").await?;

        assert!(default_op.stat("cold/file.txt").await.is_ok());
        assert_missing(&hot_op, "cold/file.txt").await?;

        cleanup_fs_root(&hot_root).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_copy_and_rename_route_by_from() -> Result<()> {
        let default_op = build_mock_operator("default");
        let hot_op = build_mock_operator("hot");

        let routed = default_op.clone().layer(
            RouteLayer::builder()
                .route("hot/**", hot_op.clone())
                .build()?,
        );

        routed.write("hot/src.txt", "v").await?;
        routed.copy("hot/src.txt", "cold/copied.txt").await?;

        assert!(hot_op.stat("hot/src.txt").await.is_ok());
        assert!(hot_op.stat("cold/copied.txt").await.is_ok());
        assert_missing(&default_op, "cold/copied.txt").await?;

        routed.write("hot/src2.txt", "v").await?;
        routed.rename("hot/src2.txt", "cold/renamed.txt").await?;

        assert_missing(&hot_op, "hot/src2.txt").await?;
        assert!(hot_op.stat("cold/renamed.txt").await.is_ok());
        assert_missing(&default_op, "cold/renamed.txt").await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_iter_routes_per_path() -> Result<()> {
        let default_op = build_memory_operator()?;
        let (hot_op, hot_root) = build_fs_operator("delete-hot").await?;

        let routed = default_op.clone().layer(
            RouteLayer::builder()
                .route("hot/**", hot_op.clone())
                .build()?,
        );

        routed.write("hot/a.txt", "v").await?;
        routed.write("cold/b.txt", "v").await?;

        routed.delete_iter(["hot/a.txt", "cold/b.txt"]).await?;

        assert_missing(&hot_op, "hot/a.txt").await?;
        assert_missing(&default_op, "cold/b.txt").await?;

        cleanup_fs_root(&hot_root).await;
        Ok(())
    }
}
