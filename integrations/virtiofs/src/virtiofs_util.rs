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

use std::cmp::min;
use std::collections::VecDeque;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::mem::size_of;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr::copy_nonoverlapping;

use virtio_queue::DescriptorChain;
use vm_memory::bitmap::Bitmap;
use vm_memory::bitmap::BitmapSlice;
use vm_memory::Address;
use vm_memory::ByteValued;
use vm_memory::GuestMemory;
use vm_memory::GuestMemoryMmap;
use vm_memory::GuestMemoryRegion;
use vm_memory::VolatileMemory;
use vm_memory::VolatileSlice;

use crate::buffer::ReadWriteAtVolatile;
use crate::error::*;

/// Used to consume and use data areas in shared memory between host and VMs.
struct DescriptorChainConsumer<'a, B> {
    buffers: VecDeque<VolatileSlice<'a, B>>,
    bytes_consumed: usize,
}

impl<'a, B: BitmapSlice> DescriptorChainConsumer<'a, B> {
    fn consume<F>(&mut self, count: usize, f: F) -> usize
    where
        F: FnOnce(&[&VolatileSlice<B>]) -> usize,
    {
        let mut len = 0;
        let mut bufs = Vec::with_capacity(self.buffers.len());
        for vs in &self.buffers {
            if len >= count {
                break;
            }
            bufs.push(vs);
            let remain = count - len;
            if remain < vs.len() {
                len += remain;
            } else {
                len += vs.len();
            }
        }
        if bufs.is_empty() {
            return 0;
        }
        let bytes_consumed = f(&bufs);
        let mut remain = bytes_consumed;
        while let Some(vs) = self.buffers.pop_front() {
            if remain < vs.len() {
                self.buffers.push_front(vs.offset(remain).unwrap());
                break;
            }
            remain -= vs.len();
        }
        self.bytes_consumed += bytes_consumed;
        bytes_consumed
    }

    fn split_at(&mut self, offset: usize) -> DescriptorChainConsumer<'a, B> {
        let mut remain = offset;
        let pos = self.buffers.iter().position(|vs| {
            if remain < vs.len() {
                true
            } else {
                remain -= vs.len();
                false
            }
        });
        if let Some(at) = pos {
            let mut other = self.buffers.split_off(at);
            if remain > 0 {
                let front = other.pop_front().unwrap();
                self.buffers.push_back(front.subslice(0, remain).unwrap());
                other.push_front(front.offset(remain).unwrap());
            }
            DescriptorChainConsumer {
                buffers: other,
                bytes_consumed: 0,
            }
        } else {
            DescriptorChainConsumer {
                buffers: VecDeque::new(),
                bytes_consumed: 0,
            }
        }
    }

    fn bytes_consumed(&self) -> usize {
        self.bytes_consumed
    }

    #[cfg(test)]
    fn available_bytes(&self) -> usize {
        self.buffers.iter().fold(0, |count, vs| count + vs.len())
    }
}

/// Provides a high-level interface for reading data in shared memory sequences.
pub struct Reader<'a, B = ()> {
    buffer: DescriptorChainConsumer<'a, B>,
}

impl<'a, B: Bitmap + BitmapSlice + 'static> Reader<'a, B> {
    pub fn new<M>(
        mem: &'a GuestMemoryMmap<B>,
        desc_chain: DescriptorChain<M>,
    ) -> Result<Reader<'a, B>>
    where
        M: Deref,
        M::Target: GuestMemory + Sized,
    {
        let buffers = desc_chain
            .readable()
            .map(|desc| {
                let region = mem
                    .find_region(desc.addr())
                    .ok_or(new_unexpected_error("cannot get volatile memory", None))?;
                let offset = desc.addr().raw_value() - region.start_addr().raw_value();
                region
                    .deref()
                    .get_slice(offset as usize, desc.len() as usize)
                    .map_err(|err| {
                        new_unexpected_error("cannot get volatile memory", Some(err.into()))
                    })
            })
            .collect::<Result<VecDeque<VolatileSlice<'a, B>>>>()?;
        Ok(Reader {
            buffer: DescriptorChainConsumer {
                buffers,
                bytes_consumed: 0,
            },
        })
    }

    pub fn read_obj<T: ByteValued>(&mut self) -> Result<T> {
        let mut obj = MaybeUninit::<T>::uninit();
        let buf =
            unsafe { std::slice::from_raw_parts_mut(obj.as_mut_ptr() as *mut u8, size_of::<T>()) };
        self.read_exact(buf)
            .map_err(|e| new_unexpected_error("failed to read object", Some(e.into())))?;
        Ok(unsafe { obj.assume_init() })
    }

    pub fn read_to_at<F: ReadWriteAtVolatile<B>>(&mut self, dst: F, count: usize) -> usize {
        self.buffer
            .consume(count, |bufs| dst.write_vectored_at_volatile(bufs))
    }

    #[cfg(test)]
    pub fn available_bytes(&self) -> usize {
        self.buffer.available_bytes()
    }

    #[cfg(test)]
    pub fn bytes_read(&self) -> usize {
        self.buffer.bytes_consumed()
    }
}

