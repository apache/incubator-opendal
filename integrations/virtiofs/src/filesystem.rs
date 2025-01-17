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
use std::ffi::CStr;
use std::io::Read;
use std::io::Write;
use std::mem::size_of;
use std::sync::Mutex;
use std::time::Duration;

use log::debug;
use log::warn;
use opendal::Buffer;
use opendal::Operator;
use sharded_slab::Slab;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;
use vm_memory::ByteValued;

use crate::buffer::BufferWrapper;
use crate::error::*;
use crate::filesystem_message::*;
use crate::virtiofs_util::Reader;
use crate::virtiofs_util::Writer;

/// Version number of this interface.
const KERNEL_VERSION: u32 = 7;
/// Minor version number of this interface.
const KERNEL_MINOR_VERSION: u32 = 38;
/// Minimum Minor version number supported.
const MIN_KERNEL_MINOR_VERSION: u32 = 27;
/// The length of the header part of the message.
const BUFFER_HEADER_SIZE: u32 = 4096;
/// The maximum length of the data part of the message, used for read/write data.
const MAX_BUFFER_SIZE: u32 = 1 << 20;
/// The default time to live of the attributes.
const DEFAULT_TTL: Duration = Duration::from_secs(1);
/// The default mode of the opened file.
const DEFAULT_OPENED_FILE_MODE: u32 = 0o755;

enum FileType {
    Dir,
    File,
}

struct InnerWriter {
    writer: opendal::Writer,
    written: u64,
}

#[derive(Clone)]
struct OpenedFile {
    path: String,
    metadata: Attr,
}

impl OpenedFile {
    fn new(file_type: FileType, path: &str, uid: u32, gid: u32) -> OpenedFile {
        let mut attr: Attr = unsafe { std::mem::zeroed() };
        attr.uid = uid;
        attr.gid = gid;
        match file_type {
            FileType::Dir => {
                attr.nlink = 2;
                attr.mode = libc::S_IFDIR | DEFAULT_OPENED_FILE_MODE;
            }
            FileType::File => {
                attr.nlink = 1;
                attr.mode = libc::S_IFREG | DEFAULT_OPENED_FILE_MODE;
            }
        }
        OpenedFile {
            path: path.to_string(),
            metadata: attr,
        }
    }
}

fn opendal_metadata2opened_file(
    path: &str,
    metadata: &opendal::Metadata,
    uid: u32,
    gid: u32,
) -> OpenedFile {
    let file_type = match metadata.mode() {
        opendal::EntryMode::DIR => FileType::Dir,
        _ => FileType::File,
    };
    OpenedFile::new(file_type, path, uid, gid)
}

/// Filesystem is a filesystem implementation with opendal backend,
/// and will decode and process messages from VMs.
pub struct Filesystem {
    rt: Runtime,
    core: Operator,
    uid: u32,
    gid: u32,
    opened_files: Slab<OpenedFile>,
    // Since we need to manually manage the allocation of inodes,
    // we record the inode of each opened file here.
    opened_files_map: Mutex<HashMap<String, u64>>,
    opened_files_writer: tokio::sync::Mutex<HashMap<String, InnerWriter>>,
}

