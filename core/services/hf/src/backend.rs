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

use std::path::PathBuf;
use std::sync::Arc;

use log::debug;

use super::HF_SCHEME;
use super::config::HfConfig;
use super::core::HfCore;
use super::core::HfDownloadMode;
use super::core::{HfRepo, HfRepoType};
use super::deleter::HfDeleter;
use super::lister::HfLister;
use super::reader::*;
use super::writer::HfLazyWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// [Hugging Face](https://huggingface.co/docs/huggingface_hub/package_reference/hf_api)'s API support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct HfBuilder {
    pub(super) config: HfConfig,
}

impl HfBuilder {
    /// Set repo type of this backend. Default is model.
    ///
    /// Available values:
    /// - model
    /// - dataset
    /// - datasets (alias for dataset)
    /// - space
    /// - bucket
    ///
    /// [Reference](https://huggingface.co/docs/hub/repositories)
    pub fn repo_type(mut self, repo_type: &str) -> Self {
        if !repo_type.is_empty()
            && let Ok(rt) = HfRepoType::parse(repo_type)
        {
            self.config.repo_type = Some(rt);
        }
        self
    }

    /// Set repo id of this backend. This is required.
    ///
    /// Repo id consists of the account name and the repository name.
    ///
    /// For example, model's repo id looks like:
    /// - meta-llama/Llama-2-7b
    ///
    /// Dataset's repo id looks like:
    /// - databricks/databricks-dolly-15k
    pub fn repo_id(mut self, repo_id: &str) -> Self {
        if !repo_id.is_empty() {
            self.config.repo_id = Some(repo_id.to_string());
        }
        self
    }

    /// Set revision of this backend. Default is main.
    ///
    /// Revision can be a branch name or a commit hash.
    ///
    /// For example, revision can be:
    /// - main
    /// - 1d0c4eb
    pub fn revision(mut self, revision: &str) -> Self {
        if !revision.is_empty() {
            self.config.revision = Some(revision.to_string());
        }
        self
    }

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

    /// Set the token of this backend.
    ///
    /// This is optional.
    pub fn token(mut self, token: &str) -> Self {
        if !token.is_empty() {
            self.config.token = Some(token.to_string());
        }
        self
    }

    /// Set the download mode. Either `xet` (default) or `http`.
    ///
    /// - `xet`: uses the XET protocol for downloads (default).
    /// - `http`: plain HTTP download, following the redirect from the server.
    ///
    /// When this is not set explicitly, the download mode is resolved from the
    /// `HF_HUB_DISABLE_XET` environment variable (the same variable used by
    /// `huggingface_hub`): if it is set to a non-empty value, the mode is forced
    /// to `http`; otherwise it defaults to `xet`. An explicit value set here
    /// always takes precedence over the environment variable.
    ///
    /// See <https://huggingface.co/docs/huggingface_hub/package_reference/environment_variables#hfhubdisablexet>.
    pub fn download_mode(mut self, mode: &str) -> Self {
        if !mode.is_empty()
            && let Ok(m) = HfDownloadMode::parse(mode)
        {
            self.config.download_mode = Some(m);
        }
        self
    }

    /// Enable caching of resolved HTTP download addresses and XET file metadata.
    ///
    /// Defaults to `false`: each new reader resolves through the Hub. An XET-mode
    /// reader retains the XET metadata returned by its first read for its lifetime,
    /// even when this option is disabled. Its subsequent ranges use that file
    /// version. Create a new reader to resolve the path again. HTTP reads resolve
    /// each range.
    ///
    /// Set to `true` to share resolve results across readers on the same backend.
    /// HTTP addresses refresh near expiry.
    ///
    /// Enable this only when previously written files are not modified. Changed
    /// files can remain invisible while cached results are reused, including
    /// changes from other clients or a floating repository revision. Issued
    /// download URLs can remain usable until expiry after Hub permissions change.
    /// Separate authorization identities must use separately constructed backends.
    pub fn enable_resolve_cache(mut self, enabled: bool) -> Self {
        self.config.enable_resolve_cache = enabled;
        self
    }

