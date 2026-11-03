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

//! Native payload buffers crossing the FFI boundary in both directions.
//!
//! Read payloads keep the [`opendal::Buffer`] behind a handle so the caller
//! copies (or views) chunks straight from native memory. Write buffers grow
//! segment by segment — segments never move, so handed-out pointers stay
//! valid — and a write consumes them, frozen into one
//! non-contiguous buffer without copying.

use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr;

use crate::chunk::OpendalChunk;
use crate::error::OpenDALError;
use crate::result::OpendalWriteResult;
use crate::utils::config_invalid_error;

#[repr(C)]
/// Read payload owned by this crate.
///
/// Holds the [`opendal::Buffer`] itself rather than a flattened copy, so the
/// caller can copy straight into its final destination. `handle` must be
/// released with `opendal_read_result_release` exactly once.
pub struct OpendalReadBuffer {
    /// Leaked `Box<opendal::Buffer>`, or null when there is no payload.
    pub handle: *mut c_void,
    /// Total readable bytes across every chunk.
    pub len: usize,
}

impl OpendalReadBuffer {
    /// Create an empty payload that does not own anything.
    pub fn empty() -> Self {
        Self {
            handle: ptr::null_mut(),
            len: 0,
        }
    }

    /// Take ownership of a buffer without flattening or copying it.
    pub fn from_buffer(value: opendal::Buffer) -> Self {
        let len = value.len();
        if len == 0 {
            return Self::empty();
        }

        Self {
            handle: Box::into_raw(Box::new(value)) as *mut c_void,
            len,
        }
    }
}

/// Copy the payload from `src_offset` onwards into `dest`, returning the
/// number of bytes written.
///
/// Copies at most `dest_len` bytes. An `opendal::Buffer` may be split across
/// several chunks, so this walks them and writes each one directly into the
/// destination; that is the only copy a read performs. An offset at or past
/// the end of the payload writes nothing.
/// # Safety
///
/// - `handle` must be null or come from `OpendalReadBuffer::from_buffer`.
/// - `dest` must be valid for writes of `dest_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_buffer_copy_to(
    handle: *const c_void,
    src_offset: usize,
    dest: *mut u8,
    dest_len: usize,
) -> usize {
    if handle.is_null() || dest.is_null() || dest_len == 0 {
        return 0;
    }

    let buffer = unsafe { &*(handle as *const opendal::Buffer) };
    if src_offset >= buffer.len() {
        return 0;
    }

    // Slicing bumps reference counts on the backing `Bytes` instead of
    // copying; iterating then consumes the sliced view, not the original.
    let chunks = buffer.slice(src_offset..);

    let mut written = 0usize;
    for chunk in chunks {
        if written == dest_len {
            break;
        }

        let take = chunk.len().min(dest_len - written);
        unsafe {
            ptr::copy_nonoverlapping(chunk.as_ptr(), dest.add(written), take);
        }
        written += take;
    }

    written
}

/// Enumerate the chunks of a read payload without copying.
///
/// Fills `out` with up to `cap` descriptors and returns the total number of
/// chunks; call with `cap` 0 first to size the array. Descriptor pointers
/// stay valid until the owning handle is released.
/// # Safety
///
/// - `handle` must be null or come from `OpendalReadBuffer::from_buffer`.
/// - When `cap > 0`, `out` must be valid for writes of `cap` descriptors.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_buffer_chunks(
    handle: *const c_void,
    out: *mut OpendalChunk,
    cap: usize,
) -> usize {
    if handle.is_null() {
        return 0;
    }

    let buffer = unsafe { &*(handle as *const opendal::Buffer) };

    // Iterated chunks are `Bytes` clones sharing storage with the handle's
    // buffer, so the recorded pointers outlive this call.
    let mut count = 0usize;
    for chunk in buffer.clone() {
        if count < cap && !out.is_null() {
            unsafe {
                *out.add(count) = OpendalChunk {
                    data: chunk.as_ptr(),
                    len: chunk.len(),
                };
            }
        }
        count += 1;
    }

    count
}

