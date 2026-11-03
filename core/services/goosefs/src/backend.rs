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
use std::sync::Arc;

use log::debug;

use super::GOOSEFS_SCHEME;
use super::config::GoosefsConfig;
use super::core::GoosefsCore;
use super::deleter::GoosefsDeleter;
use super::lister::GoosefsLister;
use super::reader::*;
use super::writer::GoosefsWriter;
use super::writer::GoosefsWriters;
use opendal_core::raw::*;
use opendal_core::*;

/// [GooseFS](https://cloud.tencent.com/product/goosefs) services support via native gRPC.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct GoosefsBuilder {
    pub(super) config: GoosefsConfig,
}

impl Debug for GoosefsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoosefsBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl GoosefsBuilder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };
        self
    }

    /// Set master address(es).
    ///
    /// Single master: `"10.0.0.1:9200"`
    /// HA (comma-separated): `"10.0.0.1:9200,10.0.0.2:9200,10.0.0.3:9200"`
    ///
    /// This is the lowest-priority source: `build()` uses it only when
    /// neither `GOOSEFS_MASTER_ADDR` nor `goosefs-site.properties` declares a
    /// master address, and fails with `ConfigInvalid` when no source supplies
    /// one. See [`crate::GoosefsConfig::master_addr`] for the full resolution
    /// order and its rationale.
    pub fn master_addr(mut self, addr: &str) -> Self {
        if !addr.is_empty() {
            self.config.master_addr = Some(addr.to_string());
        }
        self
    }

    /// Set block size for new files (bytes).
    pub fn block_size(mut self, size: u64) -> Self {
        self.config.block_size = Some(size);
        self
    }

    /// Set chunk size for streaming RPCs (bytes).
    pub fn chunk_size(mut self, size: u64) -> Self {
        self.config.chunk_size = Some(size);
        self
    }

    /// Set default write type.
    ///
    /// Values: `"must_cache"`, `"try_cache"`, `"cache_through"`, `"through"`,
    /// `"async_through"`. Matching is case-insensitive. `build()` fails with
    /// [`ErrorKind::ConfigInvalid`] when the value is not one of these.
    pub fn write_type(mut self, wt: &str) -> Self {
        if !wt.is_empty() {
            self.config.write_type = Some(wt.to_string());
        }
        self
    }

    /// Set authentication type.
    ///
    /// Values: `"nosasl"`, `"simple"` (default: `"simple"`).
    /// - `"nosasl"` — skip SASL authentication entirely.
    /// - `"simple"` — PLAIN SASL with username (server does not verify password).
    pub fn auth_type(mut self, auth_type: &str) -> Self {
        if !auth_type.is_empty() {
            self.config.auth_type = Some(auth_type.to_string());
        }
        self
    }

    /// Set authentication username.
    ///
    /// Used in SIMPLE mode as the login identity.
    /// Default: current OS user (`$USER` / `$USERNAME`).
    pub fn auth_username(mut self, username: &str) -> Self {
        if !username.is_empty() {
            self.config.auth_username = Some(username.to_string());
        }
        self
    }
}

/// The source that supplied the master addresses of an auto-loaded SDK config.
///
/// `goosefs_sdk::config::GoosefsConfig::from_properties_auto()` always returns
/// a master address: when neither `GOOSEFS_MASTER_ADDR` nor
/// `goosefs-site.properties` declares one, it keeps the SDK default
/// `127.0.0.1:9200`. The loaded value therefore cannot tell "a real source
/// configured this" from "nothing configured this", and the builder has to
/// ask which source actually spoke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MasterAddrSource {
    /// `GOOSEFS_MASTER_ADDR` supplied the addresses.
    Env,
    /// `goosefs-site.properties` declared `goosefs.master.rpc.addresses` or
    /// `goosefs.master.hostname`.
    SiteProperties,
    /// Neither spoke; the loaded addresses are the SDK defaults.
    Unset,
}

