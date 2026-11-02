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

use std::collections::VecDeque;
use std::convert::Infallible;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::io::BufRead;
use std::io::IoSlice;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::{self};
use std::iter::FusedIterator;
use std::mem;
use std::ops::Bound;
use std::ops::RangeBounds;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use bytes::Buf;
use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use futures::Stream;

use crate::*;

/// Buffer is a wrapper of contiguous `Bytes` and non-contiguous `[Bytes]`.
///
/// Buffer supports non-contiguous bytes that a storage service returns. For example,
/// HTTP-based services such as S3 can produce non-contiguous bytes while streaming.
///
/// ## Features
///
/// - [`Buffer`] can be used as [`Buf`], [`Iterator`], [`Stream`] directly.
/// - [`Buffer`] is cheap to clone like [`Bytes`], only update reference count, no allocation.
/// - Convert [`Buffer`] to [`IoSlice`] for vectored writes.
///
/// ## Examples
///
/// ### As `Buf`
///
/// `Buffer` implements `Buf` trait:
///
/// ```rust
/// use bytes::Buf;
/// use opendal_core::Buffer;
/// use serde_json;
///
/// fn test(mut buf: Buffer) -> Vec<String> {
///     serde_json::from_reader(buf.reader()).unwrap()
/// }
/// ```
///
/// ### As Bytes `Iterator`
///
/// `Buffer` implements `Iterator<Item=Bytes>` trait:
///
/// ```rust
/// use bytes::Bytes;
/// use opendal_core::Buffer;
///
/// fn test(mut buf: Buffer) -> Vec<Bytes> {
///     buf.into_iter().collect()
/// }
/// ```
///
/// ### As Bytes `Stream`
///
/// `Buffer` implements `Stream<Item=Result<Bytes, Infallible>>` trait:
///
/// ```rust
/// use bytes::Bytes;
/// use futures::TryStreamExt;
/// use opendal_core::Buffer;
///
/// async fn test(mut buf: Buffer) -> Vec<Bytes> {
///     buf.into_iter().try_collect().await.unwrap()
/// }
/// ```
///
/// ### As one contiguous Bytes
///
/// `Buffer` can make contiguous by transform into `Bytes` or `Vec<u8>`.
/// Please keep in mind that this operation involves new allocation and bytes copy, and we can't
/// reuse the same memory region anymore.
///
/// ```rust
/// use bytes::Bytes;
/// use opendal_core::Buffer;
///
/// fn test_to_vec(buf: Buffer) -> Vec<u8> {
///     buf.to_vec()
/// }
///
/// fn test_to_bytes(buf: Buffer) -> Bytes {
///     buf.to_bytes()
/// }
/// ```
#[derive(Clone)]
pub struct Buffer(Inner);

#[derive(Clone)]
enum Inner {
    Contiguous(Bytes),
    // the logic view of the buffer starts at `parts[idx][offset]` and spans `size` bytes
    // across subsequent `parts`
    NonContiguous {
        parts: Arc<[Bytes]>,
        size: usize,
        idx: usize,
        offset: usize,
    },
}

