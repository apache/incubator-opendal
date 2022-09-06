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

use std::io::Result;

use super::utils::*;
use log::debug;
use opendal::ObjectMode;
use opendal::Operator;

/// Test services that meet the following capability:
///
/// - can_read
/// - can_write
/// - can_blocking
/// - can_list
macro_rules! behavior_blocking_list_test {
    ($service:ident, $($(#[$meta:meta])* $test:ident),*,) => {
        paste::item! {
            mod [<services_ $service:lower _blocking_list>] {
                $(
                    #[test]
                    $(
                        #[$meta]
                    )*
                    fn [< $test >]() -> std::io::Result<()> {
                        let op = $crate::utils::init_service(opendal::Scheme::$service, true);
                        match op {
                            Some(op) if op.metadata().can_read()
                                && op.metadata().can_write()
                                && op.metadata().can_blocking() && op.metadata().can_list() => $crate::blocking_list::$test(op),
                            Some(_) => {
                                log::warn!("service {} doesn't support read, ignored", opendal::Scheme::$service);
                                Ok(())
                            },
                            None => {
                                log::warn!("service {} not initiated, ignored", opendal::Scheme::$service);
                                Ok(())
                            }
                        }
                    }
                )*
            }
        }
    };
}

#[macro_export]
macro_rules! behavior_blocking_list_tests {
     ($($service:ident),*) => {
        $(
            behavior_blocking_list_test!(
                $service,

                test_list_dir,
            );
        )*
    };
}

/// List dir should return newly created file.
pub fn test_list_dir(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes();

    op.object(&path)
        .blocking_write(content)
        .expect("write must succeed");

    let obs = op.object("/").blocking_list()?;
    let mut found = false;
    for de in obs {
        let de = de?;
        let meta = de.blocking_metadata()?;
        if de.path() == path {
            assert_eq!(meta.mode(), ObjectMode::FILE);
            assert_eq!(meta.content_length(), size as u64);

            found = true
        }
    }
    assert!(found, "file should be found in list");

    op.object(&path)
        .blocking_delete()
        .expect("delete must succeed");
    Ok(())
}