/// # Safety
///
/// - `handle` must be null or come from `OpendalReadBuffer::from_buffer`.
/// - This function must be called at most once for the same handle.
/// - Callers must not use `handle` after this function returns.
pub(crate) unsafe fn read_buffer_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    drop(unsafe { Box::from_raw(handle as *mut opendal::Buffer) });
}

#[repr(C)]
/// Writable segment handed to the caller.
///
/// `data` points at `capacity` writable bytes owned by `handle`. The handle
/// must be released exactly once with `write_buffer_free`, whether or not its
/// contents were consumed.
pub struct OpendalWriteBuffer {
    /// Leaked `Box<WriteBufferSlot>` owning every segment.
    pub handle: *mut c_void,
    /// Raw pointer to the newest segment, valid until the handle is freed.
    pub data: *mut u8,
    /// Writable bytes behind `data`.
    pub capacity: usize,
}

impl OpendalWriteBuffer {
    pub fn empty() -> Self {
        Self {
            handle: std::ptr::null_mut(),
            data: std::ptr::null_mut(),
            capacity: 0,
        }
    }
}

/// Sealed segments with their committed lengths, plus the segment currently
/// exposed to the caller.
pub(crate) struct FillingState {
    sealed: Vec<(Vec<MaybeUninit<u8>>, usize)>,
    current: Vec<MaybeUninit<u8>>,
}

/// Allocate a segment without initializing it, like a pooled array: contents
/// are unspecified until the caller writes them.
fn uninit_segment(capacity: usize) -> Vec<MaybeUninit<u8>> {
    let mut segment = Vec::with_capacity(capacity);
    // SAFETY: `MaybeUninit<u8>` requires no initialization.
    unsafe { segment.set_len(capacity) };
    segment
}

/// Freeze the committed prefix of a segment into `Bytes`.
///
/// The caller wrote the first `committed` bytes through the segment's raw
/// pointer, so that prefix is initialized. The tail past it is freed without
/// ever being read.
fn freeze_committed(segment: Vec<MaybeUninit<u8>>, committed: usize) -> bytes::Bytes {
    debug_assert!(committed <= segment.len());
    let mut segment = std::mem::ManuallyDrop::new(segment);
    let (ptr, capacity) = (segment.as_mut_ptr(), segment.capacity());
    // SAFETY: same allocation and layout (`MaybeUninit<u8>` mirrors `u8`),
    // length capped to the initialized prefix, capacity preserved for the
    // eventual free.
    let vec = unsafe { Vec::from_raw_parts(ptr.cast::<u8>(), committed, capacity) };
    bytes::Bytes::from(vec)
}

/// Lifecycle of an allocated write buffer.
///
/// Consumption keeps a guard clone of the assembled buffer until
/// `write_buffer_free`, so a stale caller-side view can never dangle.
pub(crate) enum WriteBufferSlot {
    Filling(FillingState),
    Consumed { _guard: opendal::Buffer },
}

/// Seal the current segment and expose a fresh one of `min_capacity` bytes.
fn grow(
    slot: &mut WriteBufferSlot,
    committed_in_current: usize,
    min_capacity: usize,
) -> Result<(*mut u8, usize), OpenDALError> {
    if min_capacity == 0 {
        return Err(config_invalid_error(
            "write buffer segment capacity must be greater than 0",
        ));
    }

    match slot {
        WriteBufferSlot::Filling(state) => {
            if committed_in_current > state.current.len() {
                return Err(config_invalid_error(
                    "committed length exceeds the current segment capacity",
                ));
            }

            let mut next = uninit_segment(min_capacity);
            let data = next.as_mut_ptr().cast::<u8>();
            let sealed = std::mem::replace(&mut state.current, next);
            state.sealed.push((sealed, committed_in_current));
            Ok((data, min_capacity))
        }
        WriteBufferSlot::Consumed { .. } => Err(config_invalid_error(
            "write buffer contents were already consumed",
        )),
    }
}