/// Apply the master-address resolution order to an auto-loaded SDK config.
///
/// `goosefs-site.properties` carries a deployment's whole HA master list,
/// which a URI authority such as `goosefs://host:9200/path` cannot express, so
/// a site file that declares masters outranks `explicit` (the `master_addr`
/// config key, whether it arrived from the builder, the option map, or the URI
/// authority). `GOOSEFS_MASTER_ADDR` stays on top as the per-process override:
///
/// ```text
/// GOOSEFS_MASTER_ADDR  >  goosefs-site.properties  >  master_addr
/// ```
///
/// `from_properties_auto()` already resolved the first two sources, so
/// `explicit` is written into `config` only when neither spoke. Returns
/// `ConfigInvalid` when `explicit` holds no address after trimming, and when
/// no source at all supplies one.
fn apply_master_addr_precedence(
    config: &mut goosefs_sdk::config::GoosefsConfig,
    explicit: Option<&str>,
) -> Result<()> {
    let explicit_addrs = match explicit {
        Some(master_addr) => {
            let addrs: Vec<String> = master_addr
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if addrs.is_empty() {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "master_addr is empty after trimming",
                )
                .with_operation("Builder::build")
                .with_context("service", GOOSEFS_SCHEME));
            }
            Some(addrs)
        }
        None => None,
    };

    match detect_master_addr_source() {
        MasterAddrSource::Unset => {
            let Some(addrs) = explicit_addrs else {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "master_addr is not configured: set it via GoosefsBuilder::master_addr(...), \
                     the `master_addr` config key, the GOOSEFS_MASTER_ADDR env var, \
                     or `goosefs.master.hostname`/`goosefs.master.rpc.addresses` in goosefs-site.properties",
                )
                .with_operation("Builder::build")
                .with_context("service", GOOSEFS_SCHEME));
            };

            config.master_addr = addrs[0].clone();
            config.master_addrs = if addrs.len() > 1 { addrs } else { Vec::new() };
        }
        source => {
            if let Some(addrs) = explicit_addrs {
                debug!(
                    "GoosefsBuilder ignores master_addr {addrs:?}: {source:?} supplies {} (addrs={:?})",
                    config.master_addr, config.master_addrs
                );
            }
        }
    }

    Ok(())
}

/// Report which source supplied the master addresses that
/// `from_properties_auto()` loaded.
fn detect_master_addr_source() -> MasterAddrSource {
    // Probe `GOOSEFS_MASTER_ADDR` through the SDK's own parser so that the
    // `gfs://h1:9200,h2:9200/root` URI form and the bare comma list are
    // recognised exactly as `from_properties_auto()` recognises them. Blanking
    // the address first makes an unset, empty, or malformed value observable:
    // in those cases `apply_env()` leaves the field untouched.
    let probe = goosefs_sdk::config::GoosefsConfig {
        master_addr: String::new(),
        master_addrs: Vec::new(),
        ..Default::default()
    }
    .apply_env();
    if !probe.master_addr.is_empty() || !probe.master_addrs.is_empty() {
        return MasterAddrSource::Env;
    }

    // The SDK searches `$GOOSEFS_CONFIG_FILE`, `$GOOSEFS_CONF_DIR`,
    // `$GOOSEFS_HOME/conf`, `~/.goosefs`, and `/etc/goosefs` for
    // `goosefs-site.properties`. An unreadable file is left to
    // `from_properties_auto()`, which already failed the build above.
    if let Some(path) = goosefs_sdk::config::discover_config_file()
        && let Ok(content) = std::fs::read_to_string(&path)
        && site_properties_declare_master(&content)
    {
        return MasterAddrSource::SiteProperties;
    }

    MasterAddrSource::Unset
}

/// Report whether `goosefs-site.properties` declares a master address.
///
/// The SDK's properties parser is private, so key lookup repeats its
/// documented Java `Properties.load()` rules here: `#` and `!` start a comment
/// line, the first `=` (else the first `:`) separates key from value, and the
/// last assignment of a key wins. `goosefs.master.rpc.addresses` shadows
/// `goosefs.master.hostname` whenever it is present, matching the order in
/// which the SDK consults the two keys.
fn site_properties_declare_master(content: &str) -> bool {
    let mut addresses = None;
    let mut hostname = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let Some(sep) = line.find('=').or_else(|| line.find(':')) else {
            continue;
        };

        match line[..sep].trim() {
            "goosefs.master.rpc.addresses" => addresses = Some(line[sep + 1..].trim()),
            "goosefs.master.hostname" => hostname = Some(line[sep + 1..].trim()),
            _ => {}
        }
    }

    match addresses {
        Some(addresses) => addresses.split(',').any(|addr| !addr.trim().is_empty()),
        None => hostname.is_some_and(|hostname| !hostname.is_empty()),
    }
}