impl<B: BitmapSlice> io::Read for Reader<'_, B> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let total = self.buffer.consume(buf.len(), |bufs| {
            let mut rem = buf;
            let mut total = 0;
            for vs in bufs {
                let copy_len = min(rem.len(), vs.len());
                unsafe {
                    copy_nonoverlapping(vs.ptr_guard().as_ptr(), rem.as_mut_ptr(), copy_len);
                }
                rem = &mut rem[copy_len..];
                total += copy_len;
            }
            total
        });
        Ok(total)
    }
}

/// Provides a high-level interface for writing data in shared memory sequences.
pub struct Writer<'a, B = ()> {
    buffer: DescriptorChainConsumer<'a, B>,
}

impl<'a, B: Bitmap + BitmapSlice + 'static> Writer<'a, B> {
    pub fn new<M>(
        mem: &'a GuestMemoryMmap<B>,
        desc_chain: DescriptorChain<M>,
    ) -> Result<Writer<'a, B>>
    where
        M: Deref,
        M::Target: GuestMemory + Sized,
    {
        let buffers = desc_chain
            .writable()
            .map(|desc| {
                let region = mem
                    .find_region(desc.addr())
                    .ok_or(new_unexpected_error("cannot get volatile memory", None))?;
                let offset = desc.addr().raw_value() - region.start_addr().raw_value();
                region
                    .deref()
                    .get_slice(offset as usize, desc.len() as usize)
                    .map_err(|err| {
                        new_unexpected_error("cannot get volatile memory", Some(err.into()))
                    })
            })
            .collect::<Result<VecDeque<VolatileSlice<'a, B>>>>()?;
        Ok(Writer {
            buffer: DescriptorChainConsumer {
                buffers,
                bytes_consumed: 0,
            },
        })
    }

    pub fn split_at(&mut self, offset: usize) -> Writer<'a, B> {
        Writer {
            buffer: self.buffer.split_at(offset),
        }
    }

    pub fn write_from_at<F: ReadWriteAtVolatile<B>>(&mut self, src: F, count: usize) -> usize {
        self.buffer
            .consume(count, |bufs| src.read_vectored_at_volatile(bufs))
    }

    pub fn bytes_written(&self) -> usize {
        self.buffer.bytes_consumed()
    }

    #[cfg(test)]
    pub fn available_bytes(&self) -> usize {
        self.buffer.available_bytes()
    }
}

