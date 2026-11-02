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
use std::io::SeekFrom;

use bytes::{Buf, Bytes, BytesMut};
use dav_server::fs::{DavFile, OpenOptions};
use dav_server::fs::{DavMetaData, FsResult};
use dav_server::fs::{FsError, FsFuture};
use futures::FutureExt;
use futures::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use opendal_core::{FuturesAsyncReader, FuturesAsyncWriter, Operator};

use super::metadata::OpendalMetaData;
use super::utils::*;

/// OpendalFile is a `DavFile` implementation for opendal.
pub struct OpendalFile {
    op: Operator,
    path: String,
    state: State,
    buf: BytesMut,
}

impl Debug for OpendalFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpendalFile")
            .field("path", &self.path)
            .field(
                "state",
                match &self.state {
                    State::Read { .. } => &"read",
                    State::Write(_) => &"write",
                },
            )
            .finish()
    }
}

enum State {
    Read(FuturesAsyncReader),
    Write(FuturesAsyncWriter),
}

impl OpendalFile {
    /// Create a new opendal file.
    pub async fn open(op: Operator, path: String, options: OpenOptions) -> FsResult<Self> {
        let state = if options.read {
            let r = op
                .reader(&path)
                .await
                .map_err(convert_error)?
                .into_futures_async_read(..)
                .await
                .map_err(convert_error)?;
            State::Read(r)
        } else if options.write {
            let w = op
                .writer_with(&path)
                .append(options.append)
                .if_not_exists(options.create_new)
                .await
                .map_err(convert_error)?
                .into_futures_async_write();
            State::Write(w)
        } else {
            return Err(FsError::NotImplemented);
        };

        Ok(Self {
            op,
            path,
            state,
            buf: BytesMut::new(),
        })
    }
}

impl DavFile for OpendalFile {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        async move {
            self.op
                .stat(&self.path)
                .await
                .map(|opendal_metadata| {
                    Box::new(OpendalMetaData::new(opendal_metadata)) as Box<dyn DavMetaData>
                })
                .map_err(convert_error)
        }
        .boxed()
    }

    fn write_buf(&mut self, mut buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        async move {
            let State::Write(w) = &mut self.state else {
                return Err(FsError::GeneralFailure);
            };

            w.write_all(&buf.copy_to_bytes(buf.remaining()))
                .await
                .map_err(|_| FsError::GeneralFailure)?;
            Ok(())
        }
        .boxed()
    }

    fn write_bytes(&mut self, buf: Bytes) -> FsFuture<'_, ()> {
        async move {
            let State::Write(w) = &mut self.state else {
                return Err(FsError::GeneralFailure);
            };

            w.write_all(&buf).await.map_err(|_| FsError::GeneralFailure)
        }
        .boxed()
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, Bytes> {
        async move {
            let State::Read(r) = &mut self.state else {
                return Err(FsError::GeneralFailure);
            };

            self.buf.resize(count, 0);
            let len = r
                .read(&mut self.buf)
                .await
                .map_err(|_| FsError::GeneralFailure)?;
            Ok(self.buf.split_to(len).freeze())
        }
        .boxed()
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        async move {
            let State::Read(r) = &mut self.state else {
                return Err(FsError::GeneralFailure);
            };

            r.seek(pos).await.map_err(|_| FsError::GeneralFailure)
        }
        .boxed()
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        async move {
            let State::Write(w) = &mut self.state else {
                return Err(FsError::GeneralFailure);
            };

            w.flush().await.map_err(|_| FsError::GeneralFailure)?;
            w.close().await.map_err(|_| FsError::GeneralFailure)
        }
        .boxed()
    }
}
