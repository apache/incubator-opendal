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
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Result;
use opendal::Operator;
use opendal::Scheme;
use tokio::signal;

pub mod config;
pub use config::Config;

mod fuse;

pub async fn execute(cfg: Config) -> Result<()> {
    if cfg.backend.has_host() {
        log::warn!("backend host will be ignored");
    }

    let scheme_str = cfg.backend.scheme();
    let op_args = cfg
        .backend
        .query_pairs()
        .into_owned()
        .collect::<HashMap<String, String>>();

    let scheme = match Scheme::from_str(scheme_str) {
        Ok(Scheme::Custom(_)) | Err(_) => Err(anyhow!("invalid scheme: {}", scheme_str)),
        Ok(s) => Ok(s),
    }?;
    let backend = Operator::via_map(scheme, op_args)?;

    let args = Args {
        mount_path: cfg.mount_path,
        backend,
    };
    execute_inner(args).await
}

#[derive(Debug)]
struct Args {
    mount_path: String,
    backend: Operator,
}
#[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "macos")))]
async fn execute_inner(args: Args) -> Result<()> {
    _ = args.backend;
    Err(anyhow::anyhow!("platform not supported"))
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "macos"))]
async fn execute_inner(args: Args) -> Result<()> {
    use fuse3::path::Session;
    use fuse3::MountOptions;

    let uid = nix::unistd::getuid();
    let gid = nix::unistd::getgid();

    let mut mount_option = MountOptions::default();
    mount_option.uid(uid.into());
    mount_option.gid(gid.into());
    mount_option.no_open_dir_support(true);

    let adapter = fuse::Fuse::new(args.backend, uid.into(), gid.into());

    let mut mount_handle = Session::new(mount_option)
        .mount_with_unprivileged(adapter, args.mount_path)
        .await?;

    let handle = &mut mount_handle;

    tokio::select! {
        res = handle => res?,
        _ = signal::ctrl_c() => {
            mount_handle.unmount().await?
        }
    }

    Ok(())
}
