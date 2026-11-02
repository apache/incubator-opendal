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

use std::collections::HashMap;
use std::sync::Arc;

use super::core::ErrorContext;
use super::core::GcsComposeSource;
use super::core::GcsCore;
use super::core::parse_error;
use super::deleter::GcsDeleter;
use opendal_core::raw::oio::Delete;
use opendal_core::raw::*;
use opendal_core::*;

const GCS_COMPOSE_MAX_SOURCES: usize = 32;
const COMPOSE_TOKEN_KEY: &str = "opendal-compose-token";

pub struct GcsComposer {
    core: Arc<GcsCore>,
    ctx: OperationContext,
    to: String,
    args: OpCompose,
    buffered: Vec<GcsComposeSource>,
    tasks: ConcurrentTasks<GcsComposeTask, GcsComposeSource>,
    input_count: usize,
    intermediates: Vec<GcsComposeSource>,
    metadata: Option<Metadata>,
    errored: bool,
}

impl GcsComposer {
    pub fn new(core: Arc<GcsCore>, ctx: OperationContext, to: &str, args: OpCompose) -> Self {
        let tasks = new_tasks(ctx.executor().clone(), args.concurrent());
        Self {
            core,
            ctx,
            to: to.to_string(),
            args,
            buffered: Vec::with_capacity(GCS_COMPOSE_MAX_SOURCES + 1),
            tasks,
            input_count: 0,
            intermediates: Vec::new(),
            metadata: None,
            errored: false,
        }
    }

    fn new_intermediate_task(&self, sources: Vec<GcsComposeSource>) -> Result<GcsComposeTask> {
        let token = uuid::Uuid::new_v4().to_string();
        let to = format!("__opendal/compose/{token}");
        let mut metadata = HashMap::new();
        metadata.insert(COMPOSE_TOKEN_KEY.to_string(), token.clone());
        let args = OpCompose::from_options(
            &self.core.capability,
            options::ComposeOptions {
                if_not_exists: true,
                content_type: Some("application/octet-stream".to_string()),
                user_metadata: Some(metadata),
                ..Default::default()
            },
        )?;

        Ok(GcsComposeTask {
            core: self.core.clone(),
            ctx: self.ctx.clone(),
            sources,
            to,
            args,
            token: Some(token),
        })
    }

