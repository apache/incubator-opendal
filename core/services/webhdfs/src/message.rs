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

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DirectoryListing {
    pub partial_listing: PartialListing,
    pub remaining_entries: u32,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct PartialListing {
    pub file_statuses: FileStatuses,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct FileStatuses {
    pub file_status: Vec<FileStatus>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub length: u64,
    pub modification_time: i64,

    pub path_suffix: String,
    #[serde(rename = "type")]
    pub ty: FileStatusType,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FileStatusType {
    Directory,
    #[default]
    File,
}

#[cfg(test)]
mod test {
    use super::*;

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

        // we should check the value of FileStatusWrapper directly.
        assert_eq!(file_statuses.len(), 2);
        assert_eq!(file_statuses[0].length, 24930);
        assert_eq!(file_statuses[0].modification_time, 1320171722771);
        assert_eq!(file_statuses[0].path_suffix, "a.patch");
        assert_eq!(file_statuses[0].ty, FileStatusType::File);
        assert_eq!(file_statuses[1].length, 0);
        assert_eq!(file_statuses[1].modification_time, 1320895981256);
        assert_eq!(file_statuses[1].path_suffix, "bar");
        assert_eq!(file_statuses[1].ty, FileStatusType::Directory);
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

        assert_eq!(directory_listing.remaining_entries, 2);
        assert_eq!(
            directory_listing
                .partial_listing
                .file_statuses
                .file_status
                .len(),
            2
        );
        assert_eq!(
            directory_listing.partial_listing.file_statuses.file_status[0].path_suffix,
            "bardir"
        );
        assert_eq!(
            directory_listing.partial_listing.file_statuses.file_status[1].path_suffix,
            "bazfile"
        );
    }
}
