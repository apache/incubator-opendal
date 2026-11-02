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

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use goosefs_sdk::io::GoosefsFileWriter as ClientWriter;

use super::core::GoosefsCore;
use super::core::{ErrorContext, parse_error};
use opendal_core::raw::*;
use opendal_core::*;

pub type GoosefsWriters = GoosefsWriter;

/// Process-wide monotonic counter used to disambiguate temporary file
/// names within a single process. Combined with the PID and a
/// nanosecond timestamp, this gives us a collision-free temporary
/// suffix without pulling in a UUID dependency.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `GoosefsWriter` implements [`oio::Write`] on top of the goosefs-sdk
/// high-level streaming writer (`GoosefsFileWriter`).
///
/// # Write-via-temp protocol
///
/// To keep in-flight writes from being visible — and to make `abort()`
/// truly leave no trace — every write first streams into a sibling
/// temporary path, and is then renamed onto the caller's target in a
/// single metadata RPC on `close()`:
///
/// 1.  **open**: on the very first `write()` / `close()` call, we pick
///     a temporary path of the form
///     `"{dir}/.opendal.tmp.{pid}.{counter}.{nanos}.{basename}"`. The
///     temp name is intentionally produced in the **same parent
///     directory** as the final target so we stay on the same UFS
///     mount — this keeps the final `rename` a pure metadata op on
///     GooseFS and avoids a cross-filesystem data copy.
/// 2.  **stream**: bytes are written block-by-block into the temp path
///     via `GoosefsFileWriter`. We iterate over the incoming `Buffer`
///     chunks directly instead of flattening them with `to_bytes()`,
///     so multi-segment buffers don't force an extra concat copy.
/// 3.  **finalize**: `close()` calls `GoosefsFileWriter::close()` (which
///     commits the temp file on the master), then removes any
///     pre-existing object at the final path (overwrite) or returns
///     `ConditionNotMatch` (if-not-exists), and finally
///     `rename(temp, final)`.
/// 4.  **abort**: `abort()` cancels the in-flight SDK writer (which
///     drops any blocks already staged on workers) and removes the
///     temp inode on the master. The final target is never touched
///     in the abort path — by construction we haven't published
///     anything there yet.
///
/// This avoids the old failure mode where an interrupted write could
/// leave a half-written blob at the caller's final path, and it makes
/// overwrite vs if-not-exists decisions race only against concurrent
/// *metadata* mutations on the final path (a small, bounded window
/// around the `rename`).
///
/// # Why check if-not-exists twice
///
/// We check `if_not_exists` both up front (before opening the SDK
/// writer, so we don't waste bytes on a temp that will never be
/// finalized) and again at finalize time (because another writer may
/// have created the object while we were streaming). The second
/// check is what actually enforces the contract; the first is a
/// best-effort fast-path optimisation.
pub struct GoosefsWriter {
    core: Arc<GoosefsCore>,
    op: OpWrite,
    /// Final destination path the caller asked us to write to.
    path: String,
    /// Sibling temporary path we actually stream into; materialised
    /// lazily on the first `write()` / `close()` call so that an
    /// `abort()` on a writer that never wrote anything is free.
    tmp_path: Option<String>,
    /// Lazily initialized SDK streaming writer, opened against
    /// `tmp_path`.
    writer: Option<ClientWriter>,
}

impl GoosefsWriter {
    pub fn new(core: Arc<GoosefsCore>, op: OpWrite, path: String) -> Self {
        GoosefsWriter {
            core,
            op,
            path,
            tmp_path: None,
            writer: None,
        }
    }

    /// Produce a sibling temporary path for `path`.
    ///
    /// The temp name is always placed in the parent directory of the
    /// final target so that the finalising `rename(temp, final)` stays
    /// on the same UFS mount and therefore resolves to a pure metadata
    /// operation on the GooseFS master (no data-plane copy across
    /// mounts). Format:
    ///
    /// ```text
    /// {dir}/.opendal.tmp.{pid}.{counter}.{nanos}.{basename}
    /// ```
    ///
    /// The `.opendal.tmp.` prefix makes leftover temps trivial to
    /// identify and sweep up; including PID + a monotonic counter +
    /// nanosecond timestamp guarantees uniqueness even under PID
    /// reuse or rapid successive writes from the same process.
    fn make_tmp_path(path: &str) -> String {
        // Split "dir/base". If there's no '/', dir is empty and base
        // is the whole path — this matches GooseFS's root-level file
        // case (e.g. `/foo` under the master's mount root).
        let (dir, base) = match path.rfind('/') {
            Some(idx) => (&path[..idx], &path[idx + 1..]),
            None => ("", path),
        };

        let pid = std::process::id();
        let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        if dir.is_empty() {
            format!(".opendal.tmp.{pid}.{counter}.{nanos}.{base}")
        } else {
            format!("{dir}/.opendal.tmp.{pid}.{counter}.{nanos}.{base}")
        }
    }

    /// Lazily open the SDK writer against a freshly-allocated temp
    /// path, and cache both.
    async fn ensure_writer(&mut self) -> Result<&mut ClientWriter> {
        if self.writer.is_none() {
            // Fast-path only. Authority is GoosefsCore::rename(..., if_not_exists=true)
            // → Master no-replace rename (DefaultFileSystemMaster →
            // FileAlreadyExistsException → gRPC ALREADY_EXISTS → ConditionNotMatch).
            // Not this get_status.
            if self.op.if_not_exists() && self.core.get_status(&self.path).await.is_ok() {
                return Err(Error::new(
                    ErrorKind::ConditionNotMatch,
                    "target already exists and if_not_exists was set",
                ));
            }

            let tmp = Self::make_tmp_path(&self.path);
            let w = self.core.create_writer(&tmp).await?;
            self.tmp_path = Some(tmp);
            self.writer = Some(w);
        }
        Ok(self.writer.as_mut().expect("just ensured"))
    }