    async fn collect_tasks(
        tasks: &mut ConcurrentTasks<GcsComposeTask, GcsComposeSource>,
    ) -> Result<Vec<GcsComposeSource>> {
        let mut outputs = Vec::new();
        while let Some(output) = tasks.next().await {
            match output {
                Ok(output) => outputs.push(output),
                Err(err) if err.is_temporary() => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(outputs)
    }

    async fn execute_task(
        tasks: &mut ConcurrentTasks<GcsComposeTask, GcsComposeSource>,
        task: GcsComposeTask,
    ) -> Result<()> {
        loop {
            match tasks.execute(task.clone()).await {
                Ok(()) => return Ok(()),
                Err(err) if err.is_temporary() => continue,
                Err(err) => return Err(err),
            }
        }
    }

    async fn reduce_level(
        &mut self,
        sources: Vec<GcsComposeSource>,
    ) -> Result<Vec<GcsComposeSource>> {
        let mut tasks = new_tasks(self.ctx.executor().clone(), self.args.concurrent());
        let mut carried = None;

        for chunk in sources.chunks(GCS_COMPOSE_MAX_SOURCES) {
            if chunk.len() == 1 {
                carried = Some(chunk[0].clone());
                continue;
            }
            Self::execute_task(&mut tasks, self.new_intermediate_task(chunk.to_vec())?).await?;
        }

        let mut outputs = Self::collect_tasks(&mut tasks).await?;
        self.intermediates.extend(outputs.iter().cloned());
        if let Some(source) = carried {
            outputs.push(source);
        }
        Ok(outputs)
    }

    async fn compose_final(&self, sources: &[GcsComposeSource]) -> Result<Metadata> {
        let resp = self
            .core
            .gcs_compose_object(&self.ctx, sources, &self.to, &self.args)
            .await?;
        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ComposeObject"))
                    .with_caller_condition(self.args.is_conditional()),
                resp,
            ));
        }
        GcsCore::build_metadata_from_object_response(&self.to, resp.into_body())
    }

    async fn cleanup_intermediates(&self) -> Result<()> {
        let mut deleter = oio::BatchDeleter::new(
            GcsDeleter::new(self.core.clone(), self.ctx.clone()),
            self.core.capability.delete_max_size,
        );
        for source in &self.intermediates {
            let Some(version) = source.version.as_deref() else {
                continue;
            };
            let args = OpDelete::from_options(
                &self.core.capability,
                options::DeleteOptions {
                    if_version_match: Some(version.to_string()),
                    ..Default::default()
                },
            )?;
            deleter.delete(&source.path, args).await?;
        }
        deleter.close().await
    }

    async fn close_inner(&mut self) -> Result<Metadata> {
        if self.input_count == 0 {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "compose requires at least one source object",
            ));
        }

        let mut sources = Self::collect_tasks(&mut self.tasks).await?;
        self.intermediates.extend(sources.iter().cloned());
        sources.append(&mut self.buffered);

        while sources.len() > GCS_COMPOSE_MAX_SOURCES {
            sources = self.reduce_level(sources).await?;
        }

        let metadata = self.compose_final(&sources).await?;
        if let Err(err) = self.cleanup_intermediates().await {
            log::warn!("failed to clean GCS compose intermediate objects: {err}");
        }
        Ok(metadata)
    }
}

impl oio::Compose for GcsComposer {
    async fn compose(&mut self, path: &str, args: OpRead) -> Result<()> {
        if self.errored {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS composer has already failed",
            ));
        }
        if self.metadata.is_some() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS composer is already closed",
            ));
        }
        if path == self.to {
            return Err(Error::new(
                ErrorKind::IsSameFile,
                "source and destination paths are same",
            ));
        }
        if args.if_match().is_some()
            || args.if_none_match().is_some()
            || args.if_version_not_match().is_some()
        {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "GCS compose supports source generation conditions only",
            ));
        }

        self.buffered.push(GcsComposeSource {
            path: path.to_string(),
            version: args.version().map(str::to_string),
            if_version_match: args.if_version_match().map(str::to_string),
        });
        self.input_count += 1;

        if self.buffered.len() > GCS_COMPOSE_MAX_SOURCES {
            let remaining = self.buffered.split_off(GCS_COMPOSE_MAX_SOURCES);
            let sources = std::mem::replace(&mut self.buffered, remaining);
            let task = match self.new_intermediate_task(sources) {
                Ok(task) => task,
                Err(err) => {
                    self.errored = true;
                    return Err(err);
                }
            };
            if let Err(err) = Self::execute_task(&mut self.tasks, task).await {
                self.errored = true;
                return Err(err);
            }
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        if self.errored {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS composer has already failed and can't be closed",
            ));
        }
        if let Some(metadata) = self.metadata.clone() {
            return Ok(metadata);
        }

        match self.close_inner().await {
            Ok(metadata) => {
                self.metadata = Some(metadata.clone());
                Ok(metadata)
            }
            Err(err) => {
                self.errored = true;
                Err(err)
            }
        }
    }
}

#[derive(Clone)]
struct GcsComposeTask {
    core: Arc<GcsCore>,
    ctx: OperationContext,
    sources: Vec<GcsComposeSource>,
    to: String,
    args: OpCompose,
    token: Option<String>,
}