/// Assemble the committed bytes of every segment into one buffer.
///
/// On success the slot moves to `Consumed`; errors leave it untouched, so
/// the caller keeps full ownership.
pub(crate) fn take_payload(
    slot: &mut WriteBufferSlot,
    committed_in_current: usize,
) -> Result<opendal::Buffer, OpenDALError> {
    match slot {
        WriteBufferSlot::Filling(state) => {
            if committed_in_current > state.current.len() {
                return Err(config_invalid_error(
                    "committed length exceeds the current segment capacity",
                ));
            }

            let mut parts: Vec<bytes::Bytes> = Vec::with_capacity(state.sealed.len() + 1);
            for (segment, committed) in state.sealed.drain(..) {
                if committed == 0 {
                    continue;
                }

                parts.push(freeze_committed(segment, committed));
            }

            if committed_in_current > 0 {
                let current = std::mem::take(&mut state.current);
                parts.push(freeze_committed(current, committed_in_current));
            }

            let buffer = if parts.is_empty() {
                opendal::Buffer::new()
            } else {
                opendal::Buffer::from(parts)
            };

            *slot = WriteBufferSlot::Consumed {
                _guard: buffer.clone(),
            };
            Ok(buffer)
        }
        WriteBufferSlot::Consumed { .. } => Err(config_invalid_error(
            "write buffer contents were already consumed",
        )),
    }
}

/// Allocate a buffer exposing `capacity` writable bytes. The memory is not
/// initialized: contents are unspecified until the caller writes them.
///
/// The returned handle must eventually be released with `write_buffer_free`.
#[unsafe(no_mangle)]
pub extern "C" fn write_buffer_create(capacity: usize) -> OpendalWriteResult {
    if capacity == 0 {
        return OpendalWriteResult::from_error(config_invalid_error(
            "write buffer capacity must be greater than 0",
        ));
    }

    let mut current = uninit_segment(capacity);
    let data = current.as_mut_ptr().cast::<u8>();
    let slot = WriteBufferSlot::Filling(FillingState {
        sealed: Vec::new(),
        current,
    });
    let handle = Box::into_raw(Box::new(slot)) as *mut c_void;

    OpendalWriteResult::ok(OpendalWriteBuffer {
        handle,
        data,
        capacity,
    })
}

/// Seal the current segment at `committed_in_current` bytes and expose a new
/// segment of at least `min_capacity` bytes. Earlier segments keep their
/// addresses.
/// # Safety
///
/// - `handle` must come from `write_buffer_create` and not have been freed.
/// - Calls for the same handle must not run concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn write_buffer_add_segment(
    handle: *mut c_void,
    committed_in_current: usize,
    min_capacity: usize,
) -> OpendalWriteResult {
    if handle.is_null() {
        return OpendalWriteResult::from_error(config_invalid_error("write buffer handle is null"));
    }

    let slot = unsafe { &mut *(handle as *mut WriteBufferSlot) };
    match grow(slot, committed_in_current, min_capacity) {
        Ok((data, capacity)) => OpendalWriteResult::ok(OpendalWriteBuffer {
            handle,
            data,
            capacity,
        }),
        Err(error) => OpendalWriteResult::from_error(error),
    }
}

