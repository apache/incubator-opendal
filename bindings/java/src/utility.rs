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

use std::collections::HashSet;

use jni::Env;
use jni::EnvUnowned;
use jni::jni_str;
use jni::objects::JClass;
use jni::objects::JObject;
use jni::objects::JObjectArray;
use jni::sys::jsize;

use crate::Result;
use crate::convert::string_to_jstring;
use crate::error::ThrowException;

/// # Safety
///
/// This function should not be called before the Operator is ready.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_apache_opendal_OpenDAL_loadEnabledServices<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
) -> JObjectArray<'local> {
    env.with_env(|env| intern_load_enabled_services(env))
        .resolve::<ThrowException>()
}

fn intern_load_enabled_services<'local>(env: &mut Env<'local>) -> Result<JObjectArray<'local>> {
    let services = HashSet::from([
        opendal::services::ALIYUN_DRIVE_SCHEME,
        opendal::services::ALLUXIO_SCHEME,
        opendal::services::AZBLOB_SCHEME,
        opendal::services::AZDLS_SCHEME,
        opendal::services::AZFILE_SCHEME,
        opendal::services::B2_SCHEME,
        opendal::services::CACACHE_SCHEME,
        opendal::services::COS_SCHEME,
        opendal::services::DASHMAP_SCHEME,
        opendal::services::DROPBOX_SCHEME,
        opendal::services::ETCD_SCHEME,
        opendal::services::FS_SCHEME,
        opendal::services::GCS_SCHEME,
        opendal::services::GDRIVE_SCHEME,
        opendal::services::GHAC_SCHEME,
        opendal::services::GRIDFS_SCHEME,
        opendal::services::HTTP_SCHEME,
        opendal::services::HF_SCHEME,
        opendal::services::IPFS_SCHEME,
        opendal::services::IPMFS_SCHEME,
        opendal::services::KOOFR_SCHEME,
        opendal::services::MEMCACHED_SCHEME,
        opendal::services::MEMORY_SCHEME,
        opendal::services::MINI_MOKA_SCHEME,
        opendal::services::MOKA_SCHEME,
        opendal::services::MONGODB_SCHEME,
        opendal::services::MYSQL_SCHEME,
        opendal::services::OBS_SCHEME,
        opendal::services::ONEDRIVE_SCHEME,
        opendal::services::OSS_SCHEME,
        opendal::services::PERSY_SCHEME,
        opendal::services::POSTGRESQL_SCHEME,
        opendal::services::REDB_SCHEME,
        opendal::services::REDIS_SCHEME,
        opendal::services::S3_SCHEME,
        opendal::services::SEAFILE_SCHEME,
        #[cfg(unix)]
        opendal::services::SFTP_SCHEME,
        opendal::services::SLED_SCHEME,
        opendal::services::SQLITE_SCHEME,
        opendal::services::SWIFT_SCHEME,
        opendal::services::TIKV_SCHEME,
        opendal::services::TOS_SCHEME,
        opendal::services::UPYUN_SCHEME,
        opendal::services::VERCEL_ARTIFACTS_SCHEME,
        opendal::services::WEBDAV_SCHEME,
        opendal::services::WEBHDFS_SCHEME,
        opendal::services::YANDEX_DISK_SCHEME,
    ]);

    let res = env.new_object_array(
        services.len() as jsize,
        jni_str!("java/lang/String"),
        JObject::null(),
    )?;

    for (idx, service) in services.into_iter().enumerate() {
        let srv = string_to_jstring(env, Some(service))?;
        res.set_element(env, idx, &srv)?;
    }

    Ok(res)
}
