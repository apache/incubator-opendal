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
use std::fmt::Display;
use std::sync::Arc;

use log::Level;
use log::log;
use opendal_core::raw::*;
use opendal_core::*;

static LOGGING_TARGET: &str = "opendal::services";

/// `LoggingLayer` records every operation with
/// [log](https://docs.rs/log/).
///
/// # Logging
///
/// - OpenDAL emits structured logs.
/// - Every operation starts with a `started` log entry.
/// - Every operation ends with one of the following statuses:
///   - `succeeded`: The operation succeeded but might have more work to perform.
///   - `finished`: The whole operation finished.
///   - `failed`: The operation returned an error.
/// - The default log level for expected errors is `Warn`.
/// - The default log level for unexpected errors is `Error`.
///
/// # Examples
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_logging::LoggingLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(LoggingLayer::default());
/// # Ok(())
/// # }
/// ```
///
/// # Output
///
/// OpenDAL uses [`log`](https://docs.rs/log/latest/log/) internally.
///
/// To enable logging output, please set `RUST_LOG`:
///
/// ```shell
/// RUST_LOG=debug ./app
/// ```
///
/// To config logging output, please refer to [Configure Logging](https://rust-lang-nursery.github.io/rust-cookbook/development_tools/debugging/config_log.html):
///
/// ```shell
/// RUST_LOG="info,opendal::services=debug" ./app
/// ```
///
/// # Logging Interceptor
///
/// You can implement your own logging interceptor to customize the logging behavior.
///
/// ```no_run
/// # use opendal_core::raw;
/// # use opendal_core::services;
/// # use opendal_core::Error;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_logging::LoggingInterceptor;
/// # use opendal_layer_logging::LoggingLayer;
/// #
/// #[derive(Debug, Clone)]
/// struct MyLoggingInterceptor;
///
/// impl LoggingInterceptor for MyLoggingInterceptor {
///     fn log(
///         &self,
///         info: &raw::ServiceInfo,
///         operation: raw::Operation,
///         context: &[(&str, &str)],
///         message: &str,
///         err: Option<&Error>,
///     ) {
///         // log something
///     }
/// }
///
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(LoggingLayer::new(MyLoggingInterceptor));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct LoggingLayer<I = DefaultLoggingInterceptor> {
    logger: I,
}

impl Default for LoggingLayer {
    fn default() -> Self {
        Self {
            logger: DefaultLoggingInterceptor,
        }
    }
}

impl LoggingLayer {
    /// Create the layer with specific logging interceptor.
    pub fn new<I: LoggingInterceptor>(logger: I) -> LoggingLayer<I> {
        LoggingLayer { logger }
    }
}

impl<I: LoggingInterceptor> Layer for LoggingLayer<I> {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl<I: LoggingInterceptor> LoggingLayer<I> {
    fn layer(&self, inner: Servicer) -> LoggingService<I> {
        let info = inner.info();
        LoggingService {
            inner,
            info,
            logger: self.logger.clone(),
        }
    }
}

/// LoggingInterceptor customizes log emission.
pub trait LoggingInterceptor: Debug + Clone + Send + Sync + Unpin + 'static {
    /// Called for every log event.
    ///
    /// # Inputs
    ///
    /// - `info`: The service information used for this operation.
    /// - `operation`: The operation being logged.
    /// - `context`: Additional key-value context such as path, range, or counters.
    /// - `message`: The event message, such as `started`, `finished`, or `failed`.
    /// - `err`: The error associated with this event, if any.
    ///
    /// # Performance
    ///
    /// This method runs inline with the operation path. Avoid expensive I/O,
    /// network calls, or long-running work here.
    fn log(
        &self,
        info: &ServiceInfo,
        operation: Operation,
        context: &[(&str, &str)],
        message: &str,
        err: Option<&Error>,
    );
}

/// The DefaultLoggingInterceptor will log the message by the standard logging macro.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultLoggingInterceptor;

impl LoggingInterceptor for DefaultLoggingInterceptor {
    fn log(
        &self,
        info: &ServiceInfo,
        operation: Operation,
        context: &[(&str, &str)],
        message: &str,
        err: Option<&Error>,
    ) {
        if let Some(err) = err {
            // Expected errors are logged as warnings; unexpected errors need stronger
            // visibility and more diagnostic context.
            let lvl = if err.kind() == ErrorKind::Unexpected {
                Level::Error
            } else {
                Level::Warn
            };

            log!(
                target: LOGGING_TARGET,
                lvl,
                "service={} name={}{}: {operation} {message} {}",
                info.scheme(),
                info.name(),
                LoggingContext(context),
                // Use Debug for unexpected errors to preserve more context.
                // String avoids conditional format_args! temporaries in this log! argument.
                if err.kind() != ErrorKind::Unexpected {
                    format!("{err}")
                } else {
                    format!("{err:?}")
                }
            );
        }

        log!(
            target: LOGGING_TARGET,
            Level::Debug,
            "service={} name={}{}: {operation} {message}",
            info.scheme(),
            info.name(),
            LoggingContext(context),
        );
    }
}

