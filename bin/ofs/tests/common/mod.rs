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

use std::{collections::HashMap, env, sync::OnceLock};

use opendal::{Capability, Operator};
use tempfile::TempDir;
use test_context::TestContext;
use tokio::runtime::{self, Runtime};

static INIT_LOGGER: OnceLock<()> = OnceLock::new();
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub struct OfsTestContext {
    pub mount_point: TempDir,
    pub capability: Capability,
    mount_handle: ofs::fuse::MountHandle,
}

impl TestContext for OfsTestContext {
    fn setup() -> Self {
        let backend = backend();
        let capability = backend.info().full_capability();

        INIT_LOGGER.get_or_init(env_logger::init);

        let mount_point = tempfile::tempdir().unwrap();
        let mount_point_str = mount_point.path().to_string_lossy().to_string();
        let mount_handle = RUNTIME
            .get_or_init(|| {
                runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("build runtime")
            })
            .block_on(async move {
                ofs::fuse::Fuse::new()
                    .mount_with_unprivileged(mount_point_str, backend)
                    .await
            })
            .unwrap();

        OfsTestContext {
            mount_point,
            capability,
            mount_handle,
        }
    }

    fn teardown(self) {
        RUNTIME
            .get()
            .expect("runtime")
            .block_on(async move { self.mount_handle.unmount().await })
            .unwrap();
        self.mount_point.close().unwrap();
    }
}

fn backend() -> Operator {
    let scheme = env::var("OPENDAL_TEST").unwrap().parse().unwrap();
    let prefix = format!("opendal_{scheme}_");

    let mut cfg = env::vars()
        .filter_map(|(k, v)| {
            k.to_lowercase()
                .strip_prefix(&prefix)
                .map(|k| (k.to_string(), v))
        })
        .collect::<HashMap<String, String>>();

    // Use random root unless OPENDAL_DISABLE_RANDOM_ROOT is set to true.
    let disable_random_root = env::var("OPENDAL_DISABLE_RANDOM_ROOT").unwrap_or_default() == "true";
    if !disable_random_root {
        let root = format!(
            "{}{}/",
            cfg.get("root").cloned().unwrap_or_else(|| "/".to_string()),
            uuid::Uuid::new_v4()
        );
        cfg.insert("root".to_string(), root);
    }

    Operator::via_map(scheme, cfg).unwrap()
}
