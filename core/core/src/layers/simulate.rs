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

use std::fmt::Debug;
use std::fmt::Formatter;
use std::sync::Arc;

use crate::raw::oio::Delete;
use crate::raw::oio::List;
use crate::raw::oio::PrefixLister;
use crate::raw::*;
use crate::*;

/// Simulate missing capabilities for backends in a configurable way.
#[derive(Debug, Clone)]
pub struct SimulateLayer {
    read_with_suffix: bool,
    list_recursive: bool,
    stat_dir: bool,
    create_dir: bool,
    delete_recursive: bool,
}

impl Default for SimulateLayer {
    fn default() -> Self {
        Self {
            read_with_suffix: true,
            list_recursive: true,
            stat_dir: true,
            create_dir: true,
            delete_recursive: true,
        }
    }
}

impl SimulateLayer {
    /// Enable or disable suffix read simulation. Default: true.
    pub fn with_read_with_suffix(mut self, enabled: bool) -> Self {
        self.read_with_suffix = enabled;
        self
    }

    /// Enable or disable recursive list simulation. Default: true.
    pub fn with_list_recursive(mut self, enabled: bool) -> Self {
        self.list_recursive = enabled;
        self
    }

    /// Enable or disable stat dir simulation. Default: true.
    pub fn with_stat_dir(mut self, enabled: bool) -> Self {
        self.stat_dir = enabled;
        self
    }

    /// Enable or disable create_dir simulation. Default: true.
    pub fn with_create_dir(mut self, enabled: bool) -> Self {
        self.create_dir = enabled;
        self
    }

    /// Enable or disable recursive delete simulation. Default: true.
    pub fn with_delete_recursive(mut self, enabled: bool) -> Self {
        self.delete_recursive = enabled;
        self
    }
}

impl Layer for SimulateLayer {
    fn apply_service(&self, srv: Servicer) -> Servicer {
        Arc::new(self.layer(srv))
    }
}

impl SimulateLayer {
    fn layer(&self, srv: Servicer) -> SimulateService {
        SimulateService {
            srv,
            config: self.clone(),
        }
    }
}

/// Service that applies capability simulation.
pub struct SimulateService {
    srv: Servicer,
    config: SimulateLayer,
}

impl Debug for SimulateService {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.srv.fmt(f)
    }
}

impl SimulateService {
    fn simulate_capability(&self, mut cap: Capability) -> Capability {
        if self.config.read_with_suffix && cap.read {
            cap.read_with_suffix = true;
        }
        if self.config.create_dir && cap.list && cap.write_can_empty {
            cap.create_dir = true;
        }
        if self.config.delete_recursive && cap.list && cap.delete {
            cap.delete_with_recursive = true;
        }
        cap
    }

    async fn simulate_create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let capability = self.srv.capability();

        if capability.create_dir || !self.config.create_dir {
            return self.srv.create_dir(ctx, path, args).await;
        }

        if capability.write_can_empty && capability.list {
            let mut w = self.srv.write(ctx, path, OpWrite::default())?;
            oio::Write::close(&mut w).await?;
            return Ok(RpCreateDir::default());
        }