    /// Set the Hub base URL.
    ///
    /// Configure this when your organization uses a
    /// [Private Hub](https://huggingface.co/enterprise).
    ///
    /// The default is `https://huggingface.co`.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.to_string());
        }
        self
    }

    /// Disable loading config from the environment.
    ///
    /// See [`HfConfig::disable_config_load`] for details.
    pub fn disable_config_load(mut self) -> Self {
        self.config.disable_config_load = true;
        self
    }

    /// Resolve the Hub base URL: an explicit config value wins, then
    /// `HF_ENDPOINT` unless disable_config_load is set, then the public Hub.
    /// A trailing slash is trimmed because every URL is built by appending
    /// `/api/...` to this, and HF answers the resulting `//api/...` with a 404.
    fn hf_endpoint(&self) -> String {
        self.config
            .endpoint
            .clone()
            .or_else(|| {
                if self.config.disable_config_load {
                    None
                } else {
                    std::env::var("HF_ENDPOINT").ok()
                }
            })
            .unwrap_or_else(|| "https://huggingface.co".to_string())
            .trim_end_matches('/')
            .to_string()
    }

    /// Resolve the download mode: an explicit config value wins; otherwise, unless
    /// disable_config_load is set, a set, non-empty HF_HUB_DISABLE_XET (a
    /// huggingface_hub env var) forces http; default Xet.
    fn hf_download_mode(&self) -> HfDownloadMode {
        if let Some(mode) = self.config.download_mode {
            return mode;
        }
        if !self.config.disable_config_load
            && let Ok(val) = std::env::var("HF_HUB_DISABLE_XET")
            && !val.is_empty()
        {
            return HfDownloadMode::Http;
        }
        HfDownloadMode::default()
    }

    fn hf_home() -> Option<PathBuf> {
        if let Ok(h) = std::env::var("HF_HOME") {
            return Some(PathBuf::from(h));
        }
        if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            return Some(PathBuf::from(xdg).join("huggingface"));
        }
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".cache/huggingface"))
    }

    /// Resolve the authentication token using the same priority order as hf-hub:
    /// explicit config → disable_config_load → HF_HUB_DISABLE_IMPLICIT_TOKEN check →
    /// HF_TOKEN env → token file.
    fn hf_token(&self) -> Option<String> {
        if let Some(t) = self.config.token.clone() {
            return Some(t);
        }
        if self.config.disable_config_load {
            return None;
        }
        if let Ok(val) = std::env::var("HF_HUB_DISABLE_IMPLICIT_TOKEN")
            && !val.is_empty()
        {
            return None;
        }
        if let Ok(t) = std::env::var("HF_TOKEN")
            && !t.is_empty()
        {
            return Some(t);
        }
        let token_path = if let Ok(p) = std::env::var("HF_TOKEN_PATH") {
            Some(PathBuf::from(p))
        } else {
            Self::hf_home().map(|h| h.join("token"))
        };
        token_path
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

impl Builder for HfBuilder {
    type Config = HfConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let token = self.hf_token();
        let endpoint = self.hf_endpoint();
        let download_mode = self.hf_download_mode();

        let repo_type = self.config.repo_type.ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "repo_type is required")
                .with_operation("Builder::build")
                .with_context("service", HF_SCHEME)
        })?;
        debug!("backend use repo_type: {:?}", repo_type);

        let repo_id = self.config.repo_id.ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "repo_id is required")
                .with_operation("Builder::build")
                .with_context("service", HF_SCHEME)
        })?;
        debug!("backend use repo_id: {}", repo_id);

        let revision = match &self.config.revision {
            Some(revision) => revision.clone(),
            None => "main".to_string(),
        };
        debug!("backend use revision: {}", revision);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root: {}", root);
        debug!("backend use token: {}", token.is_some());
        debug!("backend use endpoint: {}", endpoint);
        debug!("backend use download_mode: {:?}", download_mode);

        let info = ServiceInfo::new(HF_SCHEME, "", "");
        let capability = Capability {
            stat: true,
            read: true,
            write: token.is_some(),
            write_can_multi: token.is_some(),
            delete: token.is_some(),
            delete_max_size: Some(100),
            list: true,
            list_with_recursive: true,
            shared: true,
            ..Default::default()
        };

        let repo = HfRepo::new(repo_type, repo_id, Some(revision.clone()));
        debug!("backend repo uri: {:?}", repo.uri(&root, ""));

        let mut core = HfCore::build(info, capability, repo, root, token, endpoint, download_mode)?;
        core.enable_resolve_cache = self.config.enable_resolve_cache;
        Ok(HfBackend {
            core: Arc::new(core),
        })
    }
}

