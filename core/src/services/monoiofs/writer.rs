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

use bytes::Buf;
use bytes::Bytes;
use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::SinkExt;
use futures::StreamExt;
use monoio::fs::OpenOptions;

use super::core::MonoiofsCore;
use crate::raw::*;
use crate::*;

enum WriterRequest {
    Write {
        pos: u64,
        buf: Bytes,
        tx: oneshot::Sender<Result<()>>,
    },
    Close {
        tx: oneshot::Sender<Result<()>>,
    },
}

pub struct MonoiofsWriter {
    core: Arc<MonoiofsCore>,
    tx: mpsc::UnboundedSender<WriterRequest>,
    pos: u64,
}

impl MonoiofsWriter {
    pub async fn new(core: Arc<MonoiofsCore>, path: PathBuf, append: bool) -> Result<Self> {
        let (open_result_tx, open_result_rx) = oneshot::channel();
        let (tx, rx) = mpsc::unbounded();
        core.spawn(move || Self::worker_entrypoint(path, append, rx, open_result_tx))
            .await;
        core.unwrap(open_result_rx.await)?;
        Ok(Self { core, tx, pos: 0 })
    }

    /// entrypoint of worker task that runs in context of monoio
    async fn worker_entrypoint(
        path: PathBuf,
        append: bool,
        mut rx: mpsc::UnboundedReceiver<WriterRequest>,
        open_result_tx: oneshot::Sender<Result<()>>,
    ) {
        let result = OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path)
            .await;
        // [`monoio::fs::File`] is non-Send, hence it is kept within
        // worker thread
        let file = match result {
            Ok(file) => {
                let Ok(()) = open_result_tx.send(Ok(())) else {
                    // MonoiofsWriter::new is cancelled, exit worker task
                    return;
                };
                file
            }
            Err(e) => {
                // discard the result if send failed due to MonoiofsWriter::new
                // cancelled since we are going to exit anyway
                let _ = open_result_tx.send(Err(new_std_io_error(e)));
                return;
            }
        };
        // wait for write or close request and send back result to main thread
        loop {
            let Some(req) = rx.next().await else {
                // MonoiofsWriter is dropped, exit worker task
                break;
            };
            match req {
                WriterRequest::Write { pos, buf, tx } => {
                    let (result, _) = file.write_all_at(buf, pos).await;
                    // discard the result if send failed due to
                    // MonoiofsWriter::write cancelled
                    let _ = tx.send(result.map_err(new_std_io_error));
                }
                WriterRequest::Close { tx } => {
                    let result = file.sync_all().await;
                    // discard the result if send failed due to
                    // MonoiofsWriter::close cancelled
                    let _ = tx.send(result.map_err(new_std_io_error));
                    // file is closed in background and result is useless
                    let _ = file.close().await;
                    break;
                }
            }
        }
    }
}

impl oio::Write for MonoiofsWriter {
    /// Send write request to worker thread and wait for result. Actual
    /// write happens in [`MonoiofsWriter::worker_entrypoint`] running
    /// on worker thread.
    async fn write(&mut self, mut bs: Buffer) -> Result<()> {
        while bs.has_remaining() {
            let buf = bs.current();
            let n = buf.len();
            let (tx, rx) = oneshot::channel();
            self.core.unwrap(
                self.tx
                    .send(WriterRequest::Write {
                        pos: self.pos,
                        buf,
                        tx,
                    })
                    .await,
            );
            self.core.unwrap(rx.await)?;
            self.pos += n as u64;
            bs.advance(n);
        }
        Ok(())
    }

    /// Send close request to worker thread and wait for result. Actual
    /// close happens in [`MonoiofsWriter::worker_entrypoint`] running
    /// on worker thread.
    async fn close(&mut self) -> Result<Metadata> {
        let (tx, rx) = oneshot::channel();
        self.core
            .unwrap(self.tx.send(WriterRequest::Close { tx }).await);
        self.core.unwrap(rx.await)?;

        Ok(Metadata::default())
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "Monoiofs doesn't support abort",
        ))
    }
}