        self.srv.create_dir(ctx, path, args).await
    }

    async fn simulate_stat(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpStat,
    ) -> Result<RpStat> {
        let capability = self.srv.capability();

        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        if path.ends_with('/') {
            if capability.create_dir {
                let meta = self
                    .srv
                    .stat(ctx, path, args.clone())
                    .await?
                    .into_metadata();

                if meta.is_file() {
                    return Err(Error::new(
                        ErrorKind::NotFound,
                        "stat expected a directory, but found a file",
                    ));
                }

                return Ok(RpStat::new(meta));
            }

            if self.config.stat_dir && capability.list {
                let mut l = self.srv.list(
                    ctx,
                    path,
                    options::ListOptions {
                        recursive: capability.list_with_recursive,
                        limit: Some(1),
                        ..Default::default()
                    }
                    .into(),
                )?;

                return if l.next().await?.is_some() {
                    Ok(RpStat::new(MetadataBuilder::dir().build()))
                } else {
                    Err(Error::new(
                        ErrorKind::NotFound,
                        "the directory is not found",
                    ))
                };
            }
        }

        self.srv.stat(ctx, path, args).await
    }

    fn simulate_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpList,
    ) -> Result<SimulateLister> {
        let cap = self.srv.capability();

        let recursive = args.recursive();
        let forward = args;

        let lister = match (
            recursive,
            cap.list_with_recursive,
            self.config.list_recursive,
        ) {
            // Backend supports recursive list, forward directly.
            (_, true, _) => {
                let p = self.srv.list(ctx, path, forward)?;
                SimulateLister::One(p)
            }
            // Simulate recursive via flat list when enabled.
            (true, false, true) => {
                if path.ends_with('/') {
                    let p = ServicerFlatLister::new(ctx.clone(), self.srv.clone(), path);
                    SimulateLister::Two(p)
                } else {
                    let parent = get_parent(path);
                    let p = ServicerFlatLister::new(ctx.clone(), self.srv.clone(), parent);
                    let p = PrefixLister::new(p, path);
                    SimulateLister::Four(p)
                }
            }
            // Recursive requested but simulation disabled; rely on backend and propagate errors.
            (true, false, false) => {
                let p = self.srv.list(ctx, path, forward)?;
                SimulateLister::One(p)
            }
            // Non-recursive list: keep existing prefix handling semantics.
            (false, false, _) => {
                if path.ends_with('/') {
                    let p = self.srv.list(ctx, path, forward)?;
                    SimulateLister::One(p)
                } else {
                    let parent = get_parent(path);
                    let p = self.srv.list(ctx, parent, forward)?;
                    let p = PrefixLister::new(p, path);
                    SimulateLister::Three(p)
                }
            }
        };

        Ok(lister)
    }

    async fn simulate_delete_with_recursive(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpDelete,
        deleter: &mut oio::Deleter,
    ) -> Result<()> {
        if !self.capability().delete_with_recursive {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "recursive delete is not supported",
            ));
        }

        let mut non_recursive = args;
        non_recursive.set_recursive(false);

        let mut lister = self.simulate_list(
            ctx,
            path,
            options::ListOptions {
                recursive: true,
                ..Default::default()
            }
            .into(),
        )?;

        while let Some(entry) = lister.next().await? {
            let entry = entry.into_entry();
            let mut entry_args = non_recursive.clone();
            if let Some(version) = entry.metadata().version() {
                entry_args = entry_args.into_version(version);
            }
            deleter.delete(entry.path(), entry_args).await?;
        }

        Ok(())
    }
}

