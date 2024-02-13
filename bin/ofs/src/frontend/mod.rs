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

use anyhow::Result;
use opendal::Operator;

#[cfg(target_os = "linux")]
mod fuse;

pub(crate) struct FrontendArgs {
    pub mount_path: String,
    pub backend: Operator,
}

pub(crate) struct Frontend;

impl Frontend {
    #[cfg(any(not(target_os = "linux")))]
    pub async fn execute(_: FrontendArgs) -> Result<()> {
        Err(anyhow::anyhow!("platform not supported"))
    }

    #[cfg(target_os = "linux")]
    pub async fn execute(args: FrontendArgs) -> Result<()> {
        use fuse3::path::Session;
        use fuse3::MountOptions;

        let mut mount_option = MountOptions::default();
        mount_option.uid(nix::unistd::getuid().into());
        mount_option.gid(nix::unistd::getgid().into());

        let ofs = fuse::Ofs::new(args.backend);

        let mount_handle = Session::new(mount_option)
            .mount_with_unprivileged(ofs, args.mount_path)
            .await?;

        mount_handle.await?;

        Ok(())
    }
}