struct LoggingContext<'a>(&'a [(&'a str, &'a str)]);

impl Display for LoggingContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, v) in self.0.iter() {
            write!(f, " {k}={v}")?;
        }
        Ok(())
    }
}

#[doc(hidden)]
pub struct LoggingService<I: LoggingInterceptor> {
    inner: Servicer,
    info: ServiceInfo,
    logger: I,
}

impl<I: LoggingInterceptor> Debug for LoggingService<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggingService")
            .field("inner", &self.inner)
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl<I: LoggingInterceptor> LoggingService<I> {
    fn log_start(&self, op: Operation, context: &[(&str, &str)]) {
        self.logger.log(&self.info, op, context, "started", None);
    }

    fn log_finish(&self, op: Operation, context: &[(&str, &str)], err: Option<&Error>) {
        let message = if err.is_some() { "failed" } else { "finished" };
        self.logger.log(&self.info, op, context, message, err);
    }
}

impl<I: LoggingInterceptor> Service for LoggingService<I> {
    type Reader = LoggingReader<oio::Reader, I>;
    type Writer = LoggingWriter<oio::Writer, I>;
    type Lister = LoggingLister<oio::Lister, I>;
    type Deleter = LoggingDeleter<oio::Deleter, I>;
    type Copier = LoggingCopier<oio::Copier, I>;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.info.clone()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner.compose(ctx, to, args)
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.log_start(Operation::CreateDir, &[("path", path)]);
        let result = self.inner.create_dir(ctx, path, args).await;
        self.log_finish(
            Operation::CreateDir,
            &[("path", path)],
            result.as_ref().err(),
        );
        result
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.log_start(Operation::Read, &[("path", path)]);
        self.inner
            .read(ctx, path, args)
            .map(|r| {
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[("path", path)],
                    "created reader",
                    None,
                );
                LoggingReader::new(self.info.clone(), self.logger.clone(), path, r)
            })
            .inspect_err(|err| {
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[("path", path)],
                    "failed",
                    Some(err),
                );
            })
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.log_start(Operation::Write, &[("path", path)]);
        self.inner
            .write(ctx, path, args)
            .map(|w| {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[("path", path)],
                    "created writer",
                    None,
                );
                LoggingWriter::new(self.info.clone(), self.logger.clone(), path, w)
            })
            .inspect_err(|err| {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[("path", path)],
                    "failed",
                    Some(err),
                );
            })
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.log_start(Operation::Stat, &[("path", path)]);
        let result = self.inner.stat(ctx, path, args).await;
        self.log_finish(Operation::Stat, &[("path", path)], result.as_ref().err());
        result
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.log_start(Operation::Delete, &[]);
        self.inner
            .delete(ctx)
            .map(|d| {
                self.logger
                    .log(&self.info, Operation::Delete, &[], "finished", None);
                LoggingDeleter::new(self.info.clone(), self.logger.clone(), d)
            })
            .inspect_err(|err| {
                self.logger
                    .log(&self.info, Operation::Delete, &[], "failed", Some(err));
            })
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.log_start(Operation::Copy, &[("from", from), ("to", to)]);
        self.inner
            .copy(ctx, from, to, args)
            .map(|c| {
                self.logger.log(
                    &self.info,
                    Operation::Copy,
                    &[("from", from), ("to", to)],
                    "created copier",
                    None,
                );
                LoggingCopier::new(self.info.clone(), self.logger.clone(), from, to, c)
            })
            .inspect_err(|err| {
                self.logger.log(
                    &self.info,
                    Operation::Copy,
                    &[("from", from), ("to", to)],
                    "failed",
                    Some(err),
                );
            })
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.log_start(Operation::Rename, &[("from", from), ("to", to)]);
        let result = self.inner.rename(ctx, from, to, args).await;
        self.log_finish(
            Operation::Rename,
            &[("from", from), ("to", to)],
            result.as_ref().err(),
        );
        result
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.log_start(Operation::Restore, &[("path", path)]);
        let result = self.inner.restore(ctx, path, args).await;
        self.log_finish(Operation::Restore, &[("path", path)], result.as_ref().err());
        result
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.log_start(Operation::List, &[("path", path)]);
        self.inner
            .list(ctx, path, args)
            .map(|v| {
                self.logger.log(
                    &self.info,
                    Operation::List,
                    &[("path", path)],
                    "created lister",
                    None,
                );
                LoggingLister::new(self.info.clone(), self.logger.clone(), path, v)
            })
            .inspect_err(|err| {
                self.logger.log(
                    &self.info,
                    Operation::List,
                    &[("path", path)],
                    "failed",
                    Some(err),
                );
            })
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.log_start(Operation::Presign, &[("path", path)]);
        let result = self.inner.presign(ctx, path, args).await;
        self.log_finish(Operation::Presign, &[("path", path)], result.as_ref().err());
        result
    }
}