impl Service for SimulateService {
    type Reader = SimulateReader;
    type Writer = oio::Writer;
    type Lister = SimulateLister;
    type Deleter = SimulateDeleter;
    type Copier = oio::Copier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.srv.info()
    }

    fn capability(&self) -> Capability {
        self.simulate_capability(self.srv.capability())
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.simulate_create_dir(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let capability = self.srv.capability();
        let simulate_read_with_suffix =
            self.config.read_with_suffix && capability.read && !capability.read_with_suffix;
        let reader = self.srv.read(ctx, path, args.clone())?;
        let reader = SimulateReader::new(
            ctx.clone(),
            self.srv.clone(),
            path.to_string(),
            args,
            reader,
            simulate_read_with_suffix,
        );
        Ok(reader)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.srv.write(ctx, path, args)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.srv.copy(ctx, from, to, args)
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.srv.compose(ctx, to, args)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.srv.rename(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.srv.restore(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.simulate_stat(ctx, path, args).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let deleter = self.srv.delete(ctx)?;
        Ok(SimulateDeleter::new(
            ctx.clone(),
            self.srv.clone(),
            self.config.clone(),
            deleter,
        ))
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.simulate_list(ctx, path, args)
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.srv.presign(ctx, path, args).await
    }
}

pub type SimulateLister = FourWays<
    oio::Lister,
    ServicerFlatLister,
    PrefixLister<oio::Lister>,
    PrefixLister<ServicerFlatLister>,
>;

pub struct SimulateReader {
    ctx: OperationContext,
    srv: Servicer,
    path: String,
    args: OpRead,
    inner: oio::Reader,
    simulate_read_with_suffix: bool,
}

impl SimulateReader {
    fn new(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        args: OpRead,
        inner: oio::Reader,
        simulate_read_with_suffix: bool,
    ) -> Self {
        Self {
            ctx,
            srv,
            path,
            args,
            inner,
            simulate_read_with_suffix,
        }
    }

    async fn content_length(&self) -> Result<u64> {
        if let Some(v) = self.args.content_length_hint() {
            return Ok(v);
        }

        let op = options::StatOptions {
            version: self.args.version().map(str::to_owned),
            ..Default::default()
        }
        .into();

        Ok(self
            .srv
            .stat(&self.ctx, &self.path, op)
            .await?
            .into_metadata()
            .content_length())
    }

    async fn resolve_range(&self, range: BytesRange) -> Result<BytesRange> {
        if !self.simulate_read_with_suffix || !range.is_suffix() {
            return Ok(range);
        }

        let BytesRange::Suffix { size } = range else {
            unreachable!("checked by BytesRange::is_suffix")
        };

        let content_length = self.content_length().await?;
        let start = content_length.saturating_sub(size);
        Ok(BytesRange::new(start, Some(content_length - start)))
    }
}

impl oio::Read for SimulateReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let range = self.resolve_range(range).await?;
        self.inner.open(range).await
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let range = self.resolve_range(range).await?;
        self.inner.read(range).await
    }
}

pub struct ServicerFlatLister {
    ctx: OperationContext,
    srv: Servicer,
    next_dir: Option<oio::Entry>,
    active_lister: Vec<(Option<oio::Entry>, oio::Lister)>,
}

impl ServicerFlatLister {
    fn new(ctx: OperationContext, srv: Servicer, path: &str) -> Self {
        Self {
            ctx,
            srv,
            next_dir: Some(oio::Entry::new(path, MetadataBuilder::dir().build())),
            active_lister: vec![],
        }
    }
}

impl oio::List for ServicerFlatLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            if let Some(de) = self.next_dir.take() {
                let mut l = match self.srv.list(&self.ctx, de.path(), OpList::new()) {
                    Ok(v) => v,
                    Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                        log::warn!(
                            "ServicerFlatLister skipping directory due to permission denied: {}",
                            de.path()
                        );
                        continue;
                    }
                    Err(e) if e.kind() == ErrorKind::NotFound => {
                        log::warn!(
                            "ServicerFlatLister skipping directory due to not found during listing: {}",
                            de.path()
                        );
                        continue;
                    }
                    Err(e) => {
                        // `de` was taken out of `next_dir` above and is not in
                        // `active_lister` yet, so dropping it here would lose
                        // the directory: a caller that retries this `next()`
                        // would resume from the parent and return `Ok` with
                        // the directory's entries missing.
                        self.next_dir = Some(de);
                        return Err(e);
                    }
                };
                let first = loop {
                    match l.next().await {
                        Ok(v) => break v,
                        Err(e) if e.kind() == ErrorKind::NotFound => {
                            log::warn!(
                                "ServicerFlatLister skipping entry due to not found during listing: {}",
                                de.path()
                            );
                            continue;
                        }
                        Err(e) => {
                            // Same here, and this is the path a temporary
                            // error takes: nothing has been yielded from `l`
                            // yet, so the directory can be listed again from
                            // the start.
                            self.next_dir = Some(de);
                            return Err(e);
                        }
                    }
                };
                if let Some(v) = first {
                    self.active_lister.push((Some(de.clone()), l));

                    if v.mode().is_dir() {
                        if v.path() != de.path() {
                            self.next_dir = Some(v);
                            continue;
                        }
                    } else {
                        return Ok(Some(v));
                    }
                }
            }

            if matches!(self.active_lister.last(), Some((None, _))) {
                let _ = self.active_lister.pop();
                continue;
            }

            let (de, lister) = match self.active_lister.last_mut() {
                Some((de, lister)) => (de, lister),
                None => return Ok(None),
            };

            match lister.next().await {
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    let path = de.as_ref().map(|entry| entry.path()).unwrap_or("<unknown>");
                    log::warn!(
                        "ServicerFlatLister skipping entry due to not found during recursive listing: {}",
                        path
                    );
                    continue;
                }
                Err(e) => return Err(e),
                Ok(Some(v)) if v.mode().is_dir() => {
                    if v.path()
                        != de
                            .as_ref()
                            .expect("de must be present before listing")
                            .path()
                    {
                        self.next_dir = Some(v);
                        continue;
                    }
                }
                Ok(Some(v)) => return Ok(Some(v)),
                Ok(None) => match de.take() {
                    Some(de) => return Ok(Some(de)),
                    None => {
                        let _ = self.active_lister.pop();
                        continue;
                    }
                },
            }
        }
    }
}

