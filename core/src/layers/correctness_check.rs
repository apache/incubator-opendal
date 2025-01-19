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

use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use crate::raw::*;
use crate::*;

/// Add a correctness capability check layer for every operation
///
/// Before performing any operations, we will first verify the operation and its critical arguments
/// against the capability of the underlying service. If the operation or arguments is not supported,
/// an error will be returned directly.
///
/// # Notes
///
/// OpenDAL applies this checker to every accessor by default, so users don't need to invoke it manually.
/// this checker ensures the operation and its critical arguments, which might affect the correctness of
/// the call, are supported by the underlying service.
///
/// for example, when calling `write_with_append`, but `append` is not supported by the underlying
/// service, an `Unsupported` error is returned. without this check, undesired data may be written.
#[derive(Default)]
pub struct CorrectnessCheckLayer;

impl<A: Access> Layer<A> for CorrectnessCheckLayer {
    type LayeredAccess = CorrectnessAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        CorrectnessAccessor {
            info: inner.info(),
            inner,
        }
    }
}

pub(crate) fn new_unsupported_error(info: &AccessorInfo, op: Operation, args: &str) -> Error {
    let scheme = info.scheme();
    let op = op.into_static();

    Error::new(
        ErrorKind::Unsupported,
        format!("The service {scheme} does not support the operation {op} with the arguments {args}. Please verify if the relevant flags have been enabled, or submit an issue if you believe this is incorrect."),
    )
    .with_operation(op)
}

pub struct CorrectnessAccessor<A: Access> {
    info: Arc<AccessorInfo>,
    inner: A,
}

impl<A: Access> Debug for CorrectnessAccessor<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CorrectnessCheckAccessor")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<A: Access> LayeredAccess for CorrectnessAccessor<A> {
    type Inner = A;
    type Reader = A::Reader;
    type Writer = A::Writer;
    type Lister = A::Lister;
    type Deleter = CheckWrapper<A::Deleter>;
    type BlockingReader = A::BlockingReader;
    type BlockingWriter = A::BlockingWriter;
    type BlockingLister = A::BlockingLister;
    type BlockingDeleter = CheckWrapper<A::BlockingDeleter>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn info(&self) -> Arc<AccessorInfo> {
        self.info.clone()
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let capability = self.info.full_capability();
        if !capability.read_with_version && args.version().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Read,
                "version",
            ));
        }
        if !capability.read_with_if_match && args.if_match().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Read,
                "if_match",
            ));
        }
        if !capability.read_with_if_none_match && args.if_none_match().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Read,
                "if_none_match",
            ));
        }
        if !capability.read_with_if_modified_since && args.if_modified_since().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Read,
                "if_modified_since",
            ));
        }
        if !capability.read_with_if_unmodified_since && args.if_unmodified_since().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Read,
                "if_unmodified_since",
            ));
        }

        self.inner.read(path, args).await
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let capability = self.info.full_capability();
        if args.append() && !capability.write_can_append {
            return Err(new_unsupported_error(
                &self.info,
                Operation::Write,
                "append",
            ));
        }
        if args.if_not_exists() && !capability.write_with_if_not_exists {
            return Err(new_unsupported_error(
                &self.info,
                Operation::Write,
                "if_not_exists",
            ));
        }
        if let Some(if_none_match) = args.if_none_match() {
            if !capability.write_with_if_none_match {
                let mut err =
                    new_unsupported_error(self.info.as_ref(), Operation::Write, "if_none_match");
                if if_none_match == "*" && capability.write_with_if_not_exists {
                    err = err.with_context("hint", "use if_not_exists instead");
                }

                return Err(err);
            }
        }

        self.inner.write(path, args).await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let capability = self.info.full_capability();
        if !capability.stat_with_version && args.version().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Stat,
                "version",
            ));
        }
        if !capability.stat_with_if_match && args.if_match().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Stat,
                "if_match",
            ));
        }
        if !capability.stat_with_if_none_match && args.if_none_match().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Stat,
                "if_none_match",
            ));
        }
        if !capability.stat_with_if_modified_since && args.if_modified_since().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Stat,
                "if_modified_since",
            ));
        }
        if !capability.stat_with_if_unmodified_since && args.if_unmodified_since().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::Stat,
                "if_unmodified_since",
            ));
        }

        self.inner.stat(path, args).await
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner.delete().await.map(|(rp, deleter)| {
            let deleter = CheckWrapper::new(deleter, self.info.clone());
            (rp, deleter)
        })
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.inner.list(path, args).await
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let capability = self.info.full_capability();
        if !capability.read_with_version && args.version().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::BlockingRead,
                "version",
            ));
        }

        self.inner.blocking_read(path, args)
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        let capability = self.info.full_capability();
        if args.append() && !capability.write_can_append {
            return Err(new_unsupported_error(
                &self.info,
                Operation::BlockingWrite,
                "append",
            ));
        }
        if args.if_not_exists() && !capability.write_with_if_not_exists {
            return Err(new_unsupported_error(
                &self.info,
                Operation::BlockingWrite,
                "if_not_exists",
            ));
        }
        if args.if_none_match().is_some() && !capability.write_with_if_none_match {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::BlockingWrite,
                "if_none_match",
            ));
        }

        self.inner.blocking_write(path, args)
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let capability = self.info.full_capability();
        if !capability.stat_with_version && args.version().is_some() {
            return Err(new_unsupported_error(
                self.info.as_ref(),
                Operation::BlockingStat,
                "version",
            ));
        }

        self.inner.blocking_stat(path, args)
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner.blocking_delete().map(|(rp, deleter)| {
            let deleter = CheckWrapper::new(deleter, self.info.clone());
            (rp, deleter)
        })
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        self.inner.blocking_list(path, args)
    }
}