/// Backend for Hugging Face service
#[derive(Debug, Clone)]
pub struct HfBackend {
    pub(crate) core: Arc<HfCore>,
}

impl Service for HfBackend {
    type Reader = oio::StreamReader<HfReader>;
    type Writer = HfLazyWriter;
    type Lister = oio::PageLister<HfLister>;
    type Deleter = oio::BatchDeleter<HfDeleter>;
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
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        // Stat root always returns a DIR.
        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        // Buckets have no git directory entries; treat any trailing-slash path as a virtual dir.
        if self.core.repo.is_bucket() && path.ends_with('/') {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let info = self.core.path_info(ctx, path).await?;
        Ok(RpStat::new(info.metadata()?))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<HfReader> = {
            Ok(oio::StreamReader::new(HfReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<HfLister> = {
            let lister = HfLister::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.recursive(),
            );
            Ok(oio::PageLister::new(lister))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, _args: OpWrite) -> Result<Self::Writer> {
        Ok(HfLazyWriter::new(
            self.core.clone(),
            ctx.clone(),
            path.to_string(),
        ))
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::BatchDeleter<HfDeleter> = {
            let deleter = HfDeleter::new(self.core.clone(), ctx.clone());
            let max_batch_size = self.core.capability.delete_max_size;
            Ok(oio::BatchDeleter::new(deleter, max_batch_size))
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
        _from: &str,
        _to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
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
pub(super) mod test_utils {
    use std::sync::Arc;

    use super::super::core::{HfCore, HfDownloadMode};
    use super::super::core::{HfRepo, HfRepoType};
    use super::HfBuilder;
    use opendal_core::Capability;
    use opendal_core::HttpTransporter;
    use opendal_core::OperationContext;
    use opendal_core::Operator;
    use opendal_core::raw::ServiceInfo;

    fn finish_operator(op: Operator) -> Operator {
        let transport =
            HttpTransporter::new(opendal_http_transport_reqwest::ReqwestTransport::default());
        op.with_context(OperationContext::new().with_http_transport(transport))
    }

    pub fn mbpp_operator() -> Operator {
        let op = Operator::new(
            HfBuilder::default()
                .repo_type("dataset")
                .repo_id("google-research-datasets/mbpp"),
        )
        .unwrap();
        finish_operator(op)
    }

    /// Same public dataset as [`mbpp_operator`], but with the repo id cased
    /// differently from the canonical `google-research-datasets/mbpp`. HF
    /// answers every request for it with a `307` to the canonical URL.
    pub fn miscased_mbpp_operator() -> Operator {
        let op = Operator::new(
            HfBuilder::default()
                .repo_type("dataset")
                .repo_id("Google-Research-Datasets/MBPP"),
        )
        .unwrap();
        finish_operator(op)
    }

    pub fn testing_dataset_core() -> Arc<HfCore> {
        let repo_id = std::env::var("HF_OPENDAL_DATASET").expect("HF_OPENDAL_DATASET must be set");
        let token = std::env::var("HF_OPENDAL_TOKEN").expect("HF_OPENDAL_TOKEN must be set");

        let info = ServiceInfo::new("hf", "", "");
        let capability = Capability {
            read: true,
            write: true,
            delete: true,
            ..Default::default()
        };

        let repo = HfRepo::new(HfRepoType::Dataset, repo_id, Some("main".to_string()));

        Arc::new(
            HfCore::build(
                info,
                capability,
                repo,
                "/".to_string(),
                Some(token),
                "https://huggingface.co".to_string(),
                HfDownloadMode::Xet,
            )
            .expect("failed to build HfCore"),
        )
    }

    pub fn testing_bucket_operator() -> Operator {
        let repo_id = std::env::var("HF_OPENDAL_BUCKET").expect("HF_OPENDAL_BUCKET must be set");
        let token = std::env::var("HF_OPENDAL_TOKEN").expect("HF_OPENDAL_TOKEN must be set");
        let op = Operator::new(
            HfBuilder::default()
                .repo_type("bucket")
                .repo_id(&repo_id)
                .token(&token),
        )
        .unwrap();
        finish_operator(op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env vars are process-global; serialize all tests that mutate them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn builder_with_token(token: &str) -> HfBuilder {
        HfBuilder::default().token(token)
    }

    fn builder_no_token() -> HfBuilder {
        HfBuilder::default()
    }

    #[test]
    fn hf_token_config_takes_priority_over_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_TOKEN", "env-token") };
        let result = builder_with_token("config-token").hf_token();
        unsafe { std::env::remove_var("HF_TOKEN") };
        assert_eq!(result.as_deref(), Some("config-token"));
    }

    #[test]
    fn hf_token_reads_hf_token_env_var() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_IMPLICIT_TOKEN") };
        unsafe { std::env::remove_var("HF_TOKEN_PATH") };
        unsafe { std::env::set_var("HF_TOKEN", "my-env-token") };
        let result = builder_no_token().hf_token();
        unsafe { std::env::remove_var("HF_TOKEN") };
        assert_eq!(result.as_deref(), Some("my-env-token"));
    }

    #[test]
    fn hf_token_disable_flag_suppresses_discovery() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_HUB_DISABLE_IMPLICIT_TOKEN", "1") };
        unsafe { std::env::set_var("HF_TOKEN", "my-env-token") };
        let result = builder_no_token().hf_token();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_IMPLICIT_TOKEN") };
        unsafe { std::env::remove_var("HF_TOKEN") };
        assert_eq!(result, None);
    }

    #[test]
    fn hf_token_disable_config_load_ignores_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join("opendal-hf-disable-config-load");
        std::fs::create_dir_all(dir.join("huggingface")).unwrap();
        std::fs::write(dir.join("token"), "file-token").unwrap();
        std::fs::write(dir.join("huggingface/token"), "xdg-token").unwrap();

        // Each source alone yields a token, unless config loading is disabled.
        let keys = ["HF_TOKEN", "HF_TOKEN_PATH", "HF_HOME", "XDG_CACHE_HOME"];
        let values = [
            std::ffi::OsString::from("env-token"),
            dir.join("token").into(),
            dir.clone().into(),
            dir.clone().into(),
        ];
        let saved = keys.map(std::env::var_os);
        unsafe { std::env::remove_var("HF_HUB_DISABLE_IMPLICIT_TOKEN") };
        let mut results = Vec::new();
        for (key, value) in keys.iter().zip(&values) {
            for key in keys {
                unsafe { std::env::remove_var(key) };
            }
            unsafe { std::env::set_var(key, value) };
            results.push((
                *key,
                builder_no_token().hf_token(),
                builder_no_token().disable_config_load().hf_token(),
            ));
        }
        let explicit = builder_with_token("config-token")
            .disable_config_load()
            .hf_token();
        for (key, value) in keys.iter().zip(saved) {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        std::fs::remove_dir_all(&dir).ok();

        for (key, loaded, disabled) in results {
            assert!(loaded.is_some(), "{key} should provide a token");
            assert_eq!(disabled, None, "{key} should be ignored");
        }
        assert_eq!(explicit.as_deref(), Some("config-token"));
    }

    #[test]
    fn hf_token_reads_from_file_via_hf_token_path() {
        let _guard = ENV_LOCK.lock().unwrap();
        let token_file = std::env::temp_dir().join("opendal-hf-token-test");
        std::fs::write(&token_file, "file-token\n").unwrap();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_IMPLICIT_TOKEN") };
        unsafe { std::env::remove_var("HF_TOKEN") };
        unsafe { std::env::set_var("HF_TOKEN_PATH", &token_file) };
        let result = builder_no_token().hf_token();
        unsafe { std::env::remove_var("HF_TOKEN_PATH") };
        std::fs::remove_file(&token_file).ok();
        assert_eq!(result.as_deref(), Some("file-token"));
    }

    #[test]
    fn hf_endpoint_trims_trailing_slash() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("HF_ENDPOINT") };
        assert_eq!(
            HfBuilder::default()
                .endpoint("https://hub.example.com/")
                .hf_endpoint(),
            "https://hub.example.com"
        );
    }

    #[test]
    fn hf_endpoint_returns_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("HF_ENDPOINT") };
        let result = HfBuilder::default().hf_endpoint();
        assert_eq!(result, "https://huggingface.co");
    }

    #[test]
    fn hf_endpoint_config_takes_priority_over_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_ENDPOINT", "https://env.example.com") };
        let result = HfBuilder::default()
            .endpoint("https://config.example.com")
            .hf_endpoint();
        unsafe { std::env::remove_var("HF_ENDPOINT") };
        assert_eq!(result, "https://config.example.com");
    }