impl Debug for Buffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut b = f.debug_struct("Buffer");

        match &self.0 {
            Inner::Contiguous(bs) => {
                b.field("type", &"contiguous");
                b.field("size", &bs.len());
            }
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                b.field("type", &"non_contiguous");
                b.field("parts", &parts);
                b.field("size", &size);
                b.field("idx", &idx);
                b.field("offset", &offset);
            }
        }
        b.finish_non_exhaustive()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    /// Create a new empty buffer.
    ///
    /// This operation is const and no allocation will be performed.
    #[inline]
    pub const fn new() -> Self {
        Self(Inner::Contiguous(Bytes::new()))
    }

    /// Get the length of the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        match &self.0 {
            Inner::Contiguous(b) => b.remaining(),
            Inner::NonContiguous { size, .. } => *size,
        }
    }

    /// Check if buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of [`Bytes`] in [`Buffer`].
    ///
    /// For contiguous buffer, it's always 1. For non-contiguous buffer, it's number of bytes
    /// available for use.
    pub fn count(&self) -> usize {
        match &self.0 {
            Inner::Contiguous(_) => 1,
            Inner::NonContiguous {
                parts,
                idx,
                size,
                offset,
            } => {
                parts
                    .iter()
                    .skip(*idx)
                    .fold((0, size + offset), |(count, size), bytes| {
                        if size == 0 {
                            (count, 0)
                        } else {
                            (count + 1, size.saturating_sub(bytes.len()))
                        }
                    })
                    .0
            }
        }
    }

    /// Get current [`Bytes`].
    pub fn current(&self) -> Bytes {
        match &self.0 {
            Inner::Contiguous(inner) => inner.clone(),
            Inner::NonContiguous {
                parts,
                idx,
                offset,
                size,
            } => {
                let chunk = &parts[*idx];
                let n = (chunk.len() - *offset).min(*size);
                chunk.slice(*offset..*offset + n)
            }
        }
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the rest.
    ///
    /// If `len` is greater than the buffer’s current length, this has no effect.
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        match &mut self.0 {
            Inner::Contiguous(bs) => bs.truncate(len),
            Inner::NonContiguous { size, .. } => {
                *size = (*size).min(len);
            }
        }
    }

    /// Returns a slice of self for the provided range.
    ///
    /// This increments the reference count for the underlying memory and returns a new Buffer handle for the slice.
    ///
    /// This operation is O(1).
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        let len = self.len();

        let begin = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1).expect("out of range"),
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1).expect("out of range"),
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            begin <= end,
            "range start must not be greater than end: {begin:?} <= {end:?}",
        );
        assert!(end <= len, "range end out of bounds: {end:?} <= {len:?}",);

        if end == begin {
            return Buffer::new();
        }

        let mut ret = self.clone();
        ret.truncate(end);
        ret.advance(begin);
        ret
    }

    /// Combine all bytes together into one single [`Bytes`].
    ///
    /// This operation is zero copy if the underlying bytes are contiguous.
    /// Otherwise, it will copy all bytes into one single [`Bytes`].
    /// Please use API from [`Buf`], [`Iterator`] or [`Stream`] whenever possible.
    #[inline]
    pub fn to_bytes(&self) -> Bytes {
        match &self.0 {
            Inner::Contiguous(bytes) => bytes.clone(),
            Inner::NonContiguous {
                parts,
                size,
                idx: _,
                offset,
            } => {
                if parts.len() == 1 {
                    parts[0].slice(*offset..(*offset + *size))
                } else {
                    let mut ret = BytesMut::with_capacity(self.len());
                    ret.put(self.clone());
                    ret.freeze()
                }
            }
        }
    }

    /// Combine all bytes together into one single [`Vec<u8>`].
    ///
    /// This operation is not zero copy, it will copy all bytes into one single [`Vec<u8>`].
    /// Please use API from [`Buf`], [`Iterator`] or [`Stream`] whenever possible.
    #[inline]
    pub fn to_vec(&self) -> Vec<u8> {
        let mut ret = Vec::with_capacity(self.len());
        ret.put(self.clone());
        ret
    }

    /// Convert buffer into a slice of [`IoSlice`] for vectored write.
    #[inline]
    pub fn to_io_slice(&self) -> Vec<IoSlice<'_>> {
        match &self.0 {
            Inner::Contiguous(bs) => vec![IoSlice::new(bs.chunk())],
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                let mut ret = Vec::with_capacity(parts.len() - *idx);
                let mut new_offset = *offset;
                let mut remaining = *size;
                for part in parts.iter().skip(*idx) {
                    if remaining == 0 {
                        break;
                    }
                    let n = (part.len() - new_offset).min(remaining);
                    ret.push(IoSlice::new(&part[new_offset..new_offset + n]));
                    remaining -= n;
                    new_offset = 0;
                }
                ret
            }
        }
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Returns a new `Buffer` containing the bytes in the range `[0, at)`.
    /// Afterwards, `self` contains the bytes in the range `[at, len)`.
    ///
    /// This is an O(1) operation that just updates internal offsets and reference counts.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::Buffer;
    ///
    /// let mut buf = Buffer::from(vec![0u8, 1, 2, 3, 4, 5]);
    /// let head = buf.split_to(3);
    /// assert_eq!(head.to_vec(), vec![0, 1, 2]);
    /// assert_eq!(buf.to_vec(), vec![3, 4, 5]);
    /// ```
    pub fn split_to(&mut self, at: usize) -> Self {
        let len = self.len();
        assert!(at <= len, "split_to out of bounds: {at:?} <= {len:?}",);

        let head = self.slice(..at);
        self.advance(at);
        head
    }

    /// Splits the buffer into two at the given index.
    ///
    /// Returns a new `Buffer` containing the bytes in the range `[at, len)`.
    /// Afterwards, `self` contains the bytes in the range `[0, at)`.
    ///
    /// This is an O(1) operation that just updates internal offsets and reference counts.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::Buffer;
    ///
    /// let mut buf = Buffer::from(vec![0u8, 1, 2, 3, 4, 5]);
    /// let tail = buf.split_off(3);
    /// assert_eq!(buf.to_vec(), vec![0, 1, 2]);
    /// assert_eq!(tail.to_vec(), vec![3, 4, 5]);
    /// ```
    pub fn split_off(&mut self, at: usize) -> Self {
        let len = self.len();
        assert!(at <= len, "split_off out of bounds: {at:?} <= {len:?}",);

        let tail = self.slice(at..);
        self.truncate(at);
        tail
    }

    /// Split the buffer into an iterator of chunks, each with at most `chunk_size` bytes.
    ///
    /// The chunks share the same underlying storage with the original buffer. The last chunk
    /// will be shorter if `self.len()` is not a multiple of `chunk_size`.
    ///
    /// # Panics
    ///
    /// Panics if `chunk_size` is zero.
    pub fn chunks(&self, chunk_size: usize) -> BufferChunks {
        assert!(chunk_size != 0, "chunk size must be greater than 0");

        BufferChunks {
            buffer: self.clone(),
            chunk_size,
            position: 0,
            len: self.len(),
        }
    }
}

