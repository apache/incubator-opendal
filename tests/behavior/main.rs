// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use]
mod read;
#[macro_use]
mod blocking_read;
mod utils;
#[macro_use]
mod write;
#[macro_use]
mod blocking_write;
#[macro_use]
mod base;
#[macro_use]
mod list;
#[macro_use]
mod presign;
#[macro_use]
mod blocking_list;
#[macro_use]
mod multipart;
#[macro_use]
mod multipart_presign;

/// Generate real test cases.
/// Update function list while changed.
macro_rules! behavior_tests {
    ($($service:ident),*) => {
        $(
            behavior_base_tests!($service);
            // can_read && !can_write
            behavior_read_tests!($service);
            // can_read && !can_write && can_blocking
            behavior_blocking_read_tests!($service);
            // can_read && can_write
            behavior_write_tests!($service);
            // can_read && can_write && can_blocking
            behavior_blocking_write_tests!($service);
            // can_read && can_write && can_list
            behavior_list_tests!($service);
            // can_read && can_write && can_presign
            behavior_presign_tests!($service);
            // can_read && can_write && can_blocking && can_list
            behavior_blocking_list_tests!($service);
            // can_read && can_write && can_multipart
            behavior_multipart_tests!($service);
            // can_read && can_write && can_multipart && can_presign
            behavior_multipart_presign_tests!($service);
        )*
    };
}

behavior_tests!(Azblob);
behavior_tests!(Fs);
cfg_if::cfg_if! { if #[cfg(feature = "services-ftp")] { behavior_tests!(Ftp); }}
behavior_tests!(Memory);
behavior_tests!(Gcs);
behavior_tests!(Ipmfs);
cfg_if::cfg_if! { if #[cfg(feature = "services-hdfs")] { behavior_tests!(Hdfs); }}
cfg_if::cfg_if! { if #[cfg(feature = "services-http")] {behavior_tests!(Http); }}
behavior_tests!(Obs);
behavior_tests!(S3);