impl Builder for GoosefsBuilder {
    type Config = GoosefsConfig;

    /// Build the backend and return a GoosefsBackend.
    fn build(self) -> Result<impl Service> {
        debug!("GoosefsBuilder::build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("GoosefsBuilder use root {}", root);

        // ── Step 1: establish the base SDK config ─────────────────────────────
        //
        // We follow the same priority chain that `FileSystemContext::connect`
        // (and its `ConfigRefresher`) uses — see
        // https://github.com/Tencent/tencent-goosefs-rust-sdk/blob/main/docs/CLIENT_CONFIGURATION.md
        // §1 "Configuration Loading Priority":
        //
        //   defaults  <  goosefs-site.properties  <  GOOSEFS_* env vars
        //
        // `GoosefsConfig::from_properties_auto()` already implements this
        // chain and is the *same* function the SDK calls every 60s to refresh
        // the transparent-acceleration switches. Using it here keeps the
        // initial OpenDAL build and the in-process hot-reload semantically
        // aligned — users who deploy `goosefs-site.properties` get the exact
        // same config from both paths.
        //
        // Failure policy:
        //   * no properties file found  → silently uses defaults + env
        //     (`from_properties_auto` handles this internally)
        //   * properties file present but malformed → hard-fail
        //     (broken config must not be silently dropped)
        //
        // Builder-explicit fields overlay this config afterwards. The master
        // address is the one exception: Step 2 keeps the properties/env value
        // on top, see `apply_master_addr_precedence`.
        let mut goosefs_config = goosefs_sdk::config::GoosefsConfig::from_properties_auto()
            .map_err(|e| {
                Error::new(
                    ErrorKind::ConfigInvalid,
                    format!("failed to auto-load goosefs config: {e}"),
                )
                .with_operation("Builder::build")
                .with_context("service", GOOSEFS_SCHEME)
            })?;

        // Root always comes from OpenDAL (it's an OpenDAL-layer concept).
        goosefs_config.root = root.clone();

        // ── Step 2: resolve the master address ────────────────────────────────
        apply_master_addr_precedence(&mut goosefs_config, self.config.master_addr.as_deref())?;
        debug!(
            "GoosefsBuilder use master_addr {} (addrs={:?})",
            goosefs_config.master_addr, goosefs_config.master_addrs
        );

        // ── Step 3: overlay the remaining builder-explicit fields ─────────────

        if let Some(block_size) = self.config.block_size {
            goosefs_config.block_size = block_size;
        }
        if let Some(chunk_size) = self.config.chunk_size {
            goosefs_config.chunk_size = chunk_size;
        }

        // Parse write_type through the SDK so unknown values fail instead of
        // silently becoming MUST_CACHE. `with_write_type_str` is
        // case-insensitive, matching GooseFS `WritePType::valueOf`.
        if let Some(ref wt) = self.config.write_type {
            goosefs_config = goosefs_config.with_write_type_str(wt).map_err(|e| {
                Error::new(
                    ErrorKind::ConfigInvalid,
                    format!("invalid write_type: {}", e),
                )
                .with_operation("Builder::build")
                .with_context("service", GOOSEFS_SCHEME)
            })?;
        }

        // Parse auth_type string → goosefs_sdk::auth::AuthType
        if let Some(ref auth_type_str) = self.config.auth_type {
            goosefs_config = goosefs_config
                .with_auth_type_str(auth_type_str)
                .map_err(|e| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        format!("invalid auth_type: {}", e),
                    )
                    .with_operation("Builder::build")
                    .with_context("service", GOOSEFS_SCHEME)
                })?;
        }

        if let Some(ref auth_username) = self.config.auth_username {
            goosefs_config = goosefs_config.with_auth_username(auth_username);
        }