impl Filesystem {
    pub fn new(core: Operator) -> Filesystem {
        let rt = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        // Here we set the uid and gid to 1000, which is the default value.
        Filesystem {
            rt,
            core,
            uid: 1000,
            gid: 1000,
            opened_files: Slab::new(),
            opened_files_map: Mutex::new(HashMap::new()),
            opened_files_writer: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn handle_message(&self, mut r: Reader, w: Writer) -> Result<usize> {
        let in_header: InHeader = r.read_obj()?;
        if in_header.len > (MAX_BUFFER_SIZE + BUFFER_HEADER_SIZE) {
            return Filesystem::reply_error(
                in_header.unique,
                w,
                new_unexpected_error("message is too long", None),
            );
        }
        match Opcode::try_from(in_header.opcode) {
            Ok(opcode) => match opcode {
                Opcode::Init => self.init(in_header, r, w),
                Opcode::Destroy => self.destroy(in_header, r, w),
                Opcode::Lookup => self.lookup(in_header, r, w),
                Opcode::Getattr => self.getattr(in_header, r, w),
                Opcode::Setattr => self.setattr(in_header, r, w),
                Opcode::Create => self.create(in_header, r, w),
                Opcode::Unlink => self.unlink(in_header, r, w),
                Opcode::Release => self.release(in_header, r, w),
                Opcode::Flush => self.flush(in_header, r, w),
                Opcode::Forget => self.forget(in_header, r),
                Opcode::Open => self.open(in_header, r, w),
                Opcode::Read => self.read(in_header, r, w),
                Opcode::Write => self.write(in_header, r, w),
            },
            Err(err) => Filesystem::reply_error(in_header.unique, w, err),
        }
    }
}

impl Filesystem {
    fn reply_ok<T: ByteValued>(
        out: Option<T>,
        data: Option<&[u8]>,
        unique: u64,
        mut w: Writer,
    ) -> Result<usize> {
        let mut len = size_of::<OutHeader>();
        if out.is_some() {
            len += size_of::<T>();
        }
        if let Some(data) = data {
            len += data.len();
        }
        let header = OutHeader {
            unique,
            error: 0,
            len: len as u32,
        };
        w.write_all(header.as_slice()).map_err(|e| {
            new_unexpected_error(
                "unexpected error occured while replying to frontend",
                Some(e.into()),
            )
        })?;
        if let Some(out) = out {
            w.write_all(out.as_slice()).map_err(|e| {
                new_unexpected_error(
                    "unexpected error occured while replying to frontend",
                    Some(e.into()),
                )
            })?;
        }
        if let Some(data) = data {
            w.write_all(data).map_err(|e| {
                new_unexpected_error(
                    "unexpected error occured while replying to frontend",
                    Some(e.into()),
                )
            })?;
        }
        Ok(w.bytes_written())
    }

    fn reply_error(unique: u64, mut w: Writer, err: Error) -> Result<usize> {
        // Errors that meet expectations are consumed here.
        warn!(
            "[virtiofs] reply error to frontend: unique={} error={}",
            unique, err
        );
        let header = OutHeader {
            unique,
            error: err.into(),
            len: size_of::<OutHeader>() as u32,
        };
        w.write_all(header.as_slice()).map_err(|e| {
            new_unexpected_error(
                "unexpected error occured while replying to frontend",
                Some(e.into()),
            )
        })?;
        Ok(w.bytes_written())
    }

    fn bytes_to_str(buf: &[u8]) -> Result<&str> {
        CStr::from_bytes_with_nul(buf)
            .map_err(|e| {
                new_unexpected_error(
                    "unexpected error occured while decoding protocol messages",
                    Some(e.into()),
                )
            })?
            .to_str()
            .map_err(|e| {
                new_unexpected_error(
                    "unexpected error occured while decoding protocol messages",
                    Some(e.into()),
                )
            })
    }

    fn check_flags(&self, flags: u32) -> Result<(bool, bool)> {
        let is_trunc = flags & libc::O_TRUNC as u32 != 0 || flags & libc::O_CREAT as u32 != 0;
        let is_append = flags & libc::O_APPEND as u32 != 0;

        let mode = flags & libc::O_ACCMODE as u32;
        let is_write = mode == libc::O_WRONLY as u32 || mode == libc::O_RDWR as u32 || is_append;

        let capability = self.core.info().full_capability();
        if is_trunc && !capability.write {
            Err(Error::from(libc::EACCES))?;
        }
        if is_append && !capability.write_can_append {
            Err(Error::from(libc::EACCES))?;
        }

        Ok((is_write, is_append))
    }
}

impl Filesystem {
    fn init(&self, in_header: InHeader, mut r: Reader, w: Writer) -> Result<usize> {
        let InitIn { major, minor, .. } = r.read_obj()?;

        if major != KERNEL_VERSION || minor < MIN_KERNEL_MINOR_VERSION {
            return Filesystem::reply_error(
                in_header.unique,
                w,
                new_unexpected_error("unsupported virtiofs version", None),
            );
        }

        let mut attr = OpenedFile::new(FileType::Dir, "/", self.uid, self.gid);
        attr.metadata.ino = 1;
        // We need to allocate the inode 1 for the root directory. The double insertion
        // here makes 1 the first inode and avoids extra alignment and processing elsewhere.
        self.opened_files.insert(attr.clone());
        self.opened_files.insert(attr.clone());
        let mut opened_files_map = self.opened_files_map.lock().unwrap();
        opened_files_map.insert("/".to_string(), 1);

        let out = InitOut {
            major: KERNEL_VERSION,
            minor: KERNEL_MINOR_VERSION,
            max_write: MAX_BUFFER_SIZE,
            ..Default::default()
        };
        Filesystem::reply_ok(Some(out), None, in_header.unique, w)
    }

    fn destroy(&self, _in_header: InHeader, _r: Reader, _w: Writer) -> Result<usize> {
        // do nothing for destroy.
        Ok(0)
    }

    fn flush(&self, _in_header: InHeader, _r: Reader, _w: Writer) -> Result<usize> {
        // do nothing for flush.
        Ok(0)
    }

    fn forget(&self, _in_header: InHeader, _r: Reader) -> Result<usize> {
        // do nothing for forget.
        Ok(0)
    }

    fn lookup(&self, in_header: InHeader, mut r: Reader, w: Writer) -> Result<usize> {
        let name_len = in_header.len as usize - size_of::<InHeader>();
        let mut buf = vec![0; name_len];
        r.read_exact(&mut buf).map_err(|e| {
            new_unexpected_error(
                "unexpected error occured while decoding protocol messages",
                Some(e.into()),
            )
        })?;
        let name = match Filesystem::bytes_to_str(buf.as_ref()) {
            Ok(name) => name,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        debug!("lookup: parent inode={} name={}", in_header.nodeid, name);

        let parent_path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        let path = format!("{}/{}", parent_path, name);
        let metadata = match self.rt.block_on(self.do_get_metadata(&path)) {
            Ok(metadata) => metadata,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        let out = EntryOut {
            nodeid: metadata.metadata.ino,
            entry_valid: DEFAULT_TTL.as_secs(),
            attr_valid: DEFAULT_TTL.as_secs(),
            entry_valid_nsec: DEFAULT_TTL.subsec_nanos(),
            attr_valid_nsec: DEFAULT_TTL.subsec_nanos(),
            attr: metadata.metadata,
            ..Default::default()
        };
        Filesystem::reply_ok(Some(out), None, in_header.unique, w)
    }

    fn getattr(&self, in_header: InHeader, _r: Reader, w: Writer) -> Result<usize> {
        debug!("getattr: inode={}", in_header.nodeid);

        let path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        let metadata = match self.rt.block_on(self.do_get_metadata(&path)) {
            Ok(metadata) => metadata,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        let out = AttrOut {
            attr_valid: DEFAULT_TTL.as_secs(),
            attr_valid_nsec: DEFAULT_TTL.subsec_nanos(),
            attr: metadata.metadata,
            ..Default::default()
        };
        Filesystem::reply_ok(Some(out), None, in_header.unique, w)
    }

    fn setattr(&self, in_header: InHeader, _r: Reader, w: Writer) -> Result<usize> {
        debug!("setattr: inode={}", in_header.nodeid);

        // do nothing for setattr.
        self.getattr(in_header, _r, w)
    }

    fn create(&self, in_header: InHeader, mut r: Reader, w: Writer) -> Result<usize> {
        let CreateIn { flags, .. } = r.read_obj()?;

        let name_len = in_header.len as usize - size_of::<InHeader>() - size_of::<CreateIn>();
        let mut buf = vec![0; name_len];
        r.read_exact(&mut buf).map_err(|e| {
            new_unexpected_error(
                "unexpected error occured while decoding protocol messages",
                Some(e.into()),
            )
        })?;
        let name = match Filesystem::bytes_to_str(buf.as_ref()) {
            Ok(name) => name,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        debug!("create: parent inode={} name={}", in_header.nodeid, name);

        let parent_path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        let path = format!("{}/{}", parent_path, name);
        let mut attr = OpenedFile::new(FileType::File, &path, self.uid, self.gid);
        let inode = self
            .opened_files
            .insert(attr.clone())
            .expect("failed to allocate inode");
        attr.metadata.ino = inode as u64;
        let mut opened_files_map = self.opened_files_map.lock().unwrap();
        opened_files_map.insert(path.to_string(), inode as u64);

        match self.rt.block_on(self.do_set_writer(&path, flags)) {
            Ok(writer) => writer,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        let entry_out = EntryOut {
            nodeid: attr.metadata.ino,
            entry_valid: DEFAULT_TTL.as_secs(),
            attr_valid: DEFAULT_TTL.as_secs(),
            entry_valid_nsec: DEFAULT_TTL.subsec_nanos(),
            attr_valid_nsec: DEFAULT_TTL.subsec_nanos(),
            attr: attr.metadata,
            ..Default::default()
        };
        let open_out = OpenOut {
            ..Default::default()
        };
        Filesystem::reply_ok(
            Some(entry_out),
            Some(open_out.as_slice()),
            in_header.unique,
            w,
        )
    }

    fn unlink(&self, in_header: InHeader, mut r: Reader, w: Writer) -> Result<usize> {
        let name_len = in_header.len as usize - size_of::<InHeader>();
        let mut buf = vec![0; name_len];
        r.read_exact(&mut buf).map_err(|e| {
            new_unexpected_error(
                "unexpected error occured while decoding protocol messages",
                Some(e.into()),
            )
        })?;
        let name = match Filesystem::bytes_to_str(buf.as_ref()) {
            Ok(name) => name,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        debug!("unlink: parent inode={} name={}", in_header.nodeid, name);

        let parent_path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        let path = format!("{}/{}", parent_path, name);
        if let Err(err) = self.rt.block_on(self.do_delete(&path)) {
            return Filesystem::reply_error(in_header.unique, w, err);
        }

        let mut opened_files_map = self.opened_files_map.lock().unwrap();
        opened_files_map.remove(&path);

        Filesystem::reply_ok(None::<u8>, None, in_header.unique, w)
    }

    fn release(&self, in_header: InHeader, _r: Reader, w: Writer) -> Result<usize> {
        debug!("release: inode={}", in_header.nodeid);

        let path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        if let Err(err) = self.rt.block_on(self.do_release_writer(&path)) {
            return Filesystem::reply_error(in_header.unique, w, err);
        }

        Filesystem::reply_ok(None::<u8>, None, in_header.unique, w)
    }

    fn open(&self, in_header: InHeader, mut r: Reader, w: Writer) -> Result<usize> {
        debug!("open: inode={}", in_header.nodeid);

        let OpenIn { flags, .. } = r.read_obj()?;

        let path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        match self.rt.block_on(self.do_set_writer(&path, flags)) {
            Ok(writer) => writer,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        let out = OpenOut {
            ..Default::default()
        };
        Filesystem::reply_ok(Some(out), None, in_header.unique, w)
    }

    fn read(&self, in_header: InHeader, mut r: Reader, mut w: Writer) -> Result<usize> {
        let path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        let ReadIn { offset, size, .. } = r.read_obj()?;

        debug!(
            "read: inode={} offset={} size={}",
            in_header.nodeid, offset, size
        );

        let data = match self.rt.block_on(self.do_read(&path, offset)) {
            Ok(data) => data,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };
        let len = data.len();
        let buffer = BufferWrapper::new(data);

        let mut data_writer = w.split_at(size_of::<OutHeader>());
        data_writer.write_from_at(&buffer, len);

        let out = OutHeader {
            len: (size_of::<OutHeader>() + len) as u32,
            error: 0,
            unique: in_header.unique,
        };
        w.write_all(out.as_slice()).map_err(|e| {
            new_unexpected_error(
                "unexpected error occured while replying to frontend",
                Some(e.into()),
            )
        })?;
        Ok(out.len as usize)
    }

    fn write(&self, in_header: InHeader, mut r: Reader, w: Writer) -> Result<usize> {
        debug!("write: inode={}", in_header.nodeid);

        let path = match self
            .opened_files
            .get(in_header.nodeid as usize)
            .map(|f| f.path.clone())
        {
            Some(path) => path,
            None => return Filesystem::reply_error(in_header.unique, w, Error::from(libc::ENOENT)),
        };

        let WriteIn { offset, size, .. } = r.read_obj()?;

        let buffer = BufferWrapper::new(Buffer::new());
        r.read_to_at(&buffer, size as usize);
        let buffer = buffer.get_buffer();

        match self.rt.block_on(self.do_write(&path, offset, buffer)) {
            Ok(writer) => writer,
            Err(err) => return Filesystem::reply_error(in_header.unique, w, err),
        };

        let out = WriteOut {
            size,
            ..Default::default()
        };
        Filesystem::reply_ok(Some(out), None, in_header.unique, w)
    }
}

impl Filesystem {
    async fn do_get_metadata(&self, path: &str) -> Result<OpenedFile> {
        let metadata = self.core.stat(path).await.map_err(|err| Error::from(err))?;
        let mut attr = opendal_metadata2opened_file(path, &metadata, self.uid, self.gid);
        attr.metadata.size = metadata.content_length();
        let mut opened_files_map = self.opened_files_map.lock().unwrap();
        if let Some(inode) = opened_files_map.get(path) {
            attr.metadata.ino = *inode;
        } else {
            let inode = self
                .opened_files
                .insert(attr.clone())
                .expect("failed to allocate inode");
            attr.metadata.ino = inode as u64;
            opened_files_map.insert(path.to_string(), inode as u64);
        }

        Ok(attr)
    }

    async fn do_set_writer(&self, path: &str, flags: u32) -> Result<()> {
        let (is_write, is_append) = self.check_flags(flags)?;
        if !is_write {
            return Ok(());
        }

        let writer = self
            .core
            .writer_with(path)
            .append(is_append)
            .await
            .map_err(|err| Error::from(err))?;
        let written = if is_append {
            self.core
                .stat(path)
                .await
                .map_err(|err| Error::from(err))?
                .content_length()
        } else {
            0
        };

        let inner_writer = InnerWriter { writer, written };
        let mut opened_file_writer = self.opened_files_writer.lock().await;
        opened_file_writer.insert(path.to_string(), inner_writer);

        Ok(())
    }

    async fn do_release_writer(&self, path: &str) -> Result<()> {
        let mut opened_file_writer = self.opened_files_writer.lock().await;
        let inner_writer = opened_file_writer
            .get_mut(path)
            .ok_or(Error::from(libc::EINVAL))?;
        inner_writer
            .writer
            .close()
            .await
            .map_err(|err| Error::from(err))?;
        opened_file_writer.remove(path);

        Ok(())
    }

    async fn do_delete(&self, path: &str) -> Result<()> {
        self.core
            .delete(path)
            .await
            .map_err(|err| Error::from(err))?;

        Ok(())
    }

    async fn do_read(&self, path: &str, offset: u64) -> Result<Buffer> {
        let data = self
            .core
            .read_with(path)
            .range(offset..)
            .await
            .map_err(|err| Error::from(err))?;

        Ok(data)
    }

    async fn do_write(&self, path: &str, offset: u64, data: Buffer) -> Result<usize> {
        let len = data.len();
        let mut opened_file_writer = self.opened_files_writer.lock().await;
        let inner_writer = opened_file_writer
            .get_mut(path)
            .ok_or(Error::from(libc::EINVAL))?;
        if offset != inner_writer.written {
            return Err(Error::from(libc::EINVAL));
        }
        inner_writer
            .writer
            .write_from(data)
            .await
            .map_err(|err| Error::from(err))?;
        inner_writer.written += len as u64;

        Ok(len)
    }
}