    /// Finalize the temp file onto the caller's target path.
    ///
    /// Preconditions: `self.writer` has been `close()`d (or was never
    /// opened), and `self.tmp_path` names the committed temp inode on
    /// the master. Postconditions: on success, the temp inode has
    /// been renamed onto `self.path` and `self.tmp_path` is cleared;
    /// on failure, the temp inode is best-effort removed so we do not
    /// leak `.opendal.tmp.*` files on the UFS.
    async fn finalize_rename(&mut self) -> Result<()> {
        let Some(tmp) = self.tmp_path.take() else {
            // Nothing was ever written — `close()` handles the
            // "create an empty object" contract on its own path.
            return Ok(());
        };

        // Optional fast-path (optimization, not authority). Safe to keep:
        // a miss here is closed by rename(..., if_not_exists) below.
        if self.op.if_not_exists() && self.core.get_status(&self.path).await.is_ok() {
            // Undo our staged temp before surfacing the error.
            let _ = self.core.delete(&tmp).await;
            return Err(Error::new(
                ErrorKind::ConditionNotMatch,
                "target already exists and if_not_exists was set",
            ));
        }

        // Authority: pass OpWrite.if_not_exists into core.rename.
        // Overwrite delete lives only inside core.rename when if_not_exists=false.
        match self
            .core
            .rename(&tmp, &self.path, self.op.if_not_exists())
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                // Rename failed — sweep the temp so we don't leak a
                // stray `.opendal.tmp.*` inode. Best-effort: if the
                // sweep fails, we still surface the original rename
                // error to the caller.
                let _ = self.core.delete(&tmp).await;
                Err(e)
            }
        }
    }
}

impl oio::Write for GoosefsWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let writer = self.ensure_writer().await?;

        // Iterate chunks directly instead of `bs.to_bytes()` — the
        // latter concatenates multi-segment buffers into a single
        // `Bytes`, doubling peak memory for the (common) case where
        // the caller already handed us a split buffer. `Buffer`'s
        // Iterator yields one `Bytes` per segment; the SDK writer
        // takes `&[u8]`, so each chunk flows through without a copy.
        for chunk in bs {
            writer
                .write(&chunk)
                .await
                .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Write")), err))?;
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        if let Some(mut writer) = self.writer.take() {
            // Commit the temp inode on the master, then publish it
            // onto the caller's final path via `finalize_rename`.
            if let Err(e) = writer
                .close()
                .await
                .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Close")), err))
            {
                // Close failed — the temp is in an indeterminate state
                // on the master; best-effort sweep to avoid leaks.
                if let Some(tmp) = self.tmp_path.take() {
                    let _ = self.core.delete(&tmp).await;
                }
                return Err(e);
            }
            // Capture file_id before rename; Master rename keeps the same inode id.
            let mut meta = MetadataBuilder::unknown();
            if let Some(fid) = writer.file_info().file_id {
                meta.etag(fid.to_string());
            }
            self.finalize_rename().await?;
            return Ok(meta.build());
        }

        // Zero-write path: the caller closed without ever calling
        // `write()`. OpenDAL's contract for `write(path, "")` is to
        // materialise an empty object at `path`. We honour it by
        // opening and immediately closing a writer — still through
        // the temp-and-rename pipeline, so overwrite / if-not-exists
        // semantics remain identical to the streaming path.
        if self.op.if_not_exists() && self.core.get_status(&self.path).await.is_ok() {
            return Err(Error::new(
                ErrorKind::ConditionNotMatch,
                "target already exists and if_not_exists was set",
            ));
        }

        let tmp = Self::make_tmp_path(&self.path);
        let mut w = self.core.create_writer(&tmp).await?;
        if let Err(e) = w
            .close()
            .await
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Close")), err))
        {
            let _ = self.core.delete(&tmp).await;
            return Err(e);
        }
        let mut meta = MetadataBuilder::unknown();
        if let Some(fid) = w.file_info().file_id {
            meta.etag(fid.to_string());
        }
        self.tmp_path = Some(tmp);
        self.finalize_rename().await?;
        Ok(meta.build())
    }

    async fn abort(&mut self) -> Result<()> {
        // Temp-and-rename makes abort straightforward: the caller's
        // final path has *never* been touched, so there is nothing to
        // roll back there. All we need to do is tear down the staged
        // temp, both on the data plane (in-flight gRPC stream +
        // blocks on workers) and on the metadata plane (temp inode on
        // the master).
        if let Some(mut writer) = self.writer.take() {
            // `cancel()` tears down the in-flight bidi gRPC stream,
            // drops any blocks already committed to workers via
            // `remove_blocks`, and issues `DeleteOptions::for_cancel`
            // against the master (i.e. `unchecked=true`) so the
            // INCOMPLETE inode is reaped. We swallow its error here
            // because (a) the caller is abandoning the write and a
            // stray temp block is less disruptive than a panic, and
            // (b) the follow-up `delete(tmp)` below is our
            // belt-and-braces guarantee that the temp inode is gone.
            let _ = writer.cancel().await;
        }

        if let Some(tmp) = self.tmp_path.take() {
            // Best-effort sweep — NotFound means `cancel()` already
            // reaped it, which is the expected happy path.
            let _ = self.core.delete(&tmp).await;
        }
        Ok(())
    }
}
