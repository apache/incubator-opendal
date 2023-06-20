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

//! WebHDFS response messages

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct BooleanResp {
    pub boolean: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct FileStatusWrapper {
    pub file_status: FileStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct FileStatusesWrapper {
    pub file_statuses: FileStatuses,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct DirectoryListingWrapper {
    pub directory_listing: DirectoryListing,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DirectoryListing {
    pub partial_listing: PartialListing,
    pub remaining_entries: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct PartialListing {
    pub file_statuses: FileStatuses,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct FileStatuses {
    pub file_status: Vec<FileStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub length: u64,
    pub modification_time: i64,

    pub path_suffix: String,
    #[serde(rename = "type")]
    pub ty: FileStatusType,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FileStatusType {
    Directory,
    File,
}

impl Default for DirectoryListing {
    fn default() -> Self {
        Self {
            partial_listing: PartialListing {
                file_statuses: FileStatuses {
                    file_status: vec![],
                },
            },
            remaining_entries: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::raw::oio::Page;
    use crate::services::webhdfs::backend::WebhdfsBackend;
    use crate::services::webhdfs::pager::WebhdfsPager;
    use crate::EntryMode;

    #[test]
    fn test_file_status() {
        let json = r#"
{
  "FileStatus":
  {
    "accessTime"      : 0,
    "blockSize"       : 0,
    "group"           : "supergroup",
    "length"          : 0,
    "modificationTime": 1320173277227,
    "owner"           : "webuser",
    "pathSuffix"      : "",
    "permission"      : "777",
    "replication"     : 0,
    "type"            : "DIRECTORY"
  }
}
"#;
        let status: FileStatusWrapper = serde_json::from_str(json).expect("must success");
        assert_eq!(status.file_status.length, 0);
        assert_eq!(status.file_status.modification_time, 1320173277227);
        assert_eq!(status.file_status.path_suffix, "");
        assert_eq!(status.file_status.ty, FileStatusType::Directory);
    }

    #[tokio::test]
    async fn test_list_empty() {
        let json = r#"
    {
        "FileStatuses": {"FileStatus":[]}
    }
        "#;
        let file_statuses = serde_json::from_str::<FileStatusesWrapper>(json)
            .expect("must success")
            .file_statuses
            .file_status;
        assert!(file_statuses.is_empty());
    }

    #[tokio::test]
    async fn test_list_status() {
        let json = r#"
{
  "FileStatuses":
  {
    "FileStatus":
    [
      {
        "accessTime"      : 1320171722771,
        "blockSize"       : 33554432,
        "group"           : "supergroup",
        "length"          : 24930,
        "modificationTime": 1320171722771,
        "owner"           : "webuser",
        "pathSuffix"      : "a.patch",
        "permission"      : "644",
        "replication"     : 1,
        "type"            : "FILE"
      },
      {
        "accessTime"      : 0,
        "blockSize"       : 0,
        "group"           : "supergroup",
        "length"          : 0,
        "modificationTime": 1320895981256,
        "owner"           : "szetszwo",
        "pathSuffix"      : "bar",
        "permission"      : "711",
        "replication"     : 0,
        "type"            : "DIRECTORY"
      }
    ]
  }
}
            "#;

        let file_statuses = serde_json::from_str::<FileStatusesWrapper>(json)
            .expect("must success")
            .file_statuses
            .file_status;

        let mut backend =
            WebhdfsBackend::new("/webhdfs/v1", "http://localhost:9870", None).unwrap();
        backend.disable_list_batch = true;

        let mut pager = WebhdfsPager::new(backend, "listing/directory", file_statuses);
        let mut entries = vec![];
        while let Some(oes) = pager.next().await.expect("must success") {
            entries.extend(oes);
        }

        entries.sort_by(|a, b| a.path().cmp(b.path()));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path(), "listing/directory/a.patch");
        assert_eq!(entries[0].mode(), EntryMode::FILE);
        assert_eq!(entries[1].path(), "listing/directory/bar/");
        assert_eq!(entries[1].mode(), EntryMode::DIR);
    }

    #[tokio::test]
    async fn test_list_status_batch() {
        let json = r#"
{
    "DirectoryListing": {
        "partialListing": {
            "FileStatuses": {
                "FileStatus": [
                    {
                        "accessTime": 0,
                        "blockSize": 0,
                        "childrenNum": 0,
                        "fileId": 16387,
                        "group": "supergroup",
                        "length": 0,
                        "modificationTime": 1473305882563,
                        "owner": "andrew",
                        "pathSuffix": "bardir",
                        "permission": "755",
                        "replication": 0,
                        "storagePolicy": 0,
                        "type": "DIRECTORY"
                    },
                    {
                        "accessTime": 1473305896945,
                        "blockSize": 1024,
                        "childrenNum": 0,
                        "fileId": 16388,
                        "group": "supergroup",
                        "length": 0,
                        "modificationTime": 1473305896965,
                        "owner": "andrew",
                        "pathSuffix": "bazfile",
                        "permission": "644",
                        "replication": 3,
                        "storagePolicy": 0,
                        "type": "FILE"
                    }
                ]
            }
        },
        "remainingEntries": 2
    }
}
        "#;

        let directory_listing = serde_json::from_str::<DirectoryListingWrapper>(json)
            .expect("must success")
            .directory_listing;

        let file_statuses = directory_listing.partial_listing.file_statuses.file_status;
        let mut backend =
            WebhdfsBackend::new("/webhdfs/v1", "http://localhost:9870", None).unwrap();
        // TODO: need to setup local hadoop cluster to test list status batch
        backend.disable_list_batch = true;

        let mut pager = WebhdfsPager::new(backend, "listing/directory", file_statuses);
        let mut entries = vec![];
        while let Some(oes) = pager.next().await.expect("must success") {
            entries.extend(oes);
        }

        entries.sort_by(|a, b| a.path().cmp(b.path()));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path(), "listing/directory/bardir/");
        assert_eq!(entries[0].mode(), EntryMode::DIR);
        assert_eq!(entries[1].path(), "listing/directory/bazfile");
        assert_eq!(entries[1].mode(), EntryMode::FILE);
    }
}
