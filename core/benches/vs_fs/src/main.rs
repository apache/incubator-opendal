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

use criterion::Criterion;
use opendal::Operator;
use opendal::blocking;
use opendal::options;
use opendal::services;
use rand::prelude::*;
use rand::rng;

fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_vs_fs(&mut c);

    c.final_summary();
}

fn bench_vs_fs(c: &mut Criterion) {
    let cfg = services::Fs::default().root("/tmp/opendal/");
    let op = Operator::new(cfg).unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();
    let op = blocking::Operator::new(op).unwrap();

    let mut group = c.benchmark_group("read");
    group.throughput(criterion::Throughput::Bytes(16 * 1024 * 1024));

    group.bench_function("std_fs", |b| {
        let path = format!("/tmp/opendal/{}", prepare());
        b.iter(|| {
            let _ = std::fs::read(&path).unwrap();
        });
    });
    group.bench_function("opendal_fs", |b| {
        let path = prepare();
        b.iter(|| {
            let _ = op.read(&path).unwrap();
        });
    });
    group.bench_function("opendal_fs_with_range", |b| {
        let path = prepare();
        b.iter(|| {
            let _ = op
                .read_options(
                    &path,
                    options::ReadOptions {
                        range: (0..16 * 1024 * 1024).into(),
                        ..Default::default()
                    },
                )
                .unwrap();
        });
    });

    group.finish()
}

fn prepare() -> String {
    let mut rng = rng();
    let mut content = vec![0; 16 * 1024 * 1024];
    rng.fill_bytes(&mut content);

    let name = uuid::Uuid::new_v4();
    let path = format!("/tmp/opendal/{name}");
    let _ = std::fs::write(path, content);

    name.to_string()
}