/// Release an allocated write buffer handle. After the contents are consumed this only
/// drops the guard clone; the backend keeps the payload alive for as long as it needs.
/// # Safety
///
/// - `handle` must be null or come from `write_buffer_create`.
/// - Must be called at most once for the same handle.
/// - Segment pointers must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn write_buffer_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    drop(unsafe { Box::from_raw(handle as *mut WriteBufferSlot) });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three parts, so the copy has to advance the destination across chunks.
    /// Backends that hand back a single contiguous buffer never exercise this.
    fn split_buffer() -> opendal::Buffer {
        opendal::Buffer::from(vec![
            bytes::Bytes::from_static(b"abc"),
            bytes::Bytes::from_static(b"de"),
            bytes::Bytes::from_static(b"fgh"),
        ])
    }

    fn copy_at(buffer: &opendal::Buffer, src_offset: usize, dest: &mut [u8]) -> usize {
        let owned = OpendalReadBuffer::from_buffer(buffer.clone());
        let written =
            unsafe { read_buffer_copy_to(owned.handle, src_offset, dest.as_mut_ptr(), dest.len()) };
        unsafe { read_buffer_free(owned.handle) };
        written
    }

    fn copy(buffer: &opendal::Buffer, dest: &mut [u8]) -> usize {
        copy_at(buffer, 0, dest)
    }

    #[test]
    fn copies_every_chunk_in_order() {
        let buffer = split_buffer();
        let mut dest = vec![0u8; buffer.len()];

        assert_eq!(copy(&buffer, &mut dest), 8);
        assert_eq!(&dest, b"abcdefgh");
    }

    #[test]
    fn stops_at_the_destination_length() {
        let mut dest = vec![0u8; 4];

        assert_eq!(copy(&split_buffer(), &mut dest), 4);
        assert_eq!(&dest, b"abcd");
    }

    #[test]
    fn honors_a_slice_that_starts_mid_chunk() {
        // Ranged reads produce exactly this: a non-contiguous buffer whose logical
        // start sits inside the second part.
        let buffer = split_buffer().slice(4..7);
        let mut dest = vec![0u8; buffer.len()];

        assert_eq!(copy(&buffer, &mut dest), 3);
        assert_eq!(&dest, b"efg");
    }

    #[test]
    fn resumes_from_an_offset_inside_a_chunk() {
        // Streaming reads drain one chunk across several calls, so the copy
        // must be able to start mid-chunk and keep walking the later parts.
        let buffer = split_buffer();
        let mut dest = vec![0u8; 4];

        assert_eq!(copy_at(&buffer, 4, &mut dest), 4);
        assert_eq!(&dest, b"efgh");
    }

    #[test]
    fn writes_nothing_at_or_past_the_end() {
        let buffer = split_buffer();
        let mut dest = vec![0u8; 4];

        assert_eq!(copy_at(&buffer, buffer.len(), &mut dest), 0);
        assert_eq!(copy_at(&buffer, buffer.len() + 1, &mut dest), 0);
        assert_eq!(&dest, &[0u8; 4]);
    }

    #[test]
    fn chunks_reports_every_part_and_respects_capacity() {
        let owned = OpendalReadBuffer::from_buffer(split_buffer());

        // Sizing call: no output array, just the count.
        let count = unsafe { read_buffer_chunks(owned.handle, std::ptr::null_mut(), 0) };
        assert_eq!(count, 3);

        let mut chunks = vec![
            OpendalChunk {
                data: std::ptr::null(),
                len: 0,
            },
            OpendalChunk {
                data: std::ptr::null(),
                len: 0,
            },
            OpendalChunk {
                data: std::ptr::null(),
                len: 0,
            },
        ];
        let filled = unsafe { read_buffer_chunks(owned.handle, chunks.as_mut_ptr(), chunks.len()) };
        assert_eq!(filled, 3);

        let collected: Vec<u8> = chunks
            .iter()
            .flat_map(|chunk| unsafe { std::slice::from_raw_parts(chunk.data, chunk.len) })
            .copied()
            .collect();
        assert_eq!(&collected, b"abcdefgh");

        unsafe { read_buffer_free(owned.handle) };
    }

    #[test]
    fn empty_payload_owns_nothing() {
        let owned = OpendalReadBuffer::from_buffer(opendal::Buffer::new());

        assert!(owned.handle.is_null());
        assert_eq!(owned.len, 0);
        unsafe { read_buffer_free(owned.handle) };
    }

    fn as_uninit(bytes: Vec<u8>) -> Vec<MaybeUninit<u8>> {
        bytes.into_iter().map(MaybeUninit::new).collect()
    }

    fn filling(segments: Vec<(Vec<u8>, usize)>, current: Vec<u8>) -> WriteBufferSlot {
        WriteBufferSlot::Filling(FillingState {
            sealed: segments
                .into_iter()
                .map(|(segment, committed)| (as_uninit(segment), committed))
                .collect(),
            current: as_uninit(current),
        })
    }

    #[test]
    fn take_assembles_committed_bytes_across_segments_in_order() {
        // Sealed segments carry their own committed lengths; the tail commit
        // arrives as an argument. Uncommitted capacity must never leak.
        let mut slot = filling(
            vec![(b"abcX".to_vec(), 3), (b"deXX".to_vec(), 2)],
            b"fghXX".to_vec(),
        );

        let payload = take_payload(&mut slot, 3).unwrap();

        assert_eq!(payload.to_vec(), b"abcdefgh");
        match &slot {
            WriteBufferSlot::Consumed { _guard } => assert_eq!(_guard.len(), 8),
            WriteBufferSlot::Filling(_) => panic!("slot must move to Consumed"),
        }
    }

    #[test]
    fn take_skips_segments_with_nothing_committed() {
        let mut slot = filling(vec![(b"XXXX".to_vec(), 0)], b"ab".to_vec());

        let payload = take_payload(&mut slot, 2).unwrap();

        assert_eq!(payload.to_vec(), b"ab");
    }

    #[test]
    fn take_rejects_a_second_take() {
        let mut slot = filling(Vec::new(), b"abc".to_vec());
        take_payload(&mut slot, 3).unwrap();

        assert!(take_payload(&mut slot, 0).is_err());
    }

    #[test]
    fn take_rejects_commits_beyond_capacity_and_keeps_the_slot_usable() {
        let mut slot = filling(Vec::new(), b"abc".to_vec());

        assert!(take_payload(&mut slot, 4).is_err());

        // The failed call must not consume the buffer.
        let payload = take_payload(&mut slot, 2).unwrap();
        assert_eq!(payload.to_vec(), b"ab");
    }

    #[test]
    fn grow_seals_the_current_segment_and_keeps_earlier_pointers_stable() {
        let mut slot = filling(Vec::new(), b"abc".to_vec());
        let first_ptr = match &slot {
            WriteBufferSlot::Filling(state) => state.current.as_ptr(),
            WriteBufferSlot::Consumed { .. } => unreachable!(),
        };

        let (data, capacity) = grow(&mut slot, 2, 8).unwrap();

        assert_eq!(capacity, 8);
        assert!(!data.is_null());
        match &slot {
            WriteBufferSlot::Filling(state) => {
                assert_eq!(state.sealed.len(), 1);
                assert_eq!(state.sealed[0].1, 2);
                // The sealed segment must not have moved when it changed hands.
                assert_eq!(state.sealed[0].0.as_ptr(), first_ptr);
            }
            WriteBufferSlot::Consumed { .. } => panic!("slot must stay Filling"),
        }
    }

    #[test]
    fn create_hands_out_writable_memory_and_free_releases_it() {
        let result = write_buffer_create(8);
        assert_eq!(result.error.has_error, 0);

        let buffer = result.buffer;
        assert_eq!(buffer.capacity, 8);
        unsafe {
            *buffer.data = 7;
            assert_eq!(*buffer.data, 7);
            write_buffer_free(buffer.handle);
        }
    }

    #[test]
    fn create_rejects_zero_capacity() {
        let mut result = write_buffer_create(0);
        assert_eq!(result.error.has_error, 1);
        assert!(result.buffer.handle.is_null());
        crate::result::opendal_error_release(std::mem::replace(
            &mut result.error,
            OpenDALError::ok(),
        ));
    }
}