impl GcsComposeTask {
    async fn run(&self) -> Result<GcsComposeSource> {
        let resp = self
            .core
            .gcs_compose_object(&self.ctx, &self.sources, &self.to, &self.args)
            .await?;
        if !resp.status().is_success() {
            let err = parse_error(
                ErrorContext::new(ServiceOperation("ComposeObject"))
                    .with_caller_condition(self.args.is_conditional())
                    .with_internal_condition(self.token.is_some()),
                resp,
            );
            let recoverable_condition = if self.token.is_some() {
                ErrorKind::Conflict
            } else {
                ErrorKind::ConditionNotMatch
            };
            if err.kind() == recoverable_condition
                && let Some(source) = self.recover_completed_intermediate().await?
            {
                return Ok(source);
            }
            return Err(err);
        }

        let metadata = GcsCore::build_metadata_from_object_response(&self.to, resp.into_body())?;
        self.output_from_metadata(metadata)
    }

    async fn recover_completed_intermediate(&self) -> Result<Option<GcsComposeSource>> {
        let Some(token) = self.token.as_deref() else {
            return Ok(None);
        };
        let resp = self
            .core
            .gcs_get_object_metadata(&self.ctx, &self.to, &OpStat::new())
            .await?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let metadata = GcsCore::build_metadata_from_object_response(&self.to, resp.into_body())?;
        if metadata
            .user_metadata()
            .and_then(|values| values.get(COMPOSE_TOKEN_KEY))
            .is_none_or(|value| value != token)
        {
            return Ok(None);
        }
        self.output_from_metadata(metadata).map(Some)
    }

    fn output_from_metadata(&self, metadata: Metadata) -> Result<GcsComposeSource> {
        let version = metadata.version().ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "GCS compose response is missing object generation",
            )
        })?;
        Ok(GcsComposeSource {
            path: self.to.clone(),
            version: Some(version.to_string()),
            if_version_match: None,
        })
    }
}

