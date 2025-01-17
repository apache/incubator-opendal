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

use vm_memory::ByteValued;

use crate::error::*;

/// Opcode represents the filesystem call that needs to be executed by VMs message.
/// The corresponding value needs to be aligned with the specification.
#[non_exhaustive]
pub enum Opcode {
    Lookup = 1,
    Forget = 2,
    Getattr = 3,
    Setattr = 4,
    Unlink = 10,
    Open = 14,
    Read = 15,
    Write = 16,
    Release = 18,
    Flush = 25,
    Init = 26,
    Create = 35,
    Destroy = 38,
}

impl TryFrom<u32> for Opcode {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Opcode::Lookup),
            2 => Ok(Opcode::Forget),
            3 => Ok(Opcode::Getattr),
            4 => Ok(Opcode::Setattr),
            10 => Ok(Opcode::Unlink),
            14 => Ok(Opcode::Open),
            15 => Ok(Opcode::Read),
            16 => Ok(Opcode::Write),
            18 => Ok(Opcode::Release),
            25 => Ok(Opcode::Flush),
            26 => Ok(Opcode::Init),
            35 => Ok(Opcode::Create),
            38 => Ok(Opcode::Destroy),
            _ => Err(new_unexpected_error("unsupported opcode", None)),
        }
    }
}

/// Attr represents the file attributes in virtiofs.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L577
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Attr {
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub atimensec: u32,
    pub mtimensec: u32,
    pub ctimensec: u32,
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u32,
    pub blksize: u32,
    pub flags: u32,
}

/// InHeader represents the incoming message header in the filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L1155
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct InHeader {
    pub len: u32,
    pub opcode: u32,
    pub unique: u64,
    pub nodeid: u64,
    pub uid: u32,
    pub gid: u32,
    pub pid: u32,
    pub total_extlen: u16,
    pub padding: u16,
}

/// OutHeader represents the message header returned in the filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L1170
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct OutHeader {
    pub len: u32,
    pub error: i32,
    pub unique: u64,
}

/// InitIn is used to parse the parameters passed in the Init filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L1030
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct InitIn {
    pub major: u32,
    pub minor: u32,
    pub max_readahead: u32,
    pub flags: u32,
}

/// InitOut is used to return the result of the Init filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L1048
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct InitOut {
    pub major: u32,
    pub minor: u32,
    pub max_readahead: u32,
    pub flags: u32,
    pub max_background: u16,
    pub congestion_threshold: u16,
    pub max_write: u32,
    pub time_gran: u32,
    pub max_pages: u16,
    pub map_alignment: u16,
    pub flags2: u32,
    pub unused: [u32; 7],
}

/// EntryOut is used to return the file entry in the filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L737
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct EntryOut {
    pub nodeid: u64,
    pub generation: u64,
    pub entry_valid: u64,
    pub attr_valid: u64,
    pub entry_valid_nsec: u32,
    pub attr_valid_nsec: u32,
    pub attr: Attr,
}

/// AttrOut is used to return the file attributes in the filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L782
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct AttrOut {
    pub attr_valid: u64,
    pub attr_valid_nsec: u32,
    pub dummy: u32,
    pub attr: Attr,
}

/// CreateIn is used to parse the parameters passed in the Create filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L881
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct CreateIn {
    pub flags: u32,
    pub mode: u32,
    pub umask: u32,
    pub open_flags: u32,
}

/// OpenIn is used to parse the parameters passed in the Open filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#873
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenIn {
    pub flags: u32,
    pub open_flags: u32,
}

/// OpenOut is used to return the file descriptor in the filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L891
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenOut {
    pub fh: u64,
    pub open_flags: u32,
    pub padding: u32,
}

/// ReadIn is used to parse the parameters passed in the Read filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#920
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ReadIn {
    pub fh: u64,
    pub offset: u64,
    pub size: u32,
    pub read_flags: u32,
    pub lock_owner: u64,
    pub flags: u32,
    pub padding: u32,
}

/// WriteIn is used to parse the parameters passed in the Write filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#933
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WriteIn {
    pub fh: u64,
    pub offset: u64,
    pub size: u32,
    pub write_flags: u32,
    pub lock_owner: u64,
    pub flags: u32,
    pub padding: u32,
}

/// WriteOut is used to return the number of bytes written in the Write filesystem call.
///
/// The fields of the struct need to conform to the specific format of the virtiofs message.
/// Currently, we only need to align them exactly with virtiofsd.
/// Reference: https://gitlab.com/virtio-fs/virtiofsd/-/blob/main/src/fuse.rs?ref_type=heads#L946
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WriteOut {
    pub size: u32,
    pub padding: u32,
}

/// We will use ByteValued to implement the encoding and decoding
/// of these structures in shared memory.
unsafe impl ByteValued for Attr {}
unsafe impl ByteValued for InHeader {}
unsafe impl ByteValued for OutHeader {}
unsafe impl ByteValued for InitIn {}
unsafe impl ByteValued for InitOut {}
unsafe impl ByteValued for EntryOut {}
unsafe impl ByteValued for AttrOut {}
unsafe impl ByteValued for CreateIn {}
unsafe impl ByteValued for OpenIn {}
unsafe impl ByteValued for OpenOut {}
unsafe impl ByteValued for ReadIn {}
unsafe impl ByteValued for WriteIn {}
unsafe impl ByteValued for WriteOut {}