impl From<Vec<u8>> for Buffer {
    #[inline]
    fn from(bs: Vec<u8>) -> Self {
        Self(Inner::Contiguous(bs.into()))
    }
}

impl From<Bytes> for Buffer {
    #[inline]
    fn from(bs: Bytes) -> Self {
        Self(Inner::Contiguous(bs))
    }
}

impl From<String> for Buffer {
    #[inline]
    fn from(s: String) -> Self {
        Self(Inner::Contiguous(Bytes::from(s)))
    }
}

impl From<&'static [u8]> for Buffer {
    #[inline]
    fn from(s: &'static [u8]) -> Self {
        Self(Inner::Contiguous(Bytes::from_static(s)))
    }
}

impl From<&'static str> for Buffer {
    #[inline]
    fn from(s: &'static str) -> Self {
        Self(Inner::Contiguous(Bytes::from_static(s.as_bytes())))
    }
}

impl FromIterator<u8> for Buffer {
    #[inline]
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        Self(Inner::Contiguous(Bytes::from_iter(iter)))
    }
}

impl From<VecDeque<Bytes>> for Buffer {
    #[inline]
    fn from(bs: VecDeque<Bytes>) -> Self {
        let size = bs.iter().map(Bytes::len).sum();
        Self(Inner::NonContiguous {
            parts: Vec::from(bs).into(),
            size,
            idx: 0,
            offset: 0,
        })
    }
}

impl From<Vec<Bytes>> for Buffer {
    #[inline]
    fn from(bs: Vec<Bytes>) -> Self {
        let size = bs.iter().map(Bytes::len).sum();
        Self(Inner::NonContiguous {
            parts: bs.into(),
            size,
            idx: 0,
            offset: 0,
        })
    }
}

impl From<Arc<[Bytes]>> for Buffer {
    #[inline]
    fn from(bs: Arc<[Bytes]>) -> Self {
        let size = bs.iter().map(Bytes::len).sum();
        Self(Inner::NonContiguous {
            parts: bs,
            size,
            idx: 0,
            offset: 0,
        })
    }
}

impl FromIterator<Bytes> for Buffer {
    #[inline]
    fn from_iter<T: IntoIterator<Item = Bytes>>(iter: T) -> Self {
        let mut size = 0;
        let bs = iter.into_iter().inspect(|v| size += v.len());
        // This operation only needs one allocation from iterator to `Arc<[Bytes]>` instead
        // of iterator -> `Vec<Bytes>` -> `Arc<[Bytes]>`.
        let parts = Arc::from_iter(bs);
        Self(Inner::NonContiguous {
            parts,
            size,
            idx: 0,
            offset: 0,
        })
    }
}

impl Buf for Buffer {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        match &self.0 {
            Inner::Contiguous(b) => b.chunk(),
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                if *size == 0 {
                    return &[];
                }

                let chunk = &parts[*idx];
                let n = (chunk.len() - *offset).min(*size);
                &parts[*idx][*offset..*offset + n]
            }
        }
    }

    #[inline]
    fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        match &self.0 {
            Inner::Contiguous(b) => {
                if dst.is_empty() {
                    return 0;
                }

                dst[0] = IoSlice::new(b.chunk());
                1
            }
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                if dst.is_empty() {
                    return 0;
                }

                let mut new_offset = *offset;
                let mut remaining = *size;
                let mut count = 0;
                for (part, dst) in parts.iter().skip(*idx).zip(dst.iter_mut()) {
                    if remaining == 0 {
                        break;
                    }
                    let n = (part.len() - new_offset).min(remaining);
                    *dst = IoSlice::new(&part[new_offset..new_offset + n]);
                    remaining -= n;
                    new_offset = 0;
                    count += 1;
                }
                count
            }
        }
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        match &mut self.0 {
            Inner::Contiguous(b) => b.advance(cnt),
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                assert!(
                    cnt <= *size,
                    "cannot advance past {cnt} bytes, only {size} bytes left"
                );

                let mut new_idx = *idx;
                let mut new_offset = *offset;
                let mut remaining_cnt = cnt;
                while remaining_cnt > 0 {
                    let part_len = parts[new_idx].len();
                    let remaining_in_part = part_len - new_offset;

                    if remaining_cnt < remaining_in_part {
                        new_offset += remaining_cnt;
                        break;
                    }

                    remaining_cnt -= remaining_in_part;
                    new_idx += 1;
                    new_offset = 0;
                }

                *idx = new_idx;
                *offset = new_offset;
                *size -= cnt;
            }
        }
    }
}

impl Iterator for Buffer {
    type Item = Bytes;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            Inner::Contiguous(bs) => {
                if bs.is_empty() {
                    None
                } else {
                    Some(mem::take(bs))
                }
            }
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                if *size == 0 {
                    return None;
                }