        // ── Step 4: validate the final merged config ──────────────────────────
        goosefs_config.validate().map_err(|e| {
            Error::new(
                ErrorKind::ConfigInvalid,
                format!("invalid goosefs config: {e}"),
            )
            .with_operation("Builder::build")
            .with_context("service", GOOSEFS_SCHEME)
        })?;

        Ok(GoosefsBackend {
            core: Arc::new(GoosefsCore::new(
                ServiceInfo::new(GOOSEFS_SCHEME, &root, ""),
                Capability {
                    stat: true,
                    read: true,
                    write: true,
                    write_can_multi: true,
                    // Authoritative Create: write-via-temp then
                    // GoosefsCore::rename(..., if_not_exists=true), backed by Master
                    // no-replace rename. Not CreateFile on the final path
                    // (writes go to .opendal.tmp.*).
                    write_with_if_not_exists: true,
                    create_dir: true,
                    delete: true,
                    list: true,
                    rename: true,
                    rename_with_if_not_exists: true,
                    shared: true,
                    ..Default::default()
                },
                root,
                goosefs_config,
            )),
        })
    }
}

#[derive(Debug, Clone)]
pub struct GoosefsBackend {
    pub(crate) core: Arc<GoosefsCore>,
}

impl Service for GoosefsBackend {
    type Reader = oio::StreamReader<GoosefsReader>;
    type Writer = GoosefsWriters;
    type Lister = oio::PageLister<GoosefsLister>;
    type Deleter = oio::OneShotDeleter<GoosefsDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.core.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.core.create_dir(path).await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let file_info = self.core.get_status(path).await?;
        Ok(RpStat::new(self.core.file_info_to_metadata(&file_info)?))
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<GoosefsReader> = {
            Ok(oio::StreamReader::new(GoosefsReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: GoosefsWriters = {
            let w = GoosefsWriter::new(self.core.clone(), args.clone(), path.to_string());
            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<GoosefsDeleter> = {
            Ok(oio::OneShotDeleter::new(GoosefsDeleter::new(
                self.core.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<GoosefsLister> = {
            let l = GoosefsLister::new(self.core.clone(), path);
            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.core.rename(from, to, args.if_not_exists()).await?;
        Ok(RpRename::default())
    }

    async fn presign(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpPresign,
    ) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goosefs_sdk::config::ENV_CONF_DIR;
    use goosefs_sdk::config::ENV_CONFIG_FILE;
    use goosefs_sdk::config::ENV_HOME;
    use goosefs_sdk::config::ENV_MASTER_ADDR;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// `GOOSEFS_*` vars are process-global; serialize every test that
    /// exercises master-address resolution through them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    const SITE_HA: &str =
        "goosefs.master.rpc.addresses=172.31.5.10:9200,172.31.5.2:9200,172.31.5.11:9200\n";
    const SITE_NO_MASTER: &str = "goosefs.user.block.size.bytes.default=4MB\n";

    /// Write a `goosefs-site.properties` fixture, point `$GOOSEFS_CONFIG_FILE`
    /// at it, and clear the other resolution inputs.
    ///
    /// A file is written even for the "no master keys" cases so that discovery
    /// stops at `$GOOSEFS_CONFIG_FILE` instead of reaching a `~/.goosefs` or
    /// `/etc/goosefs` file that happens to exist on the host.
    fn site_properties(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "opendal_goosefs_{}_{name}_site.properties",
            std::process::id()
        ));
        std::fs::write(&path, content).expect("write goosefs-site.properties");

        unsafe {
            std::env::set_var(ENV_CONFIG_FILE, &path);
            std::env::remove_var(ENV_CONF_DIR);
            std::env::remove_var(ENV_HOME);
            std::env::remove_var(ENV_MASTER_ADDR);
        }

        path
    }

    fn clear_site_properties(path: &PathBuf) {
        unsafe {
            std::env::remove_var(ENV_CONFIG_FILE);
            std::env::remove_var(ENV_MASTER_ADDR);
        }
        let _ = std::fs::remove_file(path);
    }

    /// Resolve master addresses exactly as `build()` does: auto-load, then
    /// apply the precedence.
    fn resolve(explicit: Option<&str>) -> Result<(String, Vec<String>)> {
        let mut config = goosefs_sdk::config::GoosefsConfig::from_properties_auto()
            .expect("auto-load must succeed");
        apply_master_addr_precedence(&mut config, explicit)?;
        Ok((config.master_addr, config.master_addrs))
    }

    /// A site file that declares `goosefs.master.rpc.addresses` supplies the
    /// HA list even when the URI authority carries an unreachable address.
    ///
    /// This is the reported failure: with `GOOSEFS_CONFIG_FILE` pointing at a
    /// file that lists the real masters, the client still dialed the dummy
    /// address from `goosefs://192.0.2.5:9999/...`.
    #[test]
    fn site_properties_outrank_uri_authority() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = site_properties("site_over_uri", SITE_HA);

        let (master_addr, master_addrs) =
            resolve(Some("192.0.2.5:9999")).expect("resolution must succeed");

        assert_eq!(master_addr, "172.31.5.10:9200");
        assert_eq!(
            master_addrs,
            vec![
                "172.31.5.10:9200".to_string(),
                "172.31.5.2:9200".to_string(),
                "172.31.5.11:9200".to_string(),
            ]
        );

        clear_site_properties(&path);
    }

    /// Without a URI authority or `GOOSEFS_MASTER_ADDR`, a declared site file
    /// still supplies the masters — `goosefs:///path` must not be rejected for
    /// a missing host.
    #[test]
    fn site_properties_resolve_without_explicit_addr() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = site_properties("site_only", SITE_HA);

        let (master_addr, master_addrs) = resolve(None).expect("resolution must succeed");

        assert_eq!(master_addr, "172.31.5.10:9200");
        assert_eq!(master_addrs.len(), 3);

        clear_site_properties(&path);
    }

    /// `GOOSEFS_MASTER_ADDR` is the per-process override and outranks both the
    /// site file and the URI authority.
    #[test]
    fn env_master_addr_outranks_site_properties_and_uri_authority() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = site_properties("env_over_site", SITE_HA);
        unsafe { std::env::set_var(ENV_MASTER_ADDR, "10.0.0.7:9200") };

        let (master_addr, master_addrs) =
            resolve(Some("192.0.2.5:9999")).expect("resolution must succeed");

        assert_eq!(master_addr, "10.0.0.7:9200");
        assert!(
            master_addrs.is_empty(),
            "a single env address must not populate the HA list, got {master_addrs:?}"
        );

        clear_site_properties(&path);
    }

    /// A site file without master keys leaves `master_addr` in charge, so a
    /// URI authority or option-map address is used as before.
    #[test]
    fn explicit_addr_applies_when_no_source_declares_masters() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = site_properties("explicit_only", SITE_NO_MASTER);

        let (master_addr, master_addrs) =
            resolve(Some("10.0.0.1:9200,10.0.0.2:9200")).expect("resolution must succeed");

        assert_eq!(master_addr, "10.0.0.1:9200");
        assert_eq!(
            master_addrs,
            vec!["10.0.0.1:9200".to_string(), "10.0.0.2:9200".to_string()]
        );

        clear_site_properties(&path);
    }

    /// When no source declares a master address, `build()` must fail instead
    /// of silently dialing the SDK default `127.0.0.1:9200`.
    #[test]
    fn missing_master_addr_fails() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = site_properties("missing_master", SITE_NO_MASTER);

        let err = resolve(None).expect_err("resolution must fail without any master address");
        assert_eq!(err.kind(), ErrorKind::ConfigInvalid);
        assert!(
            err.to_string().contains("master_addr is not configured"),
            "unexpected error message: {err}"
        );

        clear_site_properties(&path);
    }

    /// `GOOSEFS_MASTER_ADDR` also accepts the SDK's `gfs://` URI form, and
    /// resolution must recognise it as a real source rather than falling
    /// through to `master_addr`.
    #[test]
    fn env_master_addr_accepts_gfs_uri_form() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = site_properties("env_gfs_uri", SITE_NO_MASTER);
        unsafe { std::env::set_var(ENV_MASTER_ADDR, "gfs://10.0.0.1:9200,10.0.0.2:9200/lance") };

        let (master_addr, master_addrs) =
            resolve(Some("192.0.2.5:9999")).expect("resolution must succeed");

        assert_eq!(master_addr, "10.0.0.1:9200");
        assert_eq!(master_addrs.len(), 2);

        clear_site_properties(&path);
    }

