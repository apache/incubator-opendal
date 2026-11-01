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

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
pub struct GraphErrorResponse {
    pub error: GraphError,
}

#[derive(Debug, Deserialize)]
pub struct GraphError {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct GraphOAuthRefreshTokenResponseBody {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64, // in seconds
}

/// We `$select` some fields when sending GET requests.
/// Please keep [`OneDriveItem`] fields and this variable in sync.
/// Read more at https://learn.microsoft.com/en-us/graph/query-parameters?tabs=http#select-parameter
pub const GENERAL_SELECT_PARAM: &str =
    "$select=id,name,lastModifiedDateTime,eTag,size,parentReference,folder,file";

/// We `$select` some fields when listing versions.
/// Please keep [`OneDriveItemVersion`] fields and this variable in sync.
/// Read more at https://learn.microsoft.com/en-us/graph/query-parameters?tabs=http#select-parameter
pub const VERSION_SELECT_PARAM: &str = "$select=id,size,lastModifiedDateTime";

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphApiOneDriveListResponse {
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
    pub value: Vec<OneDriveItem>,
}

/// A `DriveItem`
/// read more at https://learn.microsoft.com/en-us/onedrive/developer/rest-api/resources/driveitem
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveItem {
    pub id: String,
    pub name: String,
    pub last_modified_date_time: String,
    pub e_tag: String,
    pub size: i64,
    pub parent_reference: ParentReference,
    #[serde(flatten)]
    pub item_type: ItemType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentReference {
    pub path: String,
    pub drive_id: String,
    pub id: String,
}

/// Additional properties when represents a facet of a "DriveItem":
/// - "file", read more at https://learn.microsoft.com/en-us/onedrive/developer/rest-api/resources/file
/// - "folder", read more at https://learn.microsoft.com/en-us/onedrive/developer/rest-api/resources/folder
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum ItemType {
    Folder { folder: Folder },
    File { file: File },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct File {
    mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    child_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphApiOneDriveVersionsResponse {
    pub value: Vec<OneDriveItemVersion>,
}

/// A `driveItemVersion`
///
/// Read more at https://learn.microsoft.com/en-us/graph/api/resources/driveitemversion
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveItemVersion {
    pub id: String,
    pub last_modified_date_time: String,
    pub size: i64,
}

// Microsoft's documentation wants developers to set this as URL parameters. Some APIs use
// this as an data field in the payload.
pub const REPLACE_EXISTING_ITEM_WHEN_CONFLICT: &str = "replace";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDirPayload {
    #[serde(rename = "@microsoft.graph.conflictBehavior")]
    conflict_behavior: String,
    name: String,
    folder: EmptyStruct,
}

impl CreateDirPayload {
    pub fn new(name: String) -> Self {
        Self {
            conflict_behavior: REPLACE_EXISTING_ITEM_WHEN_CONFLICT.to_string(),
            name,
            folder: EmptyStruct {},
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmptyStruct {}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileUploadItem {
    #[serde(rename = "@microsoft.graph.conflictBehavior")]
    conflict_behavior: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveUploadSessionCreationResponseBody {
    pub upload_url: String,
    pub expiration_date_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneDriveUploadSessionCreationRequestBody {
    item: FileUploadItem,
}

impl OneDriveUploadSessionCreationRequestBody {
    pub fn new(path: String) -> Self {
        OneDriveUploadSessionCreationRequestBody {
            item: FileUploadItem {
                conflict_behavior: REPLACE_EXISTING_ITEM_WHEN_CONFLICT.to_string(),
                name: path,
            },
        }
    }
}

/// represents copy and rename (update) operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDrivePatchRequestBody {
    pub parent_reference: ParentReference,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveMonitorStatus {
    pub percentage_complete: f64, // useful for debugging
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_one_drive_list_response_json() {
        let data = r#"{
            "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('opendal@outlook.com')/drive/root/children(id,name,lastModifiedDateTime,eTag,size,parentReference,folder,file)",
            "value": [
                {
                    "@odata.etag": "\"{3B131E1C-7D81-20AF-80D0-450D00000000},10\"",
                    "eTag": "\"{3B131E1C-7D81-20AF-80D0-450D00000000},10\"",
                    "id": "A0AA0A000A000A0A!3397",
                    "lastModifiedDateTime": "2025-02-23T11:45:26Z",
                    "name": "empty_folder",
                    "size": 0,
                    "parentReference": {
                        "driveType": "personal",
                        "driveId": "A0AA0A000A000A0A",
                        "id": "A0AA0A000A000A0A!sea8cc6beffdb43d7976fbc7da445c639",
                        "name": "Documents",
                        "path": "/drive/root:",
                        "siteId": "5f1f11f3-a6b4-4414-aee0-215c774f80db"
                    },
                    "folder": {
                        "childCount": 0,
                        "view": {
                            "sortBy": "name",
                            "sortOrder": "ascending",
                            "viewType": "thumbnails"
                        }
                    }
                },
                {
                    "@odata.etag": "\"{3B131E1C-7D81-20AF-80D0-710000000000},6\"",
                    "eTag": "\"{3B131E1C-7D81-20AF-80D0-710000000000},6\"",
                    "id": "A0AA0A000A000A0A!113",
                    "lastModifiedDateTime": "2025-02-16T19:48:39Z",
                    "name": "folder_a",
                    "size": 10560537,
                    "parentReference": {
                        "driveType": "personal",
                        "driveId": "A0AA0A000A000A0A",
                        "id": "A0AA0A000A000A0A!sea8cc6beffdb43d7976fbc7da445c639",
                        "name": "Documents",
                        "path": "/drive/root:",
                        "siteId": "5f1f11f3-a6b4-4414-aee0-215c774f80db"
                    },
                    "folder": {
                        "childCount": 5,
                        "view": {
                            "sortBy": "name",
                            "sortOrder": "ascending",
                            "viewType": "thumbnails"
                        }
                    }
                }
            ]
        }"#;

        let response: GraphApiOneDriveListResponse = serde_json::from_str(data).unwrap();
        assert_eq!(response.value.len(), 2);
        let item = &response.value[0];
        assert_eq!(item.name, "empty_folder");
        assert_eq!(item.last_modified_date_time, "2025-02-23T11:45:26Z");
        assert_eq!(item.e_tag, "\"{3B131E1C-7D81-20AF-80D0-450D00000000},10\"");
        assert_eq!(item.size, 0);
        assert_eq!(item.parent_reference.path, "/drive/root:");
        if let ItemType::Folder { folder, .. } = &item.item_type {
            assert_eq!(folder.child_count, 0);
        } else {
            panic!("item_type is not a folder");
        }
    }

    #[test]
    fn test_parse_one_drive_list_response_with_next_link_json() {
        let response_json = r#"{
            "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('opendal@outlook.com')/drive/root/children(id,name,lastModifiedDateTime,eTag,size,parentReference,folder,file)",
            "@odata.nextLink": "https://graph.microsoft.com/v1.0/me/drive/root/children?$select=id%2cname%2clastModifiedDateTime%2ceTag%2csize%2cparentReference%2cfolder%2cfile&$top=2&$skiptoken=UGFnZWQ9VFJVRSZwX1NvcnRCZWhhdmlvcj0xJnBfRmlsZUxlYWZSZWY9Zm9sZGVyX2EmcF9JRD03MDAz",
            "value": [
                {
                    "@odata.etag": "\"{3B131E1C-7D81-20AF-80D0-450D00000000},10\"",
                    "eTag": "\"{3B131E1C-7D81-20AF-80D0-450D00000000},10\"",
                    "id": "A0AA0A000A000A0A!3397",
                    "lastModifiedDateTime": "2025-02-23T11:45:26Z",
                    "name": "empty_folder",
                    "size": 0,
                    "parentReference": {
                        "driveType": "personal",
                        "driveId": "A0AA0A000A000A0A",
                        "id": "A0AA0A000A000A0A!sea8cc6beffdb43d7976fbc7da445c639",
                        "name": "Documents",
                        "path": "/drive/root:",
                        "siteId": "5f1f11f3-a6b4-4414-aee0-215c774f80db"
                    },
                    "folder": {
                        "childCount": 0,
                        "view": {
                            "sortBy": "name",
                            "sortOrder": "ascending",
                            "viewType": "thumbnails"
                        }
                    }
                }
            ]
        }"#;

        let response: GraphApiOneDriveListResponse = serde_json::from_str(response_json).unwrap();
        assert_eq!(response.value.len(), 1);
        let item = &response.value[0];
        if let ItemType::Folder { folder, .. } = &item.item_type {
            assert_eq!(folder.child_count, 0);
        } else {
            panic!("item_type is not a folder");
        }
    }

    #[test]
    fn test_parse_one_drive_file_json() {
        let data = r#"{
            "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('opendal%40outlook.com')/drive/root(id,name,lastModifiedDateTime,eTag,size,parentReference,folder,file)/$entity",
            "@odata.etag": "\"{3B131E1C-7D81-20AF-80D0-720000000000},2\"",
            "eTag": "\"{3B131E1C-7D81-20AF-80D0-720000000000},2\"",
            "id": "A0AA0A000A000A0A!114",
            "lastModifiedDateTime": "2025-02-16T19:49:05Z",
            "name": "filename.txt",
            "size": 3,
            "parentReference": {
                "driveType": "personal",
                "driveId": "A0AA0A000A000A0A",
                "id": "A0AA0A000A000A0A!113",
                "name": "folder_a",
                "path": "/drive/root:/folder_a",
                "siteId": "5f1f11f3-a6b4-4414-aee0-215c774f80db"
            },
            "file": {
                "mimeType": "text/plain",
                "hashes": {
                    "quickXorHash": "79jFLwAAAAAAAAAAAwAAAAAAAAA=",
                    "sha1Hash": "57218C316B6921E2CD61027A2387EDC31A2D9471",
                    "sha256Hash": "F1945CD6C19E56B3C1C78943EF5EC18116907A4CA1EFC40A57D48AB1DB7ADFC5"
                }
            }
        }"#;

        let item: OneDriveItem = serde_json::from_str(data).unwrap();
        assert_eq!(item.name, "filename.txt");
        assert_eq!(item.last_modified_date_time, "2025-02-16T19:49:05Z");
        assert_eq!(item.e_tag, "\"{3B131E1C-7D81-20AF-80D0-720000000000},2\"");
        assert_eq!(item.size, 3);
        assert_eq!(item.parent_reference.id, "A0AA0A000A000A0A!113");
        if let ItemType::File { file, .. } = &item.item_type {
            assert_eq!(file.mime_type, "text/plain");
        } else {
            panic!("item_type is not a file");
        }
    }

    #[test]
    fn test_parse_one_drive_monitor_status_json() {
        let data = r#"{
            "@odata.context": "https://my.microsoftpersonalcontent.com/personal/A0AA0A000A000A0A/_api/v2.0/$metadata#oneDrive.asynchronousOperationStatus",
            "percentageComplete": 100.0,
            "resourceId": "01JP3NYHGSBJ7R42UN65HZ333HZFWQTGL4",
            "status": "completed"
        }"#;

        let response: OneDriveMonitorStatus = serde_json::from_str(data).unwrap();
        assert_eq!(response.status, "completed");
    }

    #[test]
    fn test_parse_one_drive_item_versions_json() {
        let data = r#"{
            "@odata.context": "https://graph.microsoft.com/v1.0/$metadata#users('erickgdev%40outlook.com')/drive/root/versions(id,size,lastModifiedDateTime)",
            "value": [
                {
                    "id": "2.0",
                    "lastModifiedDateTime": "2025-03-16T17:02:49Z",
                    "size": 74758
                },
                {
                    "id": "1.0",
                    "lastModifiedDateTime": "2025-03-12T21:59:54Z",
                    "size": 74756
                }
            ]
        }"#;

        let response: GraphApiOneDriveVersionsResponse = serde_json::from_str(data).unwrap();
        assert_eq!(response.value.len(), 2);
        let version = &response.value[0];
        assert_eq!(version.id, "2.0");
        assert_eq!(version.last_modified_date_time, "2025-03-16T17:02:49Z");
        assert_eq!(version.size, 74758);
    }
}