    #[test]
    fn hf_endpoint_reads_hf_endpoint_env_var() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_ENDPOINT", "https://env.example.com") };
        let result = HfBuilder::default().hf_endpoint();
        unsafe { std::env::remove_var("HF_ENDPOINT") };
        assert_eq!(result, "https://env.example.com");
    }

    #[test]
    fn hf_download_mode_defaults_to_xet() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_XET") };
        assert_eq!(HfBuilder::default().hf_download_mode(), HfDownloadMode::Xet);
    }

    #[test]
    fn hf_download_mode_disable_xet_env_forces_http() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_HUB_DISABLE_XET", "1") };
        let mode = HfBuilder::default().hf_download_mode();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_XET") };
        assert_eq!(mode, HfDownloadMode::Http);
    }

    #[test]
    fn hf_download_mode_config_takes_priority_over_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_HUB_DISABLE_XET", "1") };
        let mode = HfBuilder::default().download_mode("xet").hf_download_mode();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_XET") };
        assert_eq!(mode, HfDownloadMode::Xet);
    }

    #[test]
    fn hf_download_mode_empty_env_keeps_xet() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_HUB_DISABLE_XET", "") };
        let mode = HfBuilder::default().hf_download_mode();
        unsafe { std::env::remove_var("HF_HUB_DISABLE_XET") };
        assert_eq!(mode, HfDownloadMode::Xet);
    }

    #[test]
    fn disable_config_load_ignores_endpoint_and_download_mode_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("HF_ENDPOINT", "https://env.example.com") };
        unsafe { std::env::set_var("HF_HUB_DISABLE_XET", "1") };
        let builder = HfBuilder::default().disable_config_load();
        let endpoint = builder.hf_endpoint();
        let mode = builder.hf_download_mode();
        unsafe { std::env::remove_var("HF_ENDPOINT") };
        unsafe { std::env::remove_var("HF_HUB_DISABLE_XET") };
        assert_eq!(endpoint, "https://huggingface.co");
        assert_eq!(mode, HfDownloadMode::Xet);
    }

    #[tokio::test]
    async fn build_resolve_cache_requires_opt_in() -> Result<()> {
        use super::super::core::test_utils::MockHttpTransport;

        for enabled in [None, Some(false), Some(true)] {
            let mut builder = HfBuilder::default()
                .repo_type("model")
                .repo_id("org/repo")
                .download_mode("xet");
            if let Some(enabled) = enabled {
                builder = builder.enable_resolve_cache(enabled);
            }
            let transport = MockHttpTransport::new();
            let ctx = OperationContext::new()
                .with_http_transport(HttpTransporter::new(transport.clone()));
            let op = Operator::new(builder)?.with_context(ctx);
            for reads in 1..=2 {
                assert_eq!(op.read("plain.txt").await?.to_vec(), b"hello");
                assert_eq!(
                    transport.request_count(),
                    reads + usize::from(enabled == Some(true))
                );
            }
        }
        Ok(())
    }

    #[test]
    fn build_accepts_datasets_alias() {
        HfBuilder::default()
            .repo_id("org/repo")
            .repo_type("datasets")
            .build()
            .expect("builder should accept datasets alias");
    }

    #[test]
    fn build_accepts_space_repo_type() {
        HfBuilder::default()
            .repo_id("org/space")
            .repo_type("space")
            .build()
            .expect("builder should accept space repo type");
    }

    #[test]
    fn test_both_schemes_are_supported() {
        use opendal_core::OperatorRegistry;

        let registry = OperatorRegistry::get();
        super::super::register_hf_service(registry);

        // Test short scheme "hf"
        let op = registry
            .load("hf://user/repo")
            .expect("short scheme should be registered and work");
        assert_eq!(op.info().scheme(), "hf");

        // Test long scheme "huggingface"
        let op = registry
            .load("huggingface://user/repo")
            .expect("long scheme should be registered and work");
        assert_eq!(op.info().scheme(), "hf");
    }
}