    #[test]
    fn site_properties_declare_master_reads_both_keys() {
        assert!(site_properties_declare_master(
            "goosefs.master.rpc.addresses=10.0.0.1:9200,10.0.0.2:9200\n"
        ));
        assert!(site_properties_declare_master(
            "goosefs.master.hostname = master-1\ngoosefs.master.rpc.port=9200\n"
        ));
        // Java `Properties.load()` also accepts `:` as the separator.
        assert!(site_properties_declare_master(
            "goosefs.master.hostname:master-1\n"
        ));
    }

    #[test]
    fn site_properties_declare_master_rejects_absent_and_blank_keys() {
        assert!(!site_properties_declare_master(SITE_NO_MASTER));
        assert!(!site_properties_declare_master(
            "# goosefs.master.rpc.addresses=10.0.0.1:9200\n! goosefs.master.hostname=master-1\n"
        ));
        assert!(!site_properties_declare_master(
            "goosefs.master.hostname=\n"
        ));
        assert!(!site_properties_declare_master(
            "goosefs.master.rpc.addresses= , ,\n"
        ));
        // The SDK consults `addresses` first and never falls back to
        // `hostname` once the key is present, so a blank list declares
        // nothing here either.
        assert!(!site_properties_declare_master(
            "goosefs.master.rpc.addresses=\ngoosefs.master.hostname=master-1\n"
        ));
    }

