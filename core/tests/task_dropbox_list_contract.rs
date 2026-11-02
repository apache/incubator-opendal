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

use opendal::ErrorKind;

#[tokio::test]
async fn dropbox_nested_list_paths_with_owned_cleanup() {
    assert_eq!(std::env::var("OPENDAL_TEST").as_deref(), Ok("dropbox"));
    let op = opendal_testkit::init_test_service()
        .map_err(|error| error.kind())
        .expect("test service configuration must succeed")
        .expect("real Dropbox must be configured");
    assert_eq!(op.info().scheme().to_string(), "dropbox");

    let parent = format!("pr7399-{}/", uuid::Uuid::new_v4());
    let nested = format!("{parent}nested/");
    let file = format!("{nested}payload");
    let content = "opendal-pr7399";
    eprintln!("probe_namespace={parent}");

    let observed: Result<(bool, bool, bool, usize), (&str, ErrorKind)> = async {
        op.create_dir(&parent)
            .await
            .map_err(|error| ("create_parent", error.kind()))?;
        op.create_dir(&nested)
            .await
            .map_err(|error| ("create_nested", error.kind()))?;
        op.write(&file, content)
            .await
            .map_err(|error| ("write", error.kind()))?;
        let metadata = op
            .stat(&file)
            .await
            .map_err(|error| ("stat_control", error.kind()))?;
        let entries = op
            .list(&nested)
            .await
            .map_err(|error| ("list_nested", error.kind()))?;
        Ok((
            metadata.mode().is_file() && metadata.content_length() == content.len() as u64,
            entries.iter().any(|entry| entry.path() == file),
            entries.iter().any(|entry| entry.path() == "payload"),
            entries.len(),
        ))
    }
    .await;
    eprintln!("probe_observation={observed:?}");

    // Clean only known owned paths before asserting a possibly failing list result.
    let mut cleanup_ok = true;
    for path in [&file, &nested, &parent] {
        if let Err(error) = op.delete(path).await {
            if error.kind() != ErrorKind::NotFound {
                eprintln!("cleanup_error_kind={:?}", error.kind());
                cleanup_ok = false;
            }
        }
    }
    eprintln!("cleanup_ok={cleanup_ok}");
    assert!(cleanup_ok, "known-path cleanup must succeed");

    let (stat_control, nested_path_present, basename_only_present, count) =
        observed.expect("Dropbox operation failed at the reported stage and error kind");
    eprintln!(
        "stat_control={stat_control} nested_path_present={nested_path_present} basename_only_present={basename_only_present} count={count}"
    );
    assert!(stat_control, "the exact written object must exist");
    assert!(
        nested_path_present,
        "listing must retain the operator-relative parent path"
    );
}
