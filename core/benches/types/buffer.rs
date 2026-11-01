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

use bytes::Buf;
use bytes::Bytes;
use divan::Bencher;
use divan::black_box;
use opendal::Buffer;
use opendal::BufferCursor;

mod chunk {
    use super::*;

    #[divan::bench]
    fn bytes(b: Bencher) {
        b.with_inputs(|| Bytes::from(vec![1; 10]))
            .bench_refs(|buffer| {
                black_box(buffer.chunk());
            });
    }

    #[divan::bench]
    fn contiguous(b: Bencher) {
        b.with_inputs(|| Buffer::from(vec![1; 10]))
            .bench_refs(|buffer| {
                black_box(buffer.chunk());
            });
    }

    #[divan::bench(args = [10, 1_000, 1_000_000])]
    fn non_contiguous(b: Bencher, parts: usize) {
        b.with_inputs(|| Buffer::from([1; 1].repeat(parts)))
            .bench_refs(|buffer| {
                black_box(buffer.chunk());
            });
    }
}

mod advance {
    use super::*;

    #[divan::bench]
    fn bytes(b: Bencher) {
        b.with_inputs(|| Bytes::from(vec![1; 10]))
            .bench_refs(|buffer| buffer.advance(4));
    }

    #[divan::bench]
    fn contiguous(b: Bencher) {
        b.with_inputs(|| Buffer::from(vec![1; 10]))
            .bench_refs(|buffer| buffer.advance(4));
    }

    #[divan::bench(args = [10, 1_000, 1_000_000])]
    fn non_contiguous(b: Bencher, parts: usize) {
        b.with_inputs(|| Buffer::from([1; 1].repeat(parts)))
            .bench_refs(|buffer| buffer.advance(4));
    }
}

mod truncate {
    use super::*;

    #[divan::bench]
    fn bytes(b: Bencher) {
        b.with_inputs(|| Bytes::from(vec![1; 10]))
            .bench_refs(|buffer| buffer.truncate(5));
    }

    #[divan::bench]
    fn contiguous(b: Bencher) {
        b.with_inputs(|| Buffer::from(vec![1; 10]))
            .bench_refs(|buffer| buffer.truncate(5));
    }

    #[divan::bench(args = [10, 1_000, 1_000_000])]
    fn non_contiguous(b: Bencher, parts: usize) {
        b.with_inputs(|| Buffer::from([1; 1].repeat(parts)))
            .bench_refs(|buffer| buffer.truncate(4));
    }
}

mod iterator {
    use super::*;

    #[divan::bench]
    fn contiguous(b: Bencher) {
        b.with_inputs(|| Buffer::from(vec![1; 1_000_000]))
            .bench_refs(|buffer| for _ in buffer {});
    }

    #[divan::bench(args = [10, 1_000, 1_000_000])]
    fn non_contiguous(b: Bencher, parts: usize) {
        b.with_inputs(|| Buffer::from(vec![1; 1_000_000 / parts].repeat(parts)))
            .bench_refs(|buffer| for _ in buffer {});
    }
}

mod cursor {
    use super::*;

    fn create_large_buffer(size: usize) -> Buffer {
        let mut buf = Vec::with_capacity(size);
        for i in 0..size {
            buf.push(((i * 7) % 256) as u8);
        }

        // Split into 1MB chunks
        let mut pos = 0;
        let mut chunks = vec![];
        while pos < buf.len() {
            let len = (buf.len() - pos).min(1024 * 1024);
            chunks.push(bytes::Bytes::from(buf[pos..pos + len].to_vec()));
            pos += len;
        }

        Buffer::from(chunks)
    }

    #[divan::bench]
    fn buffer_to_vec(bencher: Bencher) {
        use std::io::Read;

        let src = create_large_buffer(50 * 1024 * 1024); // 50MB

        bencher.bench(|| {
            let mut cursor = std::io::Cursor::new(black_box(src.to_vec()));
            let mut buf = vec![0u8; 128 * 1024];
            loop {
                let len = cursor.read(&mut buf[..]).unwrap();
                if len == 0 {
                    break;
                }
                black_box(&buf[..len]);
            }
        });
    }

    #[divan::bench]
    fn buffer_cursor(bencher: Bencher) {
        use std::io::Read;

        let src = create_large_buffer(50 * 1024 * 1024); // 50MB

        bencher.bench(|| {
            let mut cursor = BufferCursor::new(black_box(src.clone()));
            let mut buf = vec![0u8; 128 * 1024];
            loop {
                let len = cursor.read(&mut buf[..]).unwrap();
                if len == 0 {
                    break;
                }
                black_box(&buf[..len]);
            }
        });
    }
}