impl<B: BitmapSlice> Write for Writer<'_, B> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let total = self.buffer.consume(buf.len(), |bufs| {
            let mut rem = buf;
            let mut total = 0;
            for vs in bufs {
                let copy_len = min(rem.len(), vs.len());
                unsafe {
                    copy_nonoverlapping(rem.as_ptr(), vs.ptr_guard_mut().as_ptr(), copy_len);
                }
                vs.bitmap().mark_dirty(0, copy_len);
                rem = &rem[copy_len..];
                total += copy_len;
            }
            total
        });
        Ok(total)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use virtio_queue::Queue;
    use virtio_queue::QueueOwnedT;
    use virtio_queue::QueueT;
    use vm_memory::Bytes;
    use vm_memory::GuestAddress;
    use vm_memory::Le16;
    use vm_memory::Le32;
    use vm_memory::Le64;

    const VIRTQ_DESC_F_NEXT: u16 = 0x1;
    const VIRTQ_DESC_F_WRITE: u16 = 0x2;

    enum DescriptorType {
        Readable,
        Writable,
    }

    // Helper structure for testing, used to define the layout of the descriptor chain.
    #[derive(Copy, Clone, Debug, Default)]
    #[repr(C)]
    struct VirtqDesc {
        addr: Le64,
        len: Le32,
        flags: Le16,
        next: Le16,
    }

    // Helper structure for testing, used to define the layout of the available ring.
    #[derive(Copy, Clone, Debug, Default)]
    #[repr(C)]
    struct VirtqAvail {
        flags: Le16,
        idx: Le16,
        ring: Le16,
    }

    unsafe impl ByteValued for VirtqAvail {}
    unsafe impl ByteValued for VirtqDesc {}

    // Helper function for testing, used to create a descriptor chain with the specified descriptors.
    fn create_descriptor_chain(
        memory: &GuestMemoryMmap,
        descriptor_array_addr: GuestAddress,
        mut buffers_start_addr: GuestAddress,
        descriptors: Vec<(DescriptorType, u32)>,
    ) -> DescriptorChain<&GuestMemoryMmap> {
        let descriptors_len = descriptors.len();
        for (index, (type_, size)) in descriptors.into_iter().enumerate() {
            let mut flags = 0;
            if let DescriptorType::Writable = type_ {
                flags |= VIRTQ_DESC_F_WRITE;
            }
            if index + 1 < descriptors_len {
                flags |= VIRTQ_DESC_F_NEXT;
            }

            let desc = VirtqDesc {
                addr: buffers_start_addr.raw_value().into(),
                len: size.into(),
                flags: flags.into(),
                next: (index as u16 + 1).into(),
            };

            buffers_start_addr = buffers_start_addr.checked_add(size as u64).unwrap();

            memory
                .write_obj(
                    desc,
                    descriptor_array_addr
                        .checked_add((index * std::mem::size_of::<VirtqDesc>()) as u64)
                        .unwrap(),
                )
                .unwrap();
        }

        let avail_ring = descriptor_array_addr
            .checked_add((descriptors_len * std::mem::size_of::<VirtqDesc>()) as u64)
            .unwrap();
        let avail = VirtqAvail {
            flags: 0.into(),
            idx: 1.into(),
            ring: 0.into(),
        };
        memory.write_obj(avail, avail_ring).unwrap();

        let mut queue = Queue::new(4).unwrap();
        queue
            .try_set_desc_table_address(descriptor_array_addr)
            .unwrap();
        queue.try_set_avail_ring_address(avail_ring).unwrap();
        queue.set_ready(true);
        queue.iter(memory).unwrap().next().unwrap()
    }

    #[test]
    fn simple_chain_reader_test() {
        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&[(memory_start_addr, 0x1000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (DescriptorType::Readable, 8),
                (DescriptorType::Readable, 16),
                (DescriptorType::Readable, 18),
                (DescriptorType::Readable, 64),
            ],
        );

        let mut reader = Reader::new(&memory, chain).unwrap();
        assert_eq!(reader.available_bytes(), 106);
        assert_eq!(reader.bytes_read(), 0);

        let mut buffer = [0; 64];
        reader.read_exact(&mut buffer).unwrap();
        assert_eq!(reader.available_bytes(), 42);
        assert_eq!(reader.bytes_read(), 64);
        assert_eq!(reader.read(&mut buffer).unwrap(), 42);
        assert_eq!(reader.available_bytes(), 0);
        assert_eq!(reader.bytes_read(), 106);
    }

    #[test]
    fn simple_chain_writer_test() {
        let memory_start_addr = GuestAddress(0x0);
        let memory = GuestMemoryMmap::from_ranges(&[(memory_start_addr, 0x1000)]).unwrap();

        let chain = create_descriptor_chain(
            &memory,
            GuestAddress(0x0),
            GuestAddress(0x100),
            vec![
                (DescriptorType::Writable, 8),
                (DescriptorType::Writable, 16),
                (DescriptorType::Writable, 18),
                (DescriptorType::Writable, 64),
            ],
        );

        let mut writer = Writer::new(&memory, chain).unwrap();
        assert_eq!(writer.available_bytes(), 106);
        assert_eq!(writer.bytes_written(), 0);

        let buffer = [0; 64];
        writer.write_all(&buffer).unwrap();
        assert_eq!(writer.available_bytes(), 42);
        assert_eq!(writer.bytes_written(), 64);
        assert_eq!(writer.write(&buffer).unwrap(), 42);
        assert_eq!(writer.available_bytes(), 0);
        assert_eq!(writer.bytes_written(), 106);
    }
}
