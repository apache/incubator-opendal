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

//! End-to-end master-address resolution against a live GooseFS master.
//!
//! Requires a live GooseFS master. Skip unless `OPENDAL_GOOSEFS_MASTER_ADDR`
//! is set (same env the behavior CI fixture exports).
//!
//! The whole file is one test because it mutates process-global `GOOSEFS_*`
//! environment variables.

use std::time::Duration;

use opendal::Operator;

/// A dummy address from TEST-NET-1 (RFC 5737): guaranteed not to host a
/// GooseFS master.
const DUMMY_AUTHORITY: &str = "192.0.2.5:9999";

/// Bound for an operation expected to fail against [`DUMMY_AUTHORITY`]. The
/// SDK retries master inquiry for far longer than a test should wait, so a
/// timeout counts as "did not reach a master".
const DIAL_FAILURE_TIMEOUT: Duration = Duration::from_secs(20);

/// `goosefs-site.properties` supplies the master address even when the URI
/// authority points somewhere unreachable.
///
/// Reported failure: a client configured through `$GOOSEFS_CONFIG_FILE` still
/// dialed the URI authority and failed with `gRPC transport error`, so a
/// deployment's HA master list never took effect.
#[tokio::test]
async fn site_properties_supply_the_master_address() {
    let Ok(master) = std::env::var("OPENDAL_GOOSEFS_MASTER_ADDR") else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };
    if master.trim().is_empty() {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR is empty");
        return;
    }
    let root = std::env::var("OPENDAL_GOOSEFS_ROOT").unwrap_or_else(|_| "/".to_string());
    let uri = format!("goosefs://{DUMMY_AUTHORITY}{root}");

    // `GOOSEFS_MASTER_ADDR` outranks both sources under test, and a site file
    // deployed on the host would decide the control case below.
    unsafe {
        std::env::remove_var("GOOSEFS_MASTER_ADDR");
        std::env::set_var(
            "GOOSEFS_CONFIG_FILE",
            "/nonexistent/goosefs-site.properties",
        );
    }

    // Control: without a site file the URI authority is used, and writing
    // through it must not reach any master.
    let op = Operator::from_uri(uri.as_str()).expect("build operator from dummy URI");
    let control = tokio::time::timeout(DIAL_FAILURE_TIMEOUT, op.write("resolution-control", "x"))
        .await
        .map_err(|_| "timed out");
    assert!(
        !matches!(control, Ok(Ok(_))),
        "{DUMMY_AUTHORITY} must not serve a master, otherwise this test proves nothing"
    );

    // A site file that lists the live master must win over the URI authority.
    let site_file = std::env::temp_dir().join(format!(
        "opendal_goosefs_resolution_{}_site.properties",
        std::process::id()
    ));
    std::fs::write(
        &site_file,
        format!("goosefs.master.rpc.addresses={master}\n"),
    )
    .expect("write goosefs-site.properties");
    unsafe { std::env::set_var("GOOSEFS_CONFIG_FILE", &site_file) };

    let op =
        Operator::from_uri(uri.as_str()).expect("build operator from dummy URI with a site file");
    let path = format!("resolution-{}", std::process::id());
    op.write(&path, "site-properties-win")
        .await
        .expect("write must reach the master listed in goosefs-site.properties");
    let body = op.read(&path).await.expect("read back").to_bytes();
    assert_eq!(&body[..], b"site-properties-win");

    let _ = op.delete(&path).await;
    let _ = std::fs::remove_file(&site_file);
    unsafe { std::env::remove_var("GOOSEFS_CONFIG_FILE") };
}
