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
use std::path::Path;
use std::path::PathBuf;

use compio::buf::{IoBuf, IoVectoredBuf};
use compio::dispatcher::Dispatcher;

use opendal_core::raw::*;
use opendal_core::*;

// Wrapper type to avoid orphan rules
#[derive(Debug, Clone)]
pub struct CompfsBuffer(pub Vec<compio::bytes::Bytes>);

impl IoBuf for CompfsBuffer {
    fn as_init(&self) -> &[u8] {
        self.0.first().map_or(&[], |b| b.as_ref())
    }
}

impl From<CompfsBuffer> for opendal_core::Buffer {
    fn from(buf: CompfsBuffer) -> Self {
        buf.0.into()
    }
}

impl From<opendal_core::Buffer> for CompfsBuffer {
    fn from(mut buf: opendal_core::Buffer) -> Self {
        Self(buf.by_ref().collect())
    }
}

impl IoVectoredBuf for CompfsBuffer {
    fn iter_slice(&self) -> impl Iterator<Item = &[u8]> {
        self.0.iter().map(|b| b.as_ref())
    }
}

#[derive(Debug)]
pub(super) struct CompfsCore {
    pub info: ServiceInfo,
    pub capability: Capability,

    pub root: PathBuf,
    pub dispatcher: Dispatcher,
    pub buf_pool: oio::PooledBuf,
}

impl CompfsCore {
    /// Reject `..` traversal and absolute keys so they cannot escape `root`,
    /// matching the `fs` backend (#7684). Confinement lives here because
    /// `normalize_path` deliberately leaves `.`/`..` unresolved (RFC 0112) and
    /// `PathBuf::join` discards the base when the key is absolute.
    pub fn prepare_path(&self, path: &str) -> Result<PathBuf> {
        use std::path::Component;
        let trimmed = path.trim_end_matches('/');
        if Path::new(trimmed).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(
                Error::new(ErrorKind::NotFound, "path escapes the configured root")
                    .with_context("path", path),
            );
        }
        Ok(self.root.join(trimmed))
    }

    pub async fn exec<Fn, Fut, R>(&self, f: Fn) -> opendal_core::Result<R>
    where
        Fn: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = std::io::Result<R>> + 'static,
        R: Send + 'static,
    {
        self.dispatcher
            .dispatch(f)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "compio spawn io task failed"))?
            .await
            .map_err(|_| Error::new(ErrorKind::Unexpected, "compio task cancelled"))?
            .map_err(new_std_io_error)
    }

    pub async fn exec_blocking<Fn, R>(&self, f: Fn) -> Result<R>
    where
        Fn: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.dispatcher
            .dispatch_blocking(f)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "compio spawn blocking task failed"))?
            .await
            .map_err(|_| Error::new(ErrorKind::Unexpected, "compio task cancelled"))
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use rand::{RngExt, rng};

    use super::*;

    fn setup_buffer() -> (CompfsBuffer, usize, Bytes) {
        let mut rng = rng();

        let bs = (0..100)
            .map(|_| {
                let len = rng.random_range(1..100);
                let mut buf = vec![0; len];
                rng.fill(&mut buf[..]);
                Bytes::from(buf)
            })
            .collect::<Vec<_>>();

        let total_size = bs.iter().map(|b| b.len()).sum::<usize>();
        let total_content = bs.iter().flatten().copied().collect::<Bytes>();
        let buf = Buffer::from(bs);

        (CompfsBuffer::from(buf), total_size, total_content)
    }

    #[test]
    fn test_io_buf() {
        let (buf, _len, _bytes) = setup_buffer();
        let slice = IoBuf::as_init(&buf);

        assert_eq!(slice, buf.0.first().unwrap().as_ref())
    }

    #[test]
    fn test_io_vectored_buf() {
        let (buf, len, bytes) = setup_buffer();
        let collected = buf.iter_slice().flatten().copied().collect::<Bytes>();

        assert_eq!(buf.total_len(), len);
        assert_eq!(collected, bytes);
    }

    fn new_test_core() -> CompfsCore {
        CompfsCore {
            info: ServiceInfo::new("compfs", "", ""),
            capability: Capability::default(),
            root: PathBuf::from("/data/root"),
            dispatcher: Dispatcher::new().unwrap(),
            buf_pool: oio::PooledBuf::new(16),
        }
    }

    #[test]
    fn test_prepare_path_rejects_parent_dir() {
        let core = new_test_core();
        for key in ["../etc/passwd", "../../etc/passwd", "a/../../b", "a/.."] {
            let err = core.prepare_path(key).unwrap_err();
            assert_eq!(
                err.kind(),
                ErrorKind::NotFound,
                "key should be rejected: {key}"
            );
        }
    }

    #[test]
    fn test_prepare_path_rejects_absolute_path() {
        let core = new_test_core();
        // `PathBuf::join` drops the root when the key is absolute.
        for key in ["/etc/passwd", "//etc/passwd"] {
            let err = core.prepare_path(key).unwrap_err();
            assert_eq!(
                err.kind(),
                ErrorKind::NotFound,
                "key should be rejected: {key}"
            );
        }
    }

    #[test]
    fn test_prepare_path_allows_normal_keys() {
        let core = new_test_core();
        // Normal keys, `.` (CurDir), and trailing slashes resolve unchanged.
        assert_eq!(
            core.prepare_path("a/b.txt").unwrap(),
            PathBuf::from("/data/root/a/b.txt")
        );
        assert_eq!(
            core.prepare_path("a/b/").unwrap(),
            PathBuf::from("/data/root/a/b")
        );
        assert_eq!(
            core.prepare_path("./a/b").unwrap(),
            PathBuf::from("/data/root/a/b")
        );
        // A key containing `..` only as a substring of a name is not a traversal.
        assert_eq!(
            core.prepare_path("a..b").unwrap(),
            PathBuf::from("/data/root/a..b")
        );
    }
}