pub struct CheckWrapper<T> {
    info: Arc<AccessorInfo>,
    inner: T,
}

impl<T> CheckWrapper<T> {
    fn new(inner: T, info: Arc<AccessorInfo>) -> Self {
        Self { inner, info }
    }

    fn check_delete(&self, args: &OpDelete) -> Result<()> {
        if args.version().is_some() && !self.info.full_capability().delete_with_version {
            return Err(new_unsupported_error(
                &self.info,
                Operation::DeleterDelete,
                "version",
            ));
        }

        Ok(())
    }
}

impl<T: oio::Delete> oio::Delete for CheckWrapper<T> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.check_delete(&args)?;
        self.inner.delete(path, args)
    }

    fn flush(&mut self) -> impl Future<Output = Result<usize>> + MaybeSend {
        self.inner.flush()
    }
}

impl<T: oio::BlockingDelete> oio::BlockingDelete for CheckWrapper<T> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.check_delete(&args)?;
        self.inner.delete(path, args)
    }

    fn flush(&mut self) -> Result<usize> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::oio;
    use crate::{Capability, EntryMode, Metadata, Operator};

    #[derive(Debug)]
    struct MockService {
        capability: Capability,
    }

    impl Access for MockService {
        type Reader = oio::Reader;
        type Writer = oio::Writer;
        type Lister = oio::Lister;
        type Deleter = oio::Deleter;
        type BlockingReader = oio::BlockingReader;
        type BlockingWriter = oio::BlockingWriter;
        type BlockingLister = oio::BlockingLister;
        type BlockingDeleter = oio::BlockingDeleter;

        fn info(&self) -> Arc<AccessorInfo> {
            let mut info = AccessorInfo::default();
            info.set_native_capability(self.capability);

            info.into()
        }

        async fn stat(&self, _: &str, _: OpStat) -> Result<RpStat> {
            Ok(RpStat::new(Metadata::new(EntryMode::Unknown)))
        }

        async fn read(&self, _: &str, _: OpRead) -> Result<(RpRead, Self::Reader)> {
            Ok((RpRead::new(), Box::new(bytes::Bytes::new())))
        }

        async fn write(&self, _: &str, _: OpWrite) -> Result<(RpWrite, Self::Writer)> {
            Ok((RpWrite::new(), Box::new(MockWriter)))
        }

        async fn list(&self, _: &str, _: OpList) -> Result<(RpList, Self::Lister)> {
            Ok((RpList::default(), Box::new(())))
        }

        async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
            Ok((RpDelete::default(), Box::new(MockDeleter)))
        }
    }

    struct MockWriter;

    impl oio::Write for MockWriter {
        async fn write(&mut self, _: Buffer) -> Result<()> {
            Ok(())
        }

        async fn close(&mut self) -> Result<Metadata> {
            Ok(Metadata::default())
        }

        async fn abort(&mut self) -> Result<()> {
            Ok(())
        }
    }

    struct MockDeleter;

    impl oio::Delete for MockDeleter {
        fn delete(&mut self, _: &str, _: OpDelete) -> Result<()> {
            Ok(())
        }

        async fn flush(&mut self) -> Result<usize> {
            Ok(1)
        }
    }

    fn new_test_operator(capability: Capability) -> Operator {
        let srv = MockService { capability };

        Operator::from_inner(Arc::new(srv)).layer(CorrectnessCheckLayer)
    }

    #[tokio::test]
    async fn test_read() {
        let op = new_test_operator(Capability {
            read: true,
            ..Default::default()
        });
        let res = op.read_with("path").version("version").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            read: true,
            read_with_version: true,
            ..Default::default()
        });
        let res = op.read_with("path").version("version").await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_stat() {
        let op = new_test_operator(Capability {
            stat: true,
            ..Default::default()
        });
        let res = op.stat_with("path").version("version").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            stat: true,
            stat_with_version: true,
            ..Default::default()
        });
        let res = op.stat_with("path").version("version").await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_write_with() {
        let op = new_test_operator(Capability {
            write: true,
            write_with_if_not_exists: true,
            ..Default::default()
        });
        let res = op.write_with("path", "".as_bytes()).append(true).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let res = op
            .write_with("path", "".as_bytes())
            .if_none_match("etag")
            .await;
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Unsupported (permanent) at write => The service memory does not support the operation write with the arguments if_none_match. Please verify if the relevant flags have been enabled, or submit an issue if you believe this is incorrect."
        );

        // Now try a wildcard if-none-match
        let res = op
            .write_with("path", "".as_bytes())
            .if_none_match("*")
            .await;
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
             "Unsupported (permanent) at write, context: { hint: use if_not_exists instead } => The service memory does not support the operation write with the arguments if_none_match. Please verify if the relevant flags have been enabled, or submit an issue if you believe this is incorrect."
        );

        let res = op
            .write_with("path", "".as_bytes())
            .if_not_exists(true)
            .await;
        assert!(res.is_ok());

        let op = new_test_operator(Capability {
            write: true,
            write_can_append: true,
            write_with_if_not_exists: true,
            write_with_if_none_match: true,
            ..Default::default()
        });
        let res = op.writer_with("path").append(true).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_delete() {
        let op = new_test_operator(Capability {
            delete: true,
            ..Default::default()
        });
        let res = op.delete_with("path").version("version").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            delete: true,
            delete_with_version: true,
            ..Default::default()
        });
        let res = op.delete_with("path").version("version").await;
        assert!(res.is_ok())
    }
}