#[doc(hidden)]
pub struct LoggingReader<R, I: LoggingInterceptor> {
    info: ServiceInfo,
    logger: I,
    path: String,
    range: Option<BytesRange>,

    read: u64,
    inner: R,
}

impl<R, I: LoggingInterceptor> LoggingReader<R, I> {
    fn new(info: ServiceInfo, logger: I, path: &str, reader: R) -> Self {
        Self::with_range(info, logger, path, None, reader)
    }

    fn with_range(
        info: ServiceInfo,
        logger: I,
        path: &str,
        range: Option<BytesRange>,
        reader: R,
    ) -> Self {
        Self {
            info,
            logger,
            path: path.to_string(),
            range,

            read: 0,
            inner: reader,
        }
    }

    fn range_label(&self) -> String {
        self.range
            .map(|range| range.to_string())
            .unwrap_or_default()
    }
}

impl<R: oio::ReadStream, I: LoggingInterceptor> oio::ReadStream for LoggingReader<R, I> {
    async fn read(&mut self) -> Result<Buffer> {
        match self.inner.read().await {
            Ok(bs) if bs.is_empty() => {
                let range = self.range_label();
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[
                        ("path", &self.path),
                        ("range", &range),
                        ("read", &self.read.to_string()),
                        ("size", &bs.len().to_string()),
                    ],
                    "finished",
                    None,
                );
                Ok(bs)
            }
            Ok(bs) => {
                self.read += bs.len() as u64;
                Ok(bs)
            }
            Err(err) => {
                let range = self.range_label();
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[
                        ("path", &self.path),
                        ("range", &range),
                        ("read", &self.read.to_string()),
                    ],
                    "failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }
}

impl<R: oio::Read, I: LoggingInterceptor> oio::Read for LoggingReader<R, I> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        match self.inner.open(range).await {
            Ok((rp, stream)) => Ok((
                rp,
                Box::new(LoggingReader::with_range(
                    self.info.clone(),
                    self.logger.clone(),
                    &self.path,
                    Some(range),
                    stream,
                )) as Box<dyn oio::ReadStreamDyn>,
            )),
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[("path", &self.path), ("range", &range.to_string())],
                    "failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        match self.inner.read(range).await {
            Ok((rp, buffer)) => {
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[
                        ("path", &self.path),
                        ("range", &range.to_string()),
                        ("size", &buffer.len().to_string()),
                    ],
                    "finished",
                    None,
                );
                Ok((rp, buffer))
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Read,
                    &[("path", &self.path), ("range", &range.to_string())],
                    "failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }
}

#[doc(hidden)]
pub struct LoggingWriter<W, I> {
    info: ServiceInfo,
    logger: I,
    path: String,

    written: u64,
    inner: W,
}

impl<W, I> LoggingWriter<W, I> {
    fn new(info: ServiceInfo, logger: I, path: &str, writer: W) -> Self {
        Self {
            info,
            logger,
            path: path.to_string(),

            written: 0,
            inner: writer,
        }
    }
}

impl<W: oio::Write, I: LoggingInterceptor> oio::Write for LoggingWriter<W, I> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len();

        match self.inner.write(bs).await {
            Ok(_) => {
                self.written += size as u64;
                Ok(())
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[
                        ("path", &self.path),
                        ("written", &self.written.to_string()),
                        ("size", &size.to_string()),
                    ],
                    "failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let size = range
            .size()
            .expect("writer copy range must be absolute and bounded");

        match self.inner.copy_from(path, args, range).await {
            Ok(()) => {
                self.written += size;
                Ok(())
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[
                        ("path", &self.path),
                        ("source", path),
                        ("written", &self.written.to_string()),
                        ("size", &size.to_string()),
                    ],
                    "failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }

    async fn abort(&mut self) -> Result<()> {
        match self.inner.abort().await {
            Ok(_) => {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[("path", &self.path), ("written", &self.written.to_string())],
                    "abort succeeded",
                    None,
                );
                Ok(())
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[("path", &self.path), ("written", &self.written.to_string())],
                    "abort failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }

    async fn close(&mut self) -> Result<Metadata> {
        match self.inner.close().await {
            Ok(meta) => {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[("path", &self.path), ("written", &self.written.to_string())],
                    "close succeeded",
                    None,
                );
                Ok(meta)
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Write,
                    &[("path", &self.path), ("written", &self.written.to_string())],
                    "close failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }
}

#[doc(hidden)]
pub struct LoggingLister<P, I: LoggingInterceptor> {
    info: ServiceInfo,
    logger: I,
    path: String,

    listed: usize,
    inner: P,
}

impl<P, I: LoggingInterceptor> LoggingLister<P, I> {
    fn new(info: ServiceInfo, logger: I, path: &str, inner: P) -> Self {
        Self {
            info,
            logger,
            path: path.to_string(),

            listed: 0,
            inner,
        }
    }
}

impl<P: oio::List, I: LoggingInterceptor> oio::List for LoggingLister<P, I> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let res = self.inner.next().await;

        match &res {
            Ok(Some(_)) => {
                self.listed += 1;
            }
            Ok(None) => {
                self.logger.log(
                    &self.info,
                    Operation::List,
                    &[("path", &self.path), ("listed", &self.listed.to_string())],
                    "finished",
                    None,
                );
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::List,
                    &[("path", &self.path), ("listed", &self.listed.to_string())],
                    "failed",
                    Some(err),
                );
            }
        };

        res
    }
}

#[doc(hidden)]
pub struct LoggingDeleter<D, I: LoggingInterceptor> {
    info: ServiceInfo,
    logger: I,

    deleted: usize,
    inner: D,
}

impl<D, I: LoggingInterceptor> LoggingDeleter<D, I> {
    fn new(info: ServiceInfo, logger: I, inner: D) -> Self {
        Self {
            info,
            logger,

            deleted: 0,
            inner,
        }
    }
}

impl<D: oio::Delete, I: LoggingInterceptor> oio::Delete for LoggingDeleter<D, I> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        let version = args
            .version()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<latest>".to_string());

        let res = self.inner.delete(path, args).await;

        match &res {
            Ok(_) => {
                self.deleted += 1;
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Delete,
                    &[
                        ("path", path),
                        ("version", &version),
                        ("deleted", &self.deleted.to_string()),
                    ],
                    "failed",
                    Some(err),
                );
            }
        };

        res
    }

    async fn close(&mut self) -> Result<()> {
        let res = self.inner.close().await;

        match &res {
            Ok(_) => {
                self.logger.log(
                    &self.info,
                    Operation::Delete,
                    &[("deleted", &self.deleted.to_string())],
                    "succeeded",
                    None,
                );
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Delete,
                    &[("deleted", &self.deleted.to_string())],
                    "failed",
                    Some(err),
                );
            }
        };

        res
    }
}

#[doc(hidden)]
pub struct LoggingCopier<C, I: LoggingInterceptor> {
    info: ServiceInfo,
    logger: I,
    from: String,
    to: String,

    copied: u64,
    inner: C,
}

impl<C, I: LoggingInterceptor> LoggingCopier<C, I> {
    fn new(info: ServiceInfo, logger: I, from: &str, to: &str, inner: C) -> Self {
        Self {
            info,
            logger,
            from: from.to_string(),
            to: to.to_string(),

            copied: 0,
            inner,
        }
    }
}

impl<C: oio::Copy, I: LoggingInterceptor> oio::Copy for LoggingCopier<C, I> {
    async fn next(&mut self) -> Result<Option<usize>> {
        match self.inner.next().await {
            Ok(Some(n)) => {
                self.copied += n as u64;
                Ok(Some(n))
            }
            Ok(None) => {
                self.logger.log(
                    &self.info,
                    Operation::Copy,
                    &[
                        ("from", &self.from),
                        ("to", &self.to),
                        ("copied", &self.copied.to_string()),
                    ],
                    "finished",
                    None,
                );
                Ok(None)
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Copy,
                    &[
                        ("from", &self.from),
                        ("to", &self.to),
                        ("copied", &self.copied.to_string()),
                    ],
                    "failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        match self.inner.abort().await {
            Ok(_) => {
                self.logger.log(
                    &self.info,
                    Operation::Copy,
                    &[
                        ("from", &self.from),
                        ("to", &self.to),
                        ("copied", &self.copied.to_string()),
                    ],
                    "abort succeeded",
                    None,
                );
                Ok(())
            }
            Err(err) => {
                self.logger.log(
                    &self.info,
                    Operation::Copy,
                    &[
                        ("from", &self.from),
                        ("to", &self.to),
                        ("copied", &self.copied.to_string()),
                    ],
                    "abort failed",
                    Some(&err),
                );
                Err(err)
            }
        }
    }
}