                let chunk = &parts[*idx];
                let n = (chunk.len() - *offset).min(*size);
                let buf = chunk.slice(*offset..*offset + n);
                *size -= n;
                *offset += n;

                if *offset == chunk.len() {
                    *idx += 1;
                    *offset = 0;
                }

                Some(buf)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            Inner::Contiguous(bs) => {
                if bs.is_empty() {
                    (0, Some(0))
                } else {
                    (1, Some(1))
                }
            }
            Inner::NonContiguous { parts, idx, .. } => {
                let remaining = parts.len().saturating_sub(*idx);
                (remaining, Some(remaining))
            }
        }
    }
}

impl Stream for Buffer {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.get_mut().next().map(Ok))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        Iterator::size_hint(self)
    }
}

impl Read for Buffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let chunk = self.chunk();
        let len = chunk.len().min(buf.len());
        buf[..len].copy_from_slice(&chunk[..len]);
        self.advance(len);
        Ok(len)
    }
}

impl Seek for Buffer {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let len = self.len() as u64;
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                if offset < 0 {
                    len.checked_sub(offset.unsigned_abs())
                        .ok_or(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "invalid seek to a negative position",
                        ))?
                } else {
                    len.checked_add(offset as u64).ok_or(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek out of bounds",
                    ))?
                }
            }
            SeekFrom::Current(offset) => {
                let current_pos = (len - self.remaining() as u64) as i64;
                let new_pos = current_pos.checked_add(offset).ok_or(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "seek out of bounds",
                ))?;
                if new_pos < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid seek to a negative position",
                    ));
                }
                new_pos as u64
            }
        };

        if new_pos > len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek out of bounds",
            ));
        }

        self.advance((new_pos - (len - self.remaining() as u64)) as usize);
        Ok(new_pos)
    }
}

impl BufRead for Buffer {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let chunk = match &self.0 {
            Inner::Contiguous(b) => b.chunk(),
            Inner::NonContiguous {
                parts,
                size,
                idx,
                offset,
            } => {
                if *size == 0 {
                    return Ok(&[]);
                }

                let chunk = &parts[*idx];
                let n = (chunk.len() - *offset).min(*size);
                &parts[*idx][*offset..*offset + n]
            }
        };
        Ok(chunk)
    }

    fn consume(&mut self, amt: usize) {
        self.advance(amt);
    }
}

/// Iterator that yields [`Buffer`] chunks of at most a configured length.
pub struct BufferChunks {
    buffer: Buffer,
    chunk_size: usize,
    position: usize,
    len: usize,
}

impl Iterator for BufferChunks {
    type Item = Buffer;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.len {
            return None;
        }

        let end = (self.position + self.chunk_size).min(self.len);
        let chunk = self.buffer.slice(self.position..end);
        self.position = end;
        Some(chunk)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len.saturating_sub(self.position);
        let chunks = remaining.div_ceil(self.chunk_size);
        (chunks, Some(chunks))
    }
}

impl ExactSizeIterator for BufferChunks {
    fn len(&self) -> usize {
        self.size_hint().0
    }
}

impl FusedIterator for BufferChunks {}

/// `BufferCursor` is a [`Read`] + [`Seek`] cursor for `Buffer`, analogous to [`std::io::Cursor`].
///
/// `BufferCursor` supports seeking over potentially non-contiguous bytes in a Buffer. It is useful when
/// seekability is required, such as when interfacing with archiving and compression libraries, while
/// maintaining zero-copy semantics.
///
/// ## Notes
/// - [`BufferCursor`] does not implement [`std::io::Write`], because [`Buffer`] instances are immutable.
///
/// ## Features
/// - [`BufferCursor`] can be used as a [`Read`], [`BufRead`], and [`Seek`] directly.
/// - [`BufferCursor`] is cheap to read and seek, as it only updates the [`Buffer`] reference count
///   without additional allocations.
///
/// # Examples
///
/// ```rust
/// use bytes::Bytes;
/// use opendal_core::{Buffer, BufferCursor};
/// use std::io::{Read, Seek, SeekFrom};
///
/// fn test() -> std::io::Result<()> {
///     let mut cur = BufferCursor::new(Buffer::from(vec![
///         Bytes::from("abc"),
///         Bytes::from("def"),
///     ]));
///
///     cur.seek(SeekFrom::Start(3))?;
///
///     let mut out = vec![];
///     cur.read_to_end(&mut out); // def
///
///     cur.seek(SeekFrom::Start(0))?;
///
///     let mut out = vec![];
///     cur.read_to_end(&mut out); // abcdef
///
///     Ok(())
/// }
/// ```
///
pub struct BufferCursor {
    data: Buffer,
    view: Buffer,
    pos: u64,
}

impl BufferCursor {
    /// Create a BufferCursor for a Buffer.
    #[inline]
    pub fn new(buffer: Buffer) -> Self {
        Self {
            view: buffer.slice(..),
            data: buffer,
            pos: 0,
        }
    }
}

