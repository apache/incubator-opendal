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

use std::hint::black_box;
use std::time::Duration;

use bytes::Bytes;
use criterion::Criterion;
use opendal::raw::tests::TEST_RUNTIME;
use opendal::services::{
    // Compfs,
    Fs,
    Monoiofs,
};
use opendal::Operator;
use rand::prelude::*;
use size::Size;

fn main() {
    let mut c = Criterion::default()
        .configure_from_args()
        .measurement_time(Duration::from_secs(10));

    bench_read(&mut c);
    bench_read_concurrent(&mut c);
    bench_write(&mut c);
    bench_write_concurrent(&mut c);

    c.final_summary();
}

fn init_fs_service() -> Operator {
    let builder = Fs::default().root("/tmp/opendal/fs");
    Operator::new(builder).unwrap().finish()
}

fn init_monoiofs_service() -> Operator {
    let builder = Monoiofs::default().root("/tmp/opendal/monoiofs");
    Operator::new(builder).unwrap().finish()
}

// fn init_compfs_service() -> Operator {
//     let builder = Compfs::default().root("/tmp/opendal/compfs");
//     Operator::new(builder).unwrap().finish()
// }

fn gen_bytes(rng: &mut ThreadRng, size: usize) -> Bytes {
    let mut content = vec![0; size];
    rng.fill_bytes(&mut content);
    content.into()
}

fn prepare_test_data(size: usize) -> (String, Bytes) {
    let mut rng = thread_rng();
    (uuid::Uuid::new_v4().to_string(), gen_bytes(&mut rng, size))
}

fn write_test_data(op: &Operator, path: &str, content: Bytes) {
    TEST_RUNTIME.block_on(async { op.write(path, content).await.unwrap() });
}

fn remove_test_data(op: &Operator, path: &str) {
    TEST_RUNTIME.block_on(async {
        op.delete(path).await.unwrap();
    });
}

fn bench_read(c: &mut Criterion) {
    for size in [
        Size::from_kibibytes(4),
        Size::from_kibibytes(256),
        Size::from_mebibytes(4),
        Size::from_mebibytes(16),
    ] {
        let mut group = c.benchmark_group(format!("read {}", size.to_string()));
        group.throughput(criterion::Throughput::Bytes(size.bytes() as u64));
        let (path, content) = prepare_test_data(size.bytes() as usize);

        for init_fn in [
            init_fs_service,
            init_monoiofs_service,
            // init_compfs_service,
        ] {
            let op = init_fn();
            write_test_data(&op, &path, content.clone());
            group.bench_with_input(
                op.info().scheme().to_string(),
                &(op.clone(), &path),
                |b, (op, path)| {
                    b.to_async(&*TEST_RUNTIME).iter(|| async {
                        black_box(op.read(path).await.unwrap());
                    })
                },
            );
            remove_test_data(&op, &path);
        }

        group.finish();
    }
}

fn bench_read_concurrent(c: &mut Criterion) {
    for size in [
        Size::from_kibibytes(4),
        Size::from_kibibytes(256),
        Size::from_mebibytes(4),
        Size::from_mebibytes(16),
    ] {
        let parallel = 16usize;
        let mut group =
            c.benchmark_group(format!("read concurrent {}x{}", parallel, size.to_string()));
        group.throughput(criterion::Throughput::Bytes(
            parallel as u64 * size.bytes() as u64,
        ));
        let (path, content) = prepare_test_data(parallel * size.bytes() as usize);

        for init_fn in [
            init_fs_service,
            init_monoiofs_service,
            // init_compfs_service,
        ] {
            let op = init_fn();
            write_test_data(&op, &path, content.clone());
            group.bench_with_input(
                op.info().scheme().to_string(),
                &(op.clone(), &path),
                |b, (op, path)| {
                    b.to_async(&*TEST_RUNTIME).iter(|| async {
                        let r = op
                            .reader_with(path)
                            .chunk(size.bytes() as usize)
                            .concurrent(parallel)
                            .await
                            .unwrap();
                        black_box(
                            r.read(0..parallel as u64 * size.bytes() as u64)
                                .await
                                .unwrap(),
                        );
                    })
                },
            );
            remove_test_data(&op, &path);
        }

        group.finish();
    }
}

fn bench_write(c: &mut Criterion) {
    for size in [
        Size::from_kibibytes(4),
        Size::from_kibibytes(256),
        Size::from_mebibytes(4),
        Size::from_mebibytes(16),
    ] {
        let mut group = c.benchmark_group(format!("write {}", size.to_string()));
        group.throughput(criterion::Throughput::Bytes(size.bytes() as u64));
        let (path, content) = prepare_test_data(size.bytes() as usize);

        for init_fn in [
            init_fs_service,
            init_monoiofs_service,
            // init_compfs_service,
        ] {
            let op = init_fn();
            group.bench_with_input(
                op.info().scheme().to_string(),
                &(op.clone(), &path, &content),
                |b, (op, path, content)| {
                    b.to_async(&*TEST_RUNTIME).iter(|| async {
                        op.write(path, (*content).clone()).await.unwrap();
                    })
                },
            );
            remove_test_data(&op, &path);
        }

        group.finish();
    }
}

fn bench_write_concurrent(c: &mut Criterion) {
    for size in [
        Size::from_kibibytes(4),
        Size::from_kibibytes(256),
        Size::from_mebibytes(4),
        Size::from_mebibytes(16),
    ] {
        let parallel = 16usize;
        let mut group = c.benchmark_group(format!(
            "write concurrent {}x{}",
            parallel,
            size.to_string()
        ));
        group.throughput(criterion::Throughput::Bytes(
            parallel as u64 * size.bytes() as u64,
        ));
        let (path, content) = prepare_test_data(parallel * size.bytes() as usize);

        for init_fn in [
            init_fs_service,
            init_monoiofs_service,
            // init_compfs_service,
        ] {
            let op = init_fn();
            group.bench_with_input(
                op.info().scheme().to_string(),
                &(op.clone(), &path, &content),
                |b, (op, path, content)| {
                    b.to_async(&*TEST_RUNTIME).iter(|| async {
                        let mut w = op
                            .writer_with(path)
                            .chunk(size.bytes() as usize)
                            .concurrent(parallel)
                            .await
                            .unwrap();
                        w.write((*content).clone()).await.unwrap();
                        w.close().await.unwrap();
                    })
                },
            );
            remove_test_data(&op, &path);
        }

        group.finish();
    }
}
