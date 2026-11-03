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

use std::future::Future;
use std::future::IntoFuture;

use crate::raw::oio::Compose;
use crate::raw::*;
use crate::*;

/// Creates one destination object from ordered complete source objects.
///
/// [`Composer::compose`] accepts sources incrementally and may start backend
/// work before returning. [`Composer::close`] waits for pending work and commits
/// the destination.
pub struct Composer {
    composer: oio::Composer,
    to: String,
    scheme: &'static str,
    capability: Capability,
}

impl Composer {
    pub(crate) fn create(
        ctx: OperationContext,
        srv: Servicer,
        to: String,
        args: OpCompose,
    ) -> Result<Self> {
        let scheme = srv.info().scheme();
        let capability = srv.capability();
        let composer = srv.compose(&ctx, &to, args)?;

        Ok(Self {
            composer,
            to,
            scheme,
            capability,
        })
    }

    /// Accept one complete source object in sequence.
    ///
    /// A successful return means the source was accepted in order. Backend work
    /// may still be pending. This method applies backpressure when the
    /// configured concurrency window is full.
    pub async fn compose(&mut self, input: impl IntoComposeInput) -> Result<()> {
        self.compose_with(input).await
    }

    /// Accept one complete source object with additional options.
    ///
    /// Visit [`options::ComposeSourceOptions`] for all available options.
    pub fn compose_with(
        &mut self,
        input: impl IntoComposeInput,
    ) -> FutureComposeSource<'_, impl Future<Output = Result<()>> + '_> {
        let (path, options) = input.into_compose_input();
        FutureComposeSource {
            composer: self,
            path,
            options,
            f: Self::compose_inner,
        }
    }

    async fn compose_inner(
        composer: &mut Composer,
        path: String,
        options: options::ComposeSourceOptions,
    ) -> Result<()> {
        composer.compose_options(&path, options).await
    }

    /// Accept one complete source object with explicit options.
    ///
    /// Visit [`options::ComposeSourceOptions`] for all available options.
    pub async fn compose_options(
        &mut self,
        path: &str,
        options: options::ComposeSourceOptions,
    ) -> Result<()> {
        let path = normalize_path(path);
        if !validate_path(&path, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "source path is a directory")
                    .with_operation(Operation::Compose.into_static())
                    .with_context("service", self.scheme)
                    .with_context("path", path),
            );
        }
        if path == self.to {
            return Err(Error::new(
                ErrorKind::IsSameFile,
                "source and destination paths are same",
            )
            .with_operation(Operation::Compose.into_static())
            .with_context("service", self.scheme)
            .with_context("path", path));
        }

        let args = OpRead::from_compose_source_options(&self.capability, options)
            .map_err(|err| err.with_context("service", self.scheme))?;
        self.composer.compose(&path, args).await.map_err(|err| {
            err.with_operation(Operation::Compose.into_static())
                .with_context("service", self.scheme)
                .with_context("from", path)
                .with_context("to", &self.to)
        })?;
        Ok(())
    }

    /// Commit all accepted sources and return destination metadata.
    ///
    /// Closing a composer with no accepted sources returns
    /// [`ErrorKind::ConfigInvalid`].
    pub async fn close(&mut self) -> Result<Metadata> {
        self.composer.close().await.map_err(|err| {
            err.with_operation(Operation::Compose.into_static())
                .with_context("service", self.scheme)
                .with_context("to", &self.to)
        })
    }
}

/// [`Composer::compose_with`] returns this future.
///
/// Use its methods to configure one source object before awaiting it.
pub struct FutureComposeSource<'a, F: Future<Output = Result<()>>> {
    composer: &'a mut Composer,
    path: String,
    options: options::ComposeSourceOptions,
    f: fn(&'a mut Composer, String, options::ComposeSourceOptions) -> F,
}

impl<F: Future<Output = Result<()>>> FutureComposeSource<'_, F> {
    /// Compose this version of the source object.
    pub fn version(mut self, value: &str) -> Self {
        self.options.version = Some(value.to_string());
        self
    }

    /// Require this source ETag.
    pub fn if_match(mut self, value: &str) -> Self {
        self.options.if_match = Some(value.to_string());
        self
    }

    /// Require the source to retain the identity in `metadata`.
    pub fn if_not_changed(mut self, metadata: &Metadata) -> Self {
        self.options.if_not_changed = Some(metadata.clone());
        self
    }
}

impl<F: Future<Output = Result<()>>> IntoFuture for FutureComposeSource<'_, F> {
    type Output = Result<()>;
    type IntoFuture = F;

    fn into_future(self) -> Self::IntoFuture {
        let Self {
            composer,
            path,
            options,
            f,
        } = self;
        (f)(composer, path, options)
    }
}