/// Deleter wrapper that simulates recursive deletion.
pub struct SimulateDeleter {
    ctx: OperationContext,
    srv: Servicer,
    config: SimulateLayer,
    inner: oio::Deleter,
}

impl SimulateDeleter {
    pub fn new(
        ctx: OperationContext,
        srv: Servicer,
        config: SimulateLayer,
        inner: oio::Deleter,
    ) -> Self {
        Self {
            ctx,
            srv,
            config,
            inner,
        }
    }
}

impl oio::Delete for SimulateDeleter {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        if args.recursive() {
            let cap = self.srv.capability();

            if cap.delete_with_recursive {
                return self.inner.delete(path, args).await;
            }

            if self.config.delete_recursive {
                let service = SimulateService {
                    srv: self.srv.clone(),
                    config: self.config.clone(),
                };
                return service
                    .simulate_delete_with_recursive(&self.ctx, path, args, &mut self.inner)
                    .await;
            }
        }

        self.inner.delete(path, args).await
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, Default)]
    struct MockService {
        capability: Capability,
        tree: Option<Arc<TreeState>>,
    }

    struct MockLister(bool);

    /// A directory tree for `TreeLister`, with one-shot listing faults.
    ///
    /// Each fault fires exactly once and carries a temporary error, which is
    /// what a 503 or a dropped connection looks like to the lister once
    /// `RetryLayer` has classified it, without needing a server to inject it.
    /// The two cover the lister's two ways of failing to open a directory:
    /// `list` itself returning an error, and the first `next()` on the lister
    /// it returned.
    #[derive(Debug, Default)]
    struct TreeState {
        /// Directory path to its children, as `(path, is_dir)`.
        tree: HashMap<String, Vec<(String, bool)>>,
        fail_first_list_on: Option<String>,
        fail_first_next_on: Option<String>,
        list_injected: Mutex<bool>,
        next_injected: Mutex<bool>,
        /// Every path passed to `list`, so a test can assert on retries.
        listed: Mutex<Vec<String>>,
    }

    impl TreeState {
        /// Claim a one-shot fault for `path`, if it is armed and unused.
        fn take_fault(&self, armed: &Option<String>, used: &Mutex<bool>, path: &str) -> bool {
            if armed.as_deref() != Some(path) {
                return false;
            }
            let mut used = used.lock().expect("mutex must not poison");
            let first_time = !*used;
            *used = true;
            first_time
        }
    }

    struct TreeLister {
        entries: std::vec::IntoIter<(String, bool)>,
        fail_first: bool,
    }

    impl TreeLister {
        fn new(state: &Arc<TreeState>, path: &str) -> Self {
            Self {
                entries: state
                    .tree
                    .get(path)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter(),
                fail_first: state.take_fault(&state.fail_first_next_on, &state.next_injected, path),
            }
        }
    }

    impl oio::List for TreeLister {
        async fn next(&mut self) -> Result<Option<oio::Entry>> {
            if self.fail_first {
                self.fail_first = false;
                return Err(
                    Error::new(ErrorKind::Unexpected, "injected listing failure").set_temporary(),
                );
            }

            Ok(self.entries.next().map(|(path, is_dir)| {
                let meta = if is_dir {
                    MetadataBuilder::dir().build()
                } else {
                    MetadataBuilder::file(0).build()
                };
                oio::Entry::new(&path, meta)
            }))
        }
    }

    impl oio::List for MockLister {
        async fn next(&mut self) -> Result<Option<oio::Entry>> {
            if self.0 {
                return Ok(None);
            }
            self.0 = true;
            Ok(Some(oio::Entry::new(
                "parent/file",
                MetadataBuilder::file(0).build(),
            )))
        }
    }

    impl Service for MockService {
        type Reader = ();
        type Writer = ();
        type Lister = TwoWays<MockLister, TreeLister>;
        type Deleter = ();
        type Copier = ();
        type Composer = ();

        fn info(&self) -> ServiceInfo {
            ServiceInfo::with_scheme("mock")
        }

        fn capability(&self) -> Capability {
            self.capability
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

        async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn read(&self, _ctx: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn write(&self, _ctx: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        fn list(&self, _ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
            if let Some(state) = &self.tree {
                state
                    .listed
                    .lock()
                    .expect("mutex must not poison")
                    .push(path.to_string());

                if state.take_fault(&state.fail_first_list_on, &state.list_injected, path) {
                    return Err(
                        Error::new(ErrorKind::Unexpected, "injected list failure").set_temporary()
                    );
                }

                return Ok(TwoWays::Two(TreeLister::new(state, path)));
            }
            if self.capability.list {
                return Ok(TwoWays::One(MockLister(false)));
            }
            Err(Error::new(ErrorKind::Unsupported, "list is not supported"))
        }

        fn copy(&self, _: &OperationContext, _: &str, _: &str, _: OpCopy) -> Result<Self::Copier> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        async fn rename(
            &self,
            _: &OperationContext,
            _: &str,
            _: &str,
            _: OpRename,
        ) -> Result<RpRename> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }
    }

    struct MockReader {
        observed_range: Arc<Mutex<Option<BytesRange>>>,
    }

    impl oio::Read for MockReader {
        async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
            *self.observed_range.lock().expect("mutex must not poison") = Some(range);
            Ok((
                RpRead::new({
                    let metadata = MetadataBuilder::file(0);
                    metadata.build()
                }),
                Box::new(Buffer::new()),
            ))
        }

        async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
            *self.observed_range.lock().expect("mutex must not poison") = Some(range);
            Ok((
                RpRead::new({
                    let metadata = MetadataBuilder::file(0);
                    metadata.build()
                }),
                Buffer::new(),
            ))
        }
    }

    #[test]
    fn simulate_layer_exposes_read_with_suffix() {
        let capability = Capability {
            read: true,
            read_with_suffix: false,
            ..Default::default()
        };
        let srv = Arc::new(MockService {
            capability,
            ..Default::default()
        }) as Servicer;

        let srv = SimulateLayer::default().apply_service(srv);

        assert!(srv.capability().read_with_suffix);
    }

    #[test]
    fn simulate_layer_can_disable_read_with_suffix() {
        let capability = Capability {
            read: true,
            read_with_suffix: false,
            ..Default::default()
        };
        let srv = Arc::new(MockService {
            capability,
            ..Default::default()
        }) as Servicer;

        let srv = SimulateLayer::default()
            .with_read_with_suffix(false)
            .apply_service(srv);

        assert!(!srv.capability().read_with_suffix);
    }

    #[tokio::test]
    async fn simulate_stat_dir_without_recursive_list() -> Result<()> {
        let capability = Capability {
            stat: true,
            list: true,
            list_with_recursive: false,
            ..Default::default()
        };
        let srv = Arc::new(MockService {
            capability,
            ..Default::default()
        }) as Servicer;
        let srv = SimulateLayer::default().apply_service(srv);

        let metadata = srv
            .stat(&OperationContext::new(), "parent/", OpStat::default())
            .await?
            .into_metadata();

        assert_eq!(metadata.mode(), EntryMode::DIR);
        Ok(())
    }

    #[tokio::test]
    async fn simulate_reader_uses_content_length_hint_for_suffix() -> Result<()> {
        let observed_range = Arc::new(Mutex::new(None));
        let (_, args, _) = options::ReadOptions {
            content_length_hint: Some(42),
            ..Default::default()
        }
        .into();
        let reader = SimulateReader::new(
            OperationContext::new(),
            Arc::new(MockService {
                capability: Capability::default(),
                ..Default::default()
            }),
            "test".to_string(),
            args,
            Box::new(MockReader {
                observed_range: observed_range.clone(),
            }),
            true,
        );

        oio::Read::read(&reader, BytesRange::suffix(10)).await?;

        assert_eq!(
            *observed_range.lock().expect("mutex must not poison"),
            Some(BytesRange::new(32, Some(10)))
        );

        Ok(())
    }

    /// Recursively list a two-directory tree in which `a/` fails once.
    ///
    /// Returns the files yielded, how many errors surfaced, and every path
    /// that reached `list`.
    async fn list_tree_with_one_fault(state: Arc<TreeState>) -> (Vec<String>, u32, Vec<String>) {
        let srv = Arc::new(MockService {
            capability: Capability {
                list: true,
                ..Default::default()
            },
            tree: Some(state.clone()),
        }) as Servicer;

        let mut lister = ServicerFlatLister::new(OperationContext::new(), srv, "/");

        // Drain it the way RetryLayer does: a temporary error is retried by
        // calling `next()` again on the same lister.
        let mut files = Vec::new();
        let mut errors = 0;
        loop {
            match oio::List::next(&mut lister).await {
                Ok(Some(entry)) => {
                    if entry.mode().is_file() {
                        files.push(entry.path().to_string());
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    assert!(err.is_temporary(), "injected error is temporary: {err}");
                    errors += 1;
                    assert!(errors <= 2, "the fault is injected once, not repeatedly");
                }
            }
        }
        files.sort();

        let listed = state.listed.lock().expect("mutex must not poison").clone();
        (files, errors, listed)
    }

    fn tree_with_two_dirs() -> HashMap<String, Vec<(String, bool)>> {
        // `oio::Entry` normalizes an empty path to "/", so the root is listed
        // under that name.
        HashMap::from([
            (
                "/".to_string(),
                vec![("a/".to_string(), true), ("b/".to_string(), true)],
            ),
            ("a/".to_string(), vec![("a/one.txt".to_string(), false)]),
            ("b/".to_string(), vec![("b/two.txt".to_string(), false)]),
        ])
    }

    /// The directory is lost when its first `next()` fails.
    #[tokio::test]
    async fn servicer_flat_lister_retries_a_directory_whose_first_entry_failed() {
        let state = Arc::new(TreeState {
            tree: tree_with_two_dirs(),
            fail_first_next_on: Some("a/".to_string()),
            ..Default::default()
        });

        let (files, errors, listed) = list_tree_with_one_fault(state).await;

        assert_eq!(
            errors, 1,
            "the injected fault must surface; listed {listed:?}"
        );
        assert_eq!(
            files,
            vec!["a/one.txt".to_string(), "b/two.txt".to_string()],
            "the directory that failed must be listed again, not skipped"
        );
        assert_eq!(
            listed.iter().filter(|path| path.as_str() == "a/").count(),
            2,
            "a/ must be requested again after the temporary error; listed {listed:?}"
        );
    }

    /// The same loss happens one step earlier, when `list` itself fails.
    #[tokio::test]
    async fn servicer_flat_lister_retries_a_directory_whose_list_failed() {
        let state = Arc::new(TreeState {
            tree: tree_with_two_dirs(),
            fail_first_list_on: Some("a/".to_string()),
            ..Default::default()
        });

        let (files, errors, listed) = list_tree_with_one_fault(state).await;

        assert_eq!(
            errors, 1,
            "the injected fault must surface; listed {listed:?}"
        );
        assert_eq!(
            files,
            vec!["a/one.txt".to_string(), "b/two.txt".to_string()],
            "the directory that failed must be listed again, not skipped"
        );
        assert_eq!(
            listed.iter().filter(|path| path.as_str() == "a/").count(),
            2,
            "a/ must be requested again after the temporary error; listed {listed:?}"
        );
    }

    /// The same loss, but two levels down, where the restored `next_dir` has to
    /// coexist with a non-empty `active_lister` stack.
    ///
    /// The flat cases above fail a directory while only the root's lister is on
    /// the stack. Here `/` and `a/` are both open when `a/b/` fails, so a lost
    /// entry would be masked by the parent resuming instead: the listing would
    /// pop back to `/`, move on to `c/`, and return `Ok` without `deep.txt`.
    #[tokio::test]
    async fn servicer_flat_lister_retries_a_nested_directory() {
        let state = Arc::new(TreeState {
            tree: HashMap::from([
                (
                    "/".to_string(),
                    vec![("a/".to_string(), true), ("c/".to_string(), true)],
                ),
                ("a/".to_string(), vec![("a/b/".to_string(), true)]),
                (
                    "a/b/".to_string(),
                    vec![("a/b/deep.txt".to_string(), false)],
                ),
                ("c/".to_string(), vec![("c/two.txt".to_string(), false)]),
            ]),
            fail_first_next_on: Some("a/b/".to_string()),
            ..Default::default()
        });

        let (files, errors, listed) = list_tree_with_one_fault(state).await;

        assert_eq!(
            errors, 1,
            "the injected fault must surface; listed {listed:?}"
        );
        assert_eq!(
            files,
            vec!["a/b/deep.txt".to_string(), "c/two.txt".to_string()],
            "the nested directory must be listed again, not skipped by the parent"
        );
        assert_eq!(
            listed.iter().filter(|path| path.as_str() == "a/b/").count(),
            2,
            "a/b/ must be requested again after the temporary error; listed {listed:?}"
        );
    }

    /// A permanent error must still end the listing rather than lose entries
    /// silently, so the restored `next_dir` cannot mask a real failure.
    #[tokio::test]
    async fn servicer_flat_lister_reports_a_directory_that_keeps_failing() {
        let state = Arc::new(TreeState {
            tree: tree_with_two_dirs(),
            fail_first_next_on: Some("a/".to_string()),
            ..Default::default()
        });
        let srv = Arc::new(MockService {
            capability: Capability {
                list: true,
                ..Default::default()
            },
            tree: Some(state.clone()),
        }) as Servicer;

        let mut lister = ServicerFlatLister::new(OperationContext::new(), srv, "/");

        // Walk until the fault, then stop: a caller that does not retry must
        // see the error instead of a short, successful listing.
        let mut saw_error = false;
        for _ in 0..8 {
            if let Err(err) = oio::List::next(&mut lister).await {
                assert!(err.is_temporary());
                saw_error = true;
                break;
            }
        }
        assert!(
            saw_error,
            "the error must reach a caller that does not retry"
        );
    }
}