    #[test]
    fn test_builder_build() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let builder = GoosefsBuilder::default()
            .root("/data")
            .master_addr("127.0.0.1:9200")
            .build();
        assert!(builder.is_ok());
    }

    #[test]
    fn test_builder_ha() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let builder = GoosefsBuilder::default()
            .root("/data")
            .master_addr("10.0.0.1:9200,10.0.0.2:9200,10.0.0.3:9200")
            .build();
        assert!(builder.is_ok());
    }

    /// `master_addr` is mandatory — `build()` must fail with `ConfigInvalid`
    /// when it cannot be resolved from any source. This test exercises the
    /// "explicitly blank" form (empty / whitespace / comma-only), which is
    /// environment-independent: Step 2 short-circuits on a blank override
    /// before any auto-load value can rescue it.
    #[test]
    fn test_builder_blank_master_addr_fails() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let err = GoosefsBuilder::default()
            .root("/data")
            .master_addr("   ,  , ")
            .build()
            .expect_err("build must fail when master_addr is blank");
        assert_eq!(err.kind(), ErrorKind::ConfigInvalid);
        assert!(
            err.to_string().contains("master_addr is empty"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn test_builder_unknown_write_type_fails() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        for wt in ["INVALID_WT", "cache_throughh"] {
            let err = GoosefsBuilder::default()
                .root("/data")
                .master_addr("127.0.0.1:9200")
                .write_type(wt)
                .build()
                .expect_err("build must fail on unknown write_type");
            assert_eq!(err.kind(), ErrorKind::ConfigInvalid);
            assert!(
                err.to_string().contains("write_type"),
                "unexpected error message for {wt}: {err}"
            );
        }
    }

    #[test]
    fn test_builder_write_type_is_case_insensitive() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let backend = GoosefsBuilder::default()
            .root("/data")
            .master_addr("127.0.0.1:9200")
            .write_type("CACHE_THROUGH")
            .build();
        assert!(backend.is_ok());
    }

    #[test]
    fn test_capability_rename_with_if_not_exists() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let backend = GoosefsBuilder::default()
            .root("/data")
            .master_addr("127.0.0.1:9200")
            .build()
            .expect("build");
        let cap = backend.capability();
        assert!(cap.write_with_if_not_exists);
        assert!(
            cap.rename_with_if_not_exists,
            "rename_with_if_not_exists must be declared for Create publish"
        );
    }
}