impl Read for BufferCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.view.read(buf)?;
        self.seek(SeekFrom::Current(size as i64))?;
        Ok(size)
    }
}

impl BufRead for BufferCursor {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.view.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.pos = self
            .pos
            .saturating_add(amount as u64)
            .min(self.data.len() as u64);
    }
}

impl Seek for BufferCursor {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let end = self.data.len() as u64;
        match pos {
            SeekFrom::Start(pos) => {
                self.pos = pos.min(end);
            }
            SeekFrom::Current(pos) => {
                if pos.is_negative() {
                    self.pos = self.pos.saturating_sub(pos.unsigned_abs());
                } else {
                    self.pos = self.pos.saturating_add(pos as u64).min(end);
                }
            }
            SeekFrom::End(pos) => {
                if pos.is_negative() {
                    self.pos = end.saturating_sub(pos.unsigned_abs());
                } else {
                    self.pos = end.saturating_add(pos as u64).min(end);
                }
            }
        }
        self.view = self.data.slice(self.pos as usize..);
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;

    use pretty_assertions::assert_eq;
    use rand::prelude::*;
    use rand::rng;

    use super::*;

    const EMPTY_SLICE: &[u8] = &[];

    #[test]
    fn test_contiguous_buffer() {
        let mut buf = Buffer::new();

        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);
        assert_eq!(buf.next(), None);
    }

    #[test]
    fn test_empty_non_contiguous_buffer() {
        let mut buf = Buffer::from(vec![Bytes::new()]);

        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);
        assert_eq!(buf.next(), None);
    }

    #[test]
    fn test_non_contiguous_buffer_with_empty_chunks() {
        let mut buf = Buffer::from(vec![Bytes::from("a")]);

        assert_eq!(buf.remaining(), 1);
        assert_eq!(buf.chunk(), b"a");

        buf.advance(1);

        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);
    }

    #[test]
    fn test_non_contiguous_buffer_with_next() {
        let mut buf = Buffer::from(vec![Bytes::from("a")]);

        assert_eq!(buf.remaining(), 1);
        assert_eq!(buf.chunk(), b"a");

        let bs = buf.next();

        assert_eq!(bs, Some(Bytes::from("a")));
        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);
    }

    #[test]
    fn test_buffer_advance() {
        let mut buf = Buffer::from(vec![Bytes::from("a"), Bytes::from("b"), Bytes::from("c")]);

        assert_eq!(buf.remaining(), 3);
        assert_eq!(buf.chunk(), b"a");

        buf.advance(1);

        assert_eq!(buf.remaining(), 2);
        assert_eq!(buf.chunk(), b"b");

        buf.advance(1);

        assert_eq!(buf.remaining(), 1);
        assert_eq!(buf.chunk(), b"c");

        buf.advance(1);

        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);

        buf.advance(0);

        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);
    }

    #[test]
    fn test_buffer_truncate() {
        let mut buf = Buffer::from(vec![Bytes::from("a"), Bytes::from("b"), Bytes::from("c")]);

        assert_eq!(buf.remaining(), 3);
        assert_eq!(buf.chunk(), b"a");

        buf.truncate(100);

        assert_eq!(buf.remaining(), 3);
        assert_eq!(buf.chunk(), b"a");

        buf.truncate(2);

        assert_eq!(buf.remaining(), 2);
        assert_eq!(buf.chunk(), b"a");

        buf.truncate(0);

        assert_eq!(buf.remaining(), 0);
        assert_eq!(buf.chunk(), EMPTY_SLICE);
    }

    #[test]
    fn test_buffer_chunks_contiguous() {
        let buf = Buffer::from(Bytes::from("abcdefg"));

        let chunks = buf
            .chunks(3)
            .map(|chunk| chunk.to_bytes())
            .collect::<Vec<Bytes>>();

        assert_eq!(
            chunks,
            vec![Bytes::from("abc"), Bytes::from("def"), Bytes::from("g")]
        );

        assert_eq!(Buffer::new().chunks(4).count(), 0);
    }

    #[test]
    fn test_buffer_chunks_non_contiguous() {
        let buf = Buffer::from(vec![
            Bytes::from("ab"),
            Bytes::from("c"),
            Bytes::from("def"),
        ]);

        let chunks = buf
            .chunks(2)
            .map(|chunk| chunk.to_bytes())
            .collect::<Vec<Bytes>>();

        assert_eq!(
            chunks,
            vec![Bytes::from("ab"), Bytes::from("cd"), Bytes::from("ef"),]
        );
    }

    #[test]
    #[should_panic(expected = "chunk size must be greater than 0")]
    fn test_buffer_chunks_zero_panics() {
        let buf = Buffer::from(Bytes::from("abc"));
        let _ = buf.chunks(0);
    }

    /// This setup will return
    ///
    /// - A buffer
    /// - Total size of this buffer.
    /// - Total content of this buffer.
    fn setup_buffer() -> (Buffer, usize, Bytes) {
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

        (buf, total_size, total_content)
    }

    #[test]
    fn fuzz_buffer_advance() {
        let mut rng = rng();

        let (mut buf, total_size, total_content) = setup_buffer();
        assert_eq!(buf.remaining(), total_size);
        assert_eq!(buf.to_bytes(), total_content);

        let mut cur = 0;
        // Loop at most 10000 times.
        let mut times = 10000;
        while !buf.is_empty() && times > 0 {
            times -= 1;

            let cnt = rng.random_range(0..total_size - cur);
            cur += cnt;
            buf.advance(cnt);

            assert_eq!(buf.remaining(), total_size - cur);
            assert_eq!(buf.to_bytes(), total_content.slice(cur..));
        }
    }

    #[test]
    fn fuzz_buffer_iter() {
        let mut rng = rng();

        let (mut buf, total_size, total_content) = setup_buffer();
        assert_eq!(buf.remaining(), total_size);
        assert_eq!(buf.to_bytes(), total_content);

        let mut cur = 0;
        while buf.is_empty() {
            let cnt = rng.random_range(0..total_size - cur);
            cur += cnt;
            buf.advance(cnt);

            // Before next
            assert_eq!(buf.remaining(), total_size - cur);
            assert_eq!(buf.to_bytes(), total_content.slice(cur..));

            if let Some(bs) = buf.next() {
                assert_eq!(bs, total_content.slice(cur..cur + bs.len()));
                cur += bs.len();
            }

            // After next
            assert_eq!(buf.remaining(), total_size - cur);
            assert_eq!(buf.to_bytes(), total_content.slice(cur..));
        }
    }

    #[test]
    fn fuzz_buffer_truncate() {
        let mut rng = rng();

        let (mut buf, total_size, total_content) = setup_buffer();
        assert_eq!(buf.remaining(), total_size);
        assert_eq!(buf.to_bytes(), total_content);

        let mut cur = 0;
        while buf.is_empty() {
            let cnt = rng.random_range(0..total_size - cur);
            cur += cnt;
            buf.advance(cnt);

            // Before truncate
            assert_eq!(buf.remaining(), total_size - cur);
            assert_eq!(buf.to_bytes(), total_content.slice(cur..));

            let truncate_size = rng.random_range(0..total_size - cur);
            buf.truncate(truncate_size);

            // After truncate
            assert_eq!(buf.remaining(), truncate_size);
            assert_eq!(
                buf.to_bytes(),
                total_content.slice(cur..cur + truncate_size)
            );

            // Try next after truncate
            if let Some(bs) = buf.next() {
                assert_eq!(bs, total_content.slice(cur..cur + bs.len()));
                cur += bs.len();
            }

            // After next
            assert_eq!(buf.remaining(), total_size - cur);
            assert_eq!(buf.to_bytes(), total_content.slice(cur..));
        }
    }

    #[test]
    fn test_read_trait() {
        let mut buffer = Buffer::from(vec![Bytes::from("Hello"), Bytes::from("World")]);
        let mut output = vec![0; 5];
        let size = buffer.read(&mut output).unwrap();
        assert_eq!(size, 5);
        assert_eq!(&output, b"Hello");
    }

    #[test]
    fn test_seek_trait() {
        let mut buffer = Buffer::from(vec![Bytes::from("Hello"), Bytes::from("World")]);
        buffer.seek(SeekFrom::Start(5)).unwrap();
        let mut output = vec![0; 5];
        buffer.read_exact(&mut output).unwrap();
        assert_eq!(&output, b"World");
    }

    #[test]
    fn test_bufread_trait() {
        let mut buffer = Buffer::from(vec![Bytes::from("Hello"), Bytes::from("World")]);
        let mut output = String::new();
        buffer.read_to_string(&mut output).unwrap();
        assert_eq!(output, "HelloWorld");

        let mut buffer = Buffer::from(vec![Bytes::from("Hello"), Bytes::from("World")]);
        let buf = buffer.fill_buf().unwrap();
        assert_eq!(buf, b"Hello");
        buffer.consume(5);
        let buf = buffer.fill_buf().unwrap();
        assert_eq!(buf, b"World");
    }

    #[test]
    fn test_read_partial() {
        let mut buffer = Buffer::from(vec![Bytes::from("Partial"), Bytes::from("Read")]);
        let mut output = vec![0; 4];
        let size = buffer.read(&mut output).unwrap();
        assert_eq!(size, 4);
        assert_eq!(&output, b"Part");

        let size = buffer.read(&mut output).unwrap();
        assert_eq!(size, 3);
        assert_eq!(&output[..3], b"ial");
    }

    #[test]
    fn test_seek_and_read() {
        let mut buffer = Buffer::from(vec![Bytes::from("SeekAndRead")]);
        buffer.seek(SeekFrom::Start(4)).unwrap();
        let mut output = vec![0; 3];
        buffer.read_exact(&mut output).unwrap();
        assert_eq!(&output, b"And");
    }

    #[test]
    fn test_bufread_consume() {
        let mut buffer = Buffer::from(vec![Bytes::from("ConsumeTest")]);
        let buf = buffer.fill_buf().unwrap();
        assert_eq!(buf, b"ConsumeTest");
        buffer.consume(7);
        let buf = buffer.fill_buf().unwrap();
        assert_eq!(buf, b"Test");
    }

    #[test]
    fn test_empty_buffer() {
        let mut buffer = Buffer::new();
        let mut output = vec![0; 5];
        let size = buffer.read(&mut output).unwrap();
        assert_eq!(size, 0);
        assert_eq!(&output, &[0; 5]);
    }

    #[test]
    fn test_seek_out_of_bounds() {
        let mut buffer = Buffer::from(vec![Bytes::from("OutOfBounds")]);
        let result = buffer.seek(SeekFrom::Start(100));
        assert!(result.is_err());
    }

    #[test]
    fn test_split_to_contiguous() {
        let mut buf = Buffer::from(Bytes::from("HelloWorld"));
        let head = buf.split_to(5);
        assert_eq!(head.to_bytes(), Bytes::from("Hello"));
        assert_eq!(buf.to_bytes(), Bytes::from("World"));
    }

    #[test]
    fn test_split_to_non_contiguous() {
        let mut buf = Buffer::from(vec![
            Bytes::from("ab"),
            Bytes::from("cd"),
            Bytes::from("ef"),
        ]);
        let head = buf.split_to(3);
        assert_eq!(head.to_bytes(), Bytes::from("abc"));
        assert_eq!(buf.to_bytes(), Bytes::from("def"));
    }

    #[test]
    fn test_split_to_at_zero() {
        let mut buf = Buffer::from(Bytes::from("Hello"));
        let head = buf.split_to(0);
        assert!(head.is_empty());
        assert_eq!(buf.to_bytes(), Bytes::from("Hello"));
    }

    #[test]
    fn test_split_to_at_len() {
        let mut buf = Buffer::from(Bytes::from("Hello"));
        let head = buf.split_to(5);
        assert_eq!(head.to_bytes(), Bytes::from("Hello"));
        assert!(buf.is_empty());
    }

    #[test]
    #[should_panic(expected = "split_to out of bounds")]
    fn test_split_to_out_of_bounds() {
        let mut buf = Buffer::from(Bytes::from("Hello"));
        buf.split_to(10);
    }

    #[test]
    fn test_split_off_contiguous() {
        let mut buf = Buffer::from(Bytes::from("HelloWorld"));
        let tail = buf.split_off(5);
        assert_eq!(buf.to_bytes(), Bytes::from("Hello"));
        assert_eq!(tail.to_bytes(), Bytes::from("World"));
    }

    #[test]
    fn test_split_off_non_contiguous() {
        let mut buf = Buffer::from(vec![
            Bytes::from("ab"),
            Bytes::from("cd"),
            Bytes::from("ef"),
        ]);
        let tail = buf.split_off(3);
        assert_eq!(buf.to_bytes(), Bytes::from("abc"));
        assert_eq!(tail.to_bytes(), Bytes::from("def"));
    }

    #[test]
    fn test_split_off_at_zero() {
        let mut buf = Buffer::from(Bytes::from("Hello"));
        let tail = buf.split_off(0);
        assert!(buf.is_empty());
        assert_eq!(tail.to_bytes(), Bytes::from("Hello"));
    }

    #[test]
    fn test_split_off_at_len() {
        let mut buf = Buffer::from(Bytes::from("Hello"));
        let tail = buf.split_off(5);
        assert_eq!(buf.to_bytes(), Bytes::from("Hello"));
        assert!(tail.is_empty());
    }

    #[test]
    #[should_panic(expected = "split_off out of bounds")]
    fn test_split_off_out_of_bounds() {
        let mut buf = Buffer::from(Bytes::from("Hello"));
        buf.split_off(10);
    }

    #[test]
    fn test_split_to_split_off_composition() {
        // Verify split_to and split_off can be composed correctly
        let mut buf = Buffer::from(Bytes::from("0123456789"));

        // Split off tail
        let tail = buf.split_off(7); // buf=[0-6], tail=[7-9]
        assert_eq!(buf.to_bytes(), Bytes::from("0123456"));
        assert_eq!(tail.to_bytes(), Bytes::from("789"));

        // Split head from remaining
        let head = buf.split_to(3); // head=[0-2], buf=[3-6]
        assert_eq!(head.to_bytes(), Bytes::from("012"));
        assert_eq!(buf.to_bytes(), Bytes::from("3456"));
    }

    #[test]
    fn test_split_preserves_underlying_storage() {
        // Verify that split operations share underlying storage
        let original = Bytes::from("HelloWorld");
        let mut buf = Buffer::from(original.clone());

        let head = buf.split_to(5);

        // Both should reference the same underlying storage
        // (This is implicit in Bytes behavior, but good to document)
        assert_eq!(head.to_bytes(), Bytes::from("Hello"));
        assert_eq!(buf.to_bytes(), Bytes::from("World"));
    }

    #[test]
    fn fuzz_buffer_split_to() {
        let mut rng = rng();

        let (mut buf, total_size, total_content) = setup_buffer();
        assert_eq!(buf.remaining(), total_size);

        let mut cur = 0;
        let mut times = 100;
        while !buf.is_empty() && times > 0 {
            times -= 1;
            let remaining = buf.len();
            let at = rng.random_range(0..=remaining);
            let head = buf.split_to(at);
            assert_eq!(head.to_bytes(), total_content.slice(cur..cur + at));
            cur += at;
            assert_eq!(buf.to_bytes(), total_content.slice(cur..total_size));
        }
    }

    #[test]
    fn fuzz_buffer_split_off() {
        let mut rng = rng();

        let (mut buf, total_size, total_content) = setup_buffer();
        assert_eq!(buf.remaining(), total_size);

        let mut end = total_size;
        let mut times = 100;
        let cur = 0;
        while !buf.is_empty() && times > 0 {
            times -= 1;
            let remaining = buf.len();
            let at = rng.random_range(0..=remaining);
            let tail = buf.split_off(at);
            assert_eq!(buf.to_bytes(), total_content.slice(cur..cur + at));
            assert_eq!(tail.to_bytes(), total_content.slice(cur + at..end));
            end = cur + at;
            // Continue with the head part
        }
    }

    /// Returns the bytes exposed by `to_io_slice` and by `Buf::chunks_vectored`.
    fn vectored_bytes(buf: &Buffer) -> (Bytes, Bytes) {
        let from_io_slice = buf
            .to_io_slice()
            .iter()
            .flat_map(|s| s.iter())
            .copied()
            .collect::<Bytes>();

        let mut dst = [IoSlice::new(EMPTY_SLICE); 8];
        let n = buf.chunks_vectored(&mut dst);
        let from_chunks_vectored = dst[..n]
            .iter()
            .flat_map(|s| s.iter())
            .copied()
            .collect::<Bytes>();

        (from_io_slice, from_chunks_vectored)
    }

    #[test]
    fn test_vectored_views_after_slice() {
        let buf = Buffer::from(vec![Bytes::from("abc"), Bytes::from("def")]);

        let view = buf.slice(0..2);
        assert_eq!(view.remaining(), 2);
        assert_eq!(view.to_bytes(), Bytes::from("ab"));
        assert_eq!(
            vectored_bytes(&view),
            (Bytes::from("ab"), Bytes::from("ab"))
        );

        let view = buf.slice(1..4);
        assert_eq!(view.remaining(), 3);
        assert_eq!(view.to_bytes(), Bytes::from("bcd"));
        assert_eq!(
            vectored_bytes(&view),
            (Bytes::from("bcd"), Bytes::from("bcd"))
        );
    }

    #[test]
    fn test_vectored_views_after_split_to() {
        let mut buf = Buffer::from(vec![Bytes::from("abc"), Bytes::from("def")]);
        let head = buf.split_to(2);

        assert_eq!(head.remaining(), 2);
        assert_eq!(
            vectored_bytes(&head),
            (Bytes::from("ab"), Bytes::from("ab"))
        );

        assert_eq!(buf.remaining(), 4);
        assert_eq!(
            vectored_bytes(&buf),
            (Bytes::from("cdef"), Bytes::from("cdef"))
        );
    }

    #[test]
    fn test_vectored_views_after_split_off() {
        let mut buf = Buffer::from(vec![Bytes::from("abc"), Bytes::from("def")]);
        let tail = buf.split_off(2);

        assert_eq!(buf.remaining(), 2);
        assert_eq!(vectored_bytes(&buf), (Bytes::from("ab"), Bytes::from("ab")));

        assert_eq!(tail.remaining(), 4);
        assert_eq!(
            vectored_bytes(&tail),
            (Bytes::from("cdef"), Bytes::from("cdef"))
        );
    }

    #[test]
    fn fuzz_buffer_cursor_seek_and_read() -> io::Result<()> {
        let (buf, total_size, total_content) = setup_buffer();
        let mut buf = BufferCursor::new(buf);

        let mut rng = rng();
        for _ in 0..100 {
            let pos = match rng.random_range(0..3) {
                0 => buf.seek(SeekFrom::Start(rng.random_range(0..=total_size as u64)))?,
                1 => buf.seek(SeekFrom::Current(rng.random_range(
                    -(buf.pos as i64)..=(total_size as i64 - buf.pos as i64),
                )))?,
                _ => buf.seek(SeekFrom::End(rng.random_range(-(total_size as i64)..=0)))?,
            };

            let mut out = vec![];
            buf.read_to_end(&mut out)?;

            assert_eq!(&out[..], &total_content[pos as usize..]);
        }

        Ok(())
    }
}
