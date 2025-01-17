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

use std::cell::RefCell;
use std::cmp::min;
use std::ptr;

use vm_memory::bitmap::BitmapSlice;
use vm_memory::VolatileSlice;

/// ReadWriteAtVolatile is a trait that allows reading and writing from a slice of VolatileSlice.
pub trait ReadWriteAtVolatile<B: BitmapSlice> {
    fn read_vectored_at_volatile(&self, bufs: &[&VolatileSlice<B>]) -> usize;
    fn write_vectored_at_volatile(&self, bufs: &[&VolatileSlice<B>]) -> usize;
}

impl<B: BitmapSlice, T: ReadWriteAtVolatile<B> + ?Sized> ReadWriteAtVolatile<B> for &T {
    fn read_vectored_at_volatile(&self, bufs: &[&VolatileSlice<B>]) -> usize {
        (**self).read_vectored_at_volatile(bufs)
    }

    fn write_vectored_at_volatile(&self, bufs: &[&VolatileSlice<B>]) -> usize {
        (**self).write_vectored_at_volatile(bufs)
    }
}

/// BufferWrapper is a wrapper around opendal::Buffer that implements ReadWriteAtVolatile.
pub struct BufferWrapper {
    buffer: RefCell<opendal::Buffer>,
}

impl BufferWrapper {
    pub fn new(buffer: opendal::Buffer) -> BufferWrapper {
        BufferWrapper {
            buffer: RefCell::new(buffer),
        }
    }

    pub fn get_buffer(&self) -> opendal::Buffer {
        return self.buffer.borrow().clone();
    }
}

impl<B: BitmapSlice> ReadWriteAtVolatile<B> for BufferWrapper {
    fn read_vectored_at_volatile(&self, bufs: &[&VolatileSlice<B>]) -> usize {
        let slice_guards: Vec<_> = bufs.iter().map(|s| s.ptr_guard_mut()).collect();
        let iovecs: Vec<_> = slice_guards
            .iter()
            .map(|s| libc::iovec {
                iov_base: s.as_ptr() as *mut libc::c_void,
                iov_len: s.len() as libc::size_t,
            })
            .collect();
        if iovecs.is_empty() {
            return 0;
        }
        let data = self.buffer.borrow().to_vec();
        let mut result = 0;
        for (index, iovec) in iovecs.iter().enumerate() {
            let num = min(data.len() - result, iovec.iov_len);
            if num == 0 {
                break;
            }
            unsafe {
                ptr::copy_nonoverlapping(data[result..].as_ptr(), iovec.iov_base as *mut u8, num)
            }
            bufs[index].bitmap().mark_dirty(0, num);
            result += num;
        }
        result
    }

    fn write_vectored_at_volatile(&self, bufs: &[&VolatileSlice<B>]) -> usize {
        let slice_guards: Vec<_> = bufs.iter().map(|s| s.ptr_guard()).collect();
        let iovecs: Vec<_> = slice_guards
            .iter()
            .map(|s| libc::iovec {
                iov_base: s.as_ptr() as *mut libc::c_void,
                iov_len: s.len() as libc::size_t,
            })
            .collect();
        if iovecs.is_empty() {
            return 0;
        }
        let len = iovecs.iter().map(|iov| iov.iov_len).sum();
        let mut data = vec![0; len];
        let mut offset = 0;
        for iov in iovecs.iter() {
            unsafe {
                ptr::copy_nonoverlapping(
                    iov.iov_base as *const u8,
                    data.as_mut_ptr().add(offset),
                    iov.iov_len,
                );
            }
            offset += iov.iov_len;
        }
        *self.buffer.borrow_mut() = opendal::Buffer::from(data);
        len
    }
}