fn new_tasks(
    executor: Executor,
    concurrent: usize,
) -> ConcurrentTasks<GcsComposeTask, GcsComposeSource> {
    ConcurrentTasks::new(executor, concurrent.max(1), concurrent.max(1), |task| {
        Box::pin(async move {
            let result = task.run().await;
            (task, result)
        })
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use asyncband::latch::Latch;
    use asyncband::mutex::Mutex;
    use futures::stream;
    use http::Method;
    use http::Response;
    use reqsign_core::Context;
    use reqsign_core::ProvideCredentialChain;
    use reqsign_core::Signer;
    use reqsign_google::RequestSigner;
    use reqsign_google::TokenCredentialProvider;

    use super::*;
    use opendal_core::raw::oio::Compose;

    struct ComposeTransport {
        started: Latch,
        active: AtomicUsize,
        max_active: AtomicUsize,
        generation: AtomicUsize,
        requests: Mutex<Vec<serde_json::Value>>,
    }

    impl ComposeTransport {
        fn new() -> Self {
            Self {
                started: Latch::new(2),
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                generation: AtomicUsize::new(1),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn response(status: http::StatusCode, body: Buffer) -> Result<Response<HttpBody>> {
            let size = body.len() as u64;
            Response::builder()
                .status(status)
                .body(HttpBody::new(stream::iter([Ok(body)]), Some(size)))
                .map_err(new_request_build_error)
        }
    }

    #[derive(Clone)]
    struct SharedComposeTransport(Arc<ComposeTransport>);

    struct TokioTestExecutor;

    impl Execute for TokioTestExecutor {
        fn execute(&self, future: BoxedStaticFuture<()>) {
            tokio::spawn(future);
        }
    }

    impl HttpTransport for SharedComposeTransport {
        async fn fetch(&self, req: http::Request<Buffer>) -> Result<Response<HttpBody>> {
            if req.method() == Method::DELETE {
                return ComposeTransport::response(http::StatusCode::NO_CONTENT, Buffer::new());
            }

            let value: serde_json::Value = serde_json::from_slice(&req.body().to_bytes())
                .expect("compose request body must be valid JSON");
            self.0.requests.lock().await.push(value.clone());

            if req.uri().path().contains("__opendal%2Fcompose%2F") {
                let active = self.0.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.0.max_active.fetch_max(active, Ordering::SeqCst);
                self.0.started.count_down();
                self.0.started.wait().await;
                self.0.active.fetch_sub(1, Ordering::SeqCst);
            }

            let generation = self.0.generation.fetch_add(1, Ordering::SeqCst);
            let metadata = value
                .get("destination")
                .and_then(|destination| destination.get("metadata"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let body = Buffer::from(
                serde_json::to_vec(&serde_json::json!({
                    "size": "1",
                    "generation": generation.to_string(),
                    "metadata": metadata,
                }))
                .expect("response JSON must serialize"),
            );
            ComposeTransport::response(http::StatusCode::OK, body)
        }
    }

    struct RecoveryTransport {
        intermediate_posts: AtomicUsize,
        metadata_reads: AtomicUsize,
        token: Mutex<Option<String>>,
    }

    #[derive(Clone)]
    struct SharedRecoveryTransport(Arc<RecoveryTransport>);

    impl HttpTransport for SharedRecoveryTransport {
        async fn fetch(&self, req: http::Request<Buffer>) -> Result<Response<HttpBody>> {
            if req.method() == Method::DELETE {
                return ComposeTransport::response(http::StatusCode::NO_CONTENT, Buffer::new());
            }

            if req.method() == Method::GET {
                self.0.metadata_reads.fetch_add(1, Ordering::SeqCst);
                let token = self.0.token.lock().await.clone().unwrap();
                let body = Buffer::from(
                    serde_json::to_vec(&serde_json::json!({
                        "size": "32",
                        "generation": "7",
                        "metadata": {(COMPOSE_TOKEN_KEY): token},
                    }))
                    .unwrap(),
                );
                return ComposeTransport::response(http::StatusCode::OK, body);
            }

            if req.uri().path().contains("__opendal%2Fcompose%2F") {
                let value: serde_json::Value = serde_json::from_slice(&req.body().to_bytes())
                    .expect("compose request body must be valid JSON");
                let token = value["destination"]["metadata"][COMPOSE_TOKEN_KEY]
                    .as_str()
                    .unwrap()
                    .to_string();
                *self.0.token.lock().await = Some(token);

                if self.0.intermediate_posts.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "compose response was lost after the object was created",
                    )
                    .set_temporary());
                }
                return ComposeTransport::response(
                    http::StatusCode::PRECONDITION_FAILED,
                    Buffer::new(),
                );
            }

            let body = Buffer::from(
                serde_json::to_vec(&serde_json::json!({
                    "size": "33",
                    "generation": "8",
                }))
                .unwrap(),
            );
            ComposeTransport::response(http::StatusCode::OK, body)
        }
    }

    fn test_core() -> Arc<GcsCore> {
        let sign_ctx = Context::new();
        Arc::new(GcsCore {
            info: ServiceInfo::new("gcs", "/", "test-bucket"),
            capability: Capability {
                delete_max_size: Some(1),
                ..Default::default()
            },
            endpoint: "https://storage.googleapis.com".to_string(),
            bucket: "test-bucket".to_string(),
            root: "/".to_string(),
            signer: Signer::new(
                sign_ctx.clone(),
                ProvideCredentialChain::new().push(TokenCredentialProvider::new("test-token")),
                RequestSigner::new("storage"),
            ),
            sign_ctx,
            predefined_acl: None,
            default_storage_class: None,
            skip_signature: true,
        })
    }

    #[tokio::test]
    async fn test_compose_builds_concurrent_ordered_tree() {
        let transport = Arc::new(ComposeTransport::new());
        let ctx = OperationContext::default().with_http_transport(HttpTransporter::new(
            SharedComposeTransport(transport.clone()),
        ));
        let ctx = ctx.with_executor(Executor::with(TokioTestExecutor));
        let mut composer = GcsComposer::new(
            test_core(),
            ctx,
            "target",
            OpCompose::from_options(
                &Capability::default(),
                options::ComposeOptions {
                    concurrent: 2,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

        for index in 0..65 {
            composer
                .compose(&format!("source-{index:02}"), OpRead::new())
                .await
                .expect("source must be accepted");
        }
        tokio::time::timeout(Duration::from_secs(5), composer.close())
            .await
            .expect("compose tasks must make concurrent progress")
            .expect("composition must succeed");

        assert_eq!(transport.max_active.load(Ordering::SeqCst), 2);
        let requests = transport.requests.lock().await;
        assert_eq!(requests.len(), 3);

        let source_lists: Vec<&Vec<serde_json::Value>> = requests
            .iter()
            .map(|request| request["sourceObjects"].as_array().unwrap())
            .collect();
        assert_eq!(
            source_lists
                .iter()
                .filter(|sources| sources.len() == 32)
                .count(),
            2
        );
        let final_sources = source_lists
            .iter()
            .find(|sources| sources.len() == 3)
            .expect("final composition must contain the reduced source sequence");
        assert!(
            final_sources[0]["name"]
                .as_str()
                .unwrap()
                .starts_with("__opendal/compose/")
        );
        assert!(
            final_sources[1]["name"]
                .as_str()
                .unwrap()
                .starts_with("__opendal/compose/")
        );
        assert_eq!(final_sources[2]["name"], "source-64");
    }

    #[tokio::test]
    async fn test_compose_recovers_ambiguous_intermediate_completion() {
        let transport = Arc::new(RecoveryTransport {
            intermediate_posts: AtomicUsize::new(0),
            metadata_reads: AtomicUsize::new(0),
            token: Mutex::new(None),
        });
        let ctx = OperationContext::default().with_http_transport(HttpTransporter::new(
            SharedRecoveryTransport(transport.clone()),
        ));
        let mut composer = GcsComposer::new(test_core(), ctx, "target", OpCompose::new());

        for index in 0..33 {
            composer
                .compose(&format!("source-{index:02}"), OpRead::new())
                .await
                .expect("source must be accepted");
        }
        composer.close().await.expect("composition must recover");

        assert_eq!(transport.intermediate_posts.load(Ordering::SeqCst), 2);
        assert_eq!(transport.metadata_reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_compose_hides_request_limit_across_tree_levels() {
        let transport = Arc::new(ComposeTransport::new());
        let ctx = OperationContext::default().with_http_transport(HttpTransporter::new(
            SharedComposeTransport(transport.clone()),
        ));
        let ctx = ctx.with_executor(Executor::with(TokioTestExecutor));
        let mut composer = GcsComposer::new(
            test_core(),
            ctx,
            "target",
            OpCompose::from_options(
                &Capability::default(),
                options::ComposeOptions {
                    concurrent: 4,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

        for index in 0..1025 {
            composer
                .compose(&format!("source-{index:04}"), OpRead::new())
                .await
                .expect("source must be accepted");
        }
        composer.close().await.expect("composition must succeed");

        let requests = transport.requests.lock().await;
        assert_eq!(requests.len(), 34);
        let source_lists: Vec<&Vec<serde_json::Value>> = requests
            .iter()
            .map(|request| request["sourceObjects"].as_array().unwrap())
            .collect();
        assert!(
            source_lists
                .iter()
                .all(|sources| !sources.is_empty() && sources.len() <= GCS_COMPOSE_MAX_SOURCES)
        );
        assert_eq!(
            source_lists
                .iter()
                .filter(|sources| sources.len() == 32)
                .count(),
            33
        );
        let final_sources = source_lists
            .iter()
            .find(|sources| sources.len() == 2)
            .expect("final composition must contain the reduced source sequence");
        assert!(
            final_sources[0]["name"]
                .as_str()
                .unwrap()
                .starts_with("__opendal/compose/")
        );
        assert_eq!(final_sources[1]["name"], "source-1024");
    }
}
