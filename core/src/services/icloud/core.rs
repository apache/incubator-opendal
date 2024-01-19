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

use async_trait::async_trait;
use bytes::Buf;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use http::header;
use http::header::{IF_MATCH, IF_NONE_MATCH};
use http::Method;
use http::Request;
use http::Response;
use http::StatusCode;
use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

use crate::types::Result;
use crate::{Error, ErrorKind};

use crate::raw::{
    build_rooted_abs_path, get_basename, get_parent, new_json_deserialize_error,
    parse_header_to_str, with_error_response_context, AsyncBody, HttpClient, IncomingAsyncBody,
    OpRead, PathCacher, PathQuery,
};

static ACCOUNT_COUNTRY_HEADER: &str = "X-Apple-ID-Account-Country";
static OAUTH_STATE_HEADER: &str = "X-Apple-OAuth-State";

static SESSION_ID_HEADER: &str = "X-Apple-ID-Session-Id";

static SCNT_HEADER: &str = "scnt";

static SESSION_TOKEN_HEADER: &str = "X-Apple-Session-Token";
static AUTH_ENDPOINT: &str = "https://idmsa.apple.com/appleauth/auth";
static SETUP_ENDPOINT: &str = "https://setup.icloud.com/setup/ws/1";

static APPLE_RESPONSE_HEADER: &str = "X-Apple-I-Rscd";

const AUTH_HEADERS: [(&str, &str); 7] = [
    (
        // This code inspire from
        // https://github.com/picklepete/pyicloud/blob/master/pyicloud/base.py#L392
        "X-Apple-OAuth-Client-Id",
        "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d",
    ),
    ("X-Apple-OAuth-Client-Type", "firstPartyAuth"),
    ("X-Apple-OAuth-Redirect-URI", "https://www.icloud.com"),
    ("X-Apple-OAuth-Require-Grant-Code", "true"),
    ("X-Apple-OAuth-Response-Mode", "web_message"),
    ("X-Apple-OAuth-Response-Type", "code"),
    (
        "X-Apple-Widget-Key",
        "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d",
    ),
];

#[derive(Clone)]
pub struct ServiceInfo {
    pub url: String,
}

#[derive(Clone)]
pub struct SessionData {
    oauth_state: String,
    session_id: Option<String>,
    session_token: Option<String>,

    scnt: Option<String>,
    account_country: Option<String>,

    cookies: BTreeMap<String, String>,
    webservices: HashMap<String, ServiceInfo>,
}

impl SessionData {
    pub fn new() -> SessionData {
        Self {
            oauth_state: format!("auth-{}", uuid::Uuid::new_v4()).to_string(),
            session_id: None,
            session_token: None,
            scnt: None,
            account_country: None,
            cookies: BTreeMap::new(),
            webservices: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct IcloudSigner {
    pub client: HttpClient,

    pub data: SessionData,
    pub apple_id: String,
    pub password: String,

    pub trust_token: Option<String>,
    pub ds_web_auth_token: Option<String>,
    pub is_china_mainland: bool,
}

impl Debug for IcloudSigner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("icloud signer");
        de.field("is_china_mainland", &self.is_china_mainland);
        de.finish()
    }
}

impl IcloudSigner {
    pub fn get_service_info(&self, name: String) -> Option<&ServiceInfo> {
        self.data.webservices.get(&name)
    }

    // create_dir could use client_id
    pub fn get_client_id(&self) -> &String {
        &self.data.oauth_state
    }
}

impl IcloudSigner {
    pub async fn signer(&mut self) -> Result<()> {
        let body = json!({
            "accountName" : self.apple_id,
            "password" : self.password,
            "rememberMe": true,
            "trustTokens": [self.trust_token.clone().unwrap()],
        })
        .to_string();

        let uri = format!("{}/signin?isRememberMeEnable=true", AUTH_ENDPOINT);

        let async_body = AsyncBody::Bytes(bytes::Bytes::from(body));

        let response = self.sign(Method::POST, uri, async_body).await?;

        let status = response.status();

        return match status {
            StatusCode::OK => {
                if let Some(rscd) = response.headers().get(APPLE_RESPONSE_HEADER) {
                    let status_code = StatusCode::from_bytes(rscd.as_bytes()).unwrap();
                    if status_code != StatusCode::CONFLICT {
                        return Err(parse_error(response).await?);
                    }
                }
                self.authenticate().await
            }
            _ => Err(parse_error(response).await?),
        };
    }

    pub async fn authenticate(&mut self) -> Result<()> {
        let body = json!({
            "accountCountryCode": self.data.account_country.as_ref().unwrap_or(&String::new()),
            "dsWebAuthToken":self.ds_web_auth_token.as_ref().unwrap_or(&String::new()),
                    "extended_login": true,
                    "trustToken": self.trust_token.as_ref().unwrap_or(&String::new())
        })
        .to_string();

        let uri = format!("{}/accountLogin", SETUP_ENDPOINT);

        let async_body = AsyncBody::Bytes(bytes::Bytes::from(body));

        let response = self.sign(Method::POST, uri, async_body).await?;

        let status = response.status();

        match status {
            StatusCode::OK => {
                let body = &response.into_body().bytes().await?;
                let auth_info: IcloudWebservicesResponse =
                    serde_json::from_slice(body.chunk()).map_err(new_json_deserialize_error)?;

                if let Some(drivews_url) = &auth_info.webservices.drivews.url {
                    self.data.webservices.insert(
                        String::from("drive"),
                        ServiceInfo {
                            url: drivews_url.to_string(),
                        },
                    );
                }
                if let Some(docws_url) = &auth_info.webservices.docws.url {
                    self.data.webservices.insert(
                        String::from("docw"),
                        ServiceInfo {
                            url: docws_url.to_string(),
                        },
                    );
                }

                if auth_info.hsa_challenge_required {
                    if auth_info.hsa_trusted_browser {
                        Ok(())
                    } else {
                        Err(Error::new(ErrorKind::Unexpected, "Apple icloud AuthenticationFailed:Unauthorized request:Needs two-factor authentication"))
                    }
                } else {
                    Ok(())
                }
            }
            _ => Err(Error::new(
                ErrorKind::Unexpected,
                "Apple icloud AuthenticationFailed:Unauthorized:Invalid token",
            )),
        }
    }
}

impl IcloudSigner {
    pub async fn sign(
        &mut self,
        method: Method,
        uri: String,
        body: AsyncBody,
    ) -> Result<Response<IncomingAsyncBody>> {
        let mut request = Request::builder().method(method).uri(uri);

        request = request.header(OAUTH_STATE_HEADER, self.data.oauth_state.clone());

        if let Some(session_id) = &self.data.session_id {
            request = request.header(SESSION_ID_HEADER, session_id);
        }
        if let Some(scnt) = &self.data.scnt {
            request = request.header(SCNT_HEADER, scnt);
        }

        // China region
        // ("Origin", "https://www.icloud.com.cn")
        // ("Referer", "https://www.icloud.com.cn/")
        // You can get more information from [apple.com](https://support.apple.com/en-us/111754)
        if self.is_china_mainland {
            request = request.header("Origin", "https://www.icloud.com.cn");
            request = request.header("Referer", "https://www.icloud.com.cn/");
        } else {
            request = request.header("Origin", "https://www.icloud.com");
            request = request.header("Referer", "https://www.icloud.com/");
        }

        if !self.data.cookies.is_empty() {
            let cookies: Vec<String> = self
                .data
                .cookies
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            request = request.header(header::COOKIE, cookies.as_slice().join("; "));
        }

        if let Some(headers) = request.headers_mut() {
            headers.insert("Content-Type", "application/json".parse().unwrap());
            headers.insert("Accept", "*/*".parse().unwrap());
            for (key, value) in AUTH_HEADERS {
                headers.insert(key, value.parse().unwrap());
            }
        }

        match self.client.send(request.body(body).unwrap()).await {
            Ok(response) => {
                if let Some(account_country) =
                    parse_header_to_str(response.headers(), ACCOUNT_COUNTRY_HEADER)?
                {
                    self.data.account_country = Some(account_country.to_string());
                }

                if let Some(session_id) =
                    parse_header_to_str(response.headers(), SESSION_ID_HEADER)?
                {
                    self.data.session_id = Some(session_id.to_string());
                }
                if let Some(session_token) =
                    parse_header_to_str(response.headers(), SESSION_TOKEN_HEADER)?
                {
                    self.data.session_token = Some(session_token.to_string());
                }

                if let Some(scnt) = parse_header_to_str(response.headers(), SCNT_HEADER)? {
                    self.data.scnt = Some(scnt.to_string());
                }

                for (key, value) in response.headers() {
                    if key == header::SET_COOKIE {
                        if let Some(cookie) = value.to_str().unwrap().split(';').next() {
                            if let Some((key, value)) = cookie.split_once('=') {
                                self.data
                                    .cookies
                                    .insert(String::from(key), String::from(value));
                            }
                        }
                    }
                }
                match response.status() {
                    StatusCode::UNAUTHORIZED => Err(parse_error(response).await?),
                    _ => Ok(response),
                }
            }
            _ => Err(Error::new(
                ErrorKind::Unexpected,
                "Apple icloud AuthenticationFailed:Unauthorized request",
            )),
        }
    }
}

pub struct IcloudCore {
    pub signer: Arc<Mutex<IcloudSigner>>,
    pub root: String,
    pub path_cache: PathCacher<IcloudPathQuery>,
}

impl Debug for IcloudCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("IcloudCore");
        de.field("root", &self.root);
        de.finish()
    }
}

pub struct IcloudPathQuery {
    pub client: HttpClient,
    pub signer: Arc<Mutex<IcloudSigner>>,
}

impl IcloudPathQuery {
    pub fn new(client: HttpClient, signer: Arc<Mutex<IcloudSigner>>) -> Self {
        IcloudPathQuery { client, signer }
    }
}

#[async_trait]
impl PathQuery for IcloudPathQuery {
    async fn root(&self) -> Result<String> {
        Ok("FOLDER::com.apple.CloudDocs::root".to_string())
    }

    // Retrieves the root directory within the icloud Drive.
    async fn query(&self, parent_id: &str, name: &str) -> Result<Option<String>> {
        let mut signer = self.signer.lock().await;
        let drive_url = signer
            .get_service_info(String::from("drive"))
            .ok_or(Error::new(
                ErrorKind::NotFound,
                "drive service info drivews not found",
            ))?
            .clone()
            .url;

        let uri = format!("{}/retrieveItemDetailsInFolders", drive_url);

        let body = json!([
                         {
                             "drivewsid": parent_id,
                             "partialData": false
                         }
        ])
        .to_string();

        let async_body = AsyncBody::Bytes(bytes::Bytes::from(body));

        let response = signer.sign(Method::POST, uri, async_body).await?;

        if response.status() == StatusCode::OK {
            let body = &response.into_body().bytes().await?;

            let root: Vec<IcloudRoot> =
                serde_json::from_slice(body.chunk()).map_err(new_json_deserialize_error)?;

            let node = &root[0];

            let id = match node.items.iter().find(|it| it.name == name) {
                Some(it) => Ok(Some(it.drivewsid.clone())),
                None => Ok(None),
            }?;
            Ok(id)
        } else {
            Err(parse_error(response).await?)
        }
    }

    async fn create_dir(&self, parent_id: &str, name: &str) -> Result<String> {
        let mut signer = self.signer.lock().await;
        let client_id = signer.get_client_id();
        let drive_url = signer
            .get_service_info(String::from("drive"))
            .ok_or(Error::new(
                ErrorKind::NotFound,
                "drive service info drivews not found",
            ))?
            .url
            .clone();

        let uri = format!("{}/createFolders", drive_url);
        let body = json!(
                         {
                             "destinationDrivewsId": parent_id,
                             "folders": [
                             {
                                "clientId": client_id,
                                "name": name,
                            }
                            ],
                         }
        )
        .to_string();

        let async_body = AsyncBody::Bytes(bytes::Bytes::from(body));
        let response = signer.sign(Method::POST, uri, async_body).await?;

        match response.status() {
            StatusCode::OK => {
                let body = &response.into_body().bytes().await?;

                let create_folder: IcloudCreateFolder =
                    serde_json::from_slice(body.chunk()).map_err(new_json_deserialize_error)?;

                Ok(create_folder.destination_drivews_id)
            }
            _ => Err(parse_error(response).await?),
        }
    }
}

impl IcloudCore {
    // Logs into icloud using the provided credentials.
    pub async fn login(&self) -> Result<()> {
        let mut signer = self.signer.lock().await;

        signer.signer().await
    }

    //Apple Drive
    pub async fn drive(&self) -> Option<DriveService> {
        let clone = self.signer.clone();
        let signer = self.signer.lock().await;

        let docws = signer.get_service_info(String::from("docw")).unwrap();
        signer
            .get_service_info(String::from("drive"))
            .map(|s| DriveService::new(clone, s.url.clone(), docws.url.clone()))
    }

    pub async fn read(&self, path: &str, args: &OpRead) -> Result<Response<IncomingAsyncBody>> {
        self.login().await?;

        let path = build_rooted_abs_path(&self.root, path);
        let base = get_basename(&path);

        let path_id = self.path_cache.get(base).await?.ok_or(Error::new(
            ErrorKind::NotFound,
            &format!("read path not found: {}", base),
        ))?;

        let drive = self
            .drive()
            .await
            .expect("icloud DriveService read drive not found");

        if let Some(docwsid) = path_id.strip_prefix("FILE::com.apple.CloudDocs::") {
            Ok(drive
                .get_file(docwsid, "com.apple.CloudDocs", args.clone())
                .await?)
        } else {
            Err(Error::new(
                ErrorKind::NotFound,
                "icloud DriveService read error",
            ))
        }
    }

    pub async fn stat(&self, path: &str) -> Result<IcloudItem> {
        self.login().await?;

        let path = build_rooted_abs_path(&self.root, path);

        let mut base = get_basename(&path);
        let parent = get_parent(&path);

        if base.ends_with('/') {
            base = base.trim_end_matches('/');
        }

        let file_id = self.path_cache.get(base).await?.ok_or(Error::new(
            ErrorKind::NotFound,
            &format!("stat path not found: {}", base),
        ))?;

        let drive = self
            .drive()
            .await
            .expect("icloud DriveService stat drive not found");

        let folder_id = self.path_cache.get(parent).await?.ok_or(Error::new(
            ErrorKind::NotFound,
            &format!("stat path not found: {}", parent),
        ))?;

        let node = drive.get_root(&folder_id).await?;

        match node.items.iter().find(|it| it.drivewsid == file_id.clone()) {
            Some(it) => Ok(it.clone()),
            None => Err(Error::new(
                ErrorKind::NotFound,
                "icloud DriveService stat get parent items error",
            )),
        }
    }
}

pub struct DriveService {
    signer: Arc<Mutex<IcloudSigner>>,
    drive_url: String,
    docw_url: String,
}

impl DriveService {
    // Constructs an interface to an icloud Drive.
    pub fn new(
        signer: Arc<Mutex<IcloudSigner>>,
        drive_url: String,
        docw_url: String,
    ) -> DriveService {
        DriveService {
            signer,
            drive_url,
            docw_url,
        }
    }

    // Retrieves a root within the icloud Drive.
    // "FOLDER::com.apple.CloudDocs::root"
    pub async fn get_root(&self, id: &str) -> Result<IcloudRoot> {
        let uri = format!("{}/retrieveItemDetailsInFolders", self.drive_url);

        let body = json!([
                         {
                             "drivewsid": id,
                             "partialData": false
                         }
        ])
        .to_string();

        let mut signer = self.signer.lock().await;
        let async_body = AsyncBody::Bytes(bytes::Bytes::from(body));

        let response = signer.sign(Method::POST, uri, async_body).await?;

        if response.status() == StatusCode::OK {
            let body = &response.into_body().bytes().await?;

            let drive_node: Vec<IcloudRoot> =
                serde_json::from_slice(body.chunk()).map_err(new_json_deserialize_error)?;

            Ok(drive_node[0].clone())
        } else {
            Err(parse_error(response).await?)
        }
    }

    pub async fn get_file(
        &self,
        id: &str,
        zone: &str,
        args: OpRead,
    ) -> Result<Response<IncomingAsyncBody>> {
        let uri = format!(
            "{}\
        /ws/{}/download/by_id?document_id={}",
            self.docw_url, zone, id
        );
        debug!("{}", uri);

        let mut signer = self.signer.lock().await;

        let response = signer.sign(Method::GET, uri, AsyncBody::Empty).await?;

        match response.status() {
            StatusCode::OK => {
                let body = &response.into_body().bytes().await?;
                let object: IcloudObject =
                    serde_json::from_slice(body.chunk()).map_err(new_json_deserialize_error)?;

                let url = object.data_token.url.to_string();

                let mut request_builder = Request::builder().method(Method::GET).uri(url);

                if let Some(if_match) = args.if_match() {
                    request_builder = request_builder.header(IF_MATCH, if_match);
                }

                let range = args.range();
                if !range.is_full() {
                    request_builder = request_builder.header(header::RANGE, range.to_header())
                }

                if let Some(if_none_match) = args.if_none_match() {
                    request_builder = request_builder.header(IF_NONE_MATCH, if_none_match);
                }

                let async_body = request_builder.body(AsyncBody::Empty).unwrap();

                let response = signer.client.send(async_body).await?;

                Ok(response)
            }
            _ => Err(parse_error(response).await?),
        }
    }
}

pub async fn parse_error(resp: Response<IncomingAsyncBody>) -> Result<Error> {
    let (parts, body) = resp.into_parts();
    let bs = body.bytes().await?;

    let mut kind = match parts.status.as_u16() {
        421 | 450 | 500 => ErrorKind::NotFound,
        401 => ErrorKind::Unexpected,
        _ => ErrorKind::Unexpected,
    };

    let (message, icloud_err) = serde_json::from_reader::<_, IcloudError>(bs.clone().reader())
        .map(|icloud_err| (format!("{icloud_err:?}"), Some(icloud_err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

    if let Some(icloud_err) = &icloud_err {
        kind = match icloud_err.status_code.as_str() {
            "NOT_FOUND" => ErrorKind::NotFound,
            "PERMISSION_DENIED" => ErrorKind::PermissionDenied,
            _ => ErrorKind::Unexpected,
        }
    }

    let mut err = Error::new(kind, &message);

    err = with_error_response_context(err, parts);

    Ok(err)
}

#[derive(Default, Debug, Deserialize)]
#[allow(dead_code)]
struct IcloudError {
    status_code: String,
    message: String,
}

#[derive(Default, Deserialize, Clone)]
pub struct IcloudWebservicesResponse {
    #[serde(default)]
    pub hsa_challenge_required: bool,
    #[serde(default)]
    pub hsa_trusted_browser: bool,
    pub webservices: Webservices,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct Webservices {
    pub drivews: Drivews,
    pub docws: Docws,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct Drivews {
    #[serde(rename = "pcsRequired")]
    pub pcs_required: bool,
    pub status: String,
    pub url: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct Docws {
    #[serde(rename = "pcsRequired")]
    pub pcs_required: bool,
    pub status: String,
    pub url: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IcloudRoot {
    #[serde(default)]
    pub asset_quota: i64,
    #[serde(default)]
    pub date_created: String,
    #[serde(default)]
    pub direct_children_count: i64,
    pub docwsid: String,
    pub drivewsid: String,
    pub etag: String,
    #[serde(default)]
    pub file_count: i64,
    pub items: Vec<IcloudItem>,
    pub name: String,
    pub number_of_items: i64,
    pub status: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub zone: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IcloudItem {
    #[serde(default)]
    pub asset_quota: Option<i64>,
    #[serde(default)]
    pub date_created: String,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub direct_children_count: Option<i64>,
    pub docwsid: String,
    pub drivewsid: String,
    pub etag: String,
    #[serde(default)]
    pub file_count: Option<i64>,
    pub item_id: Option<String>,
    pub name: String,
    pub parent_id: String,
    #[serde(default)]
    pub size: u64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub zone: String,
    #[serde(default)]
    pub max_depth: Option<String>,
    #[serde(default)]
    pub is_chained_to_parent: Option<bool>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IcloudObject {
    pub document_id: String,
    pub item_id: String,
    pub owner_dsid: i64,
    pub data_token: DataToken,
    pub double_etag: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataToken {
    pub url: String,
    pub token: String,
    pub signature: String,
    pub wrapping_key: String,
    pub reference_signature: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IcloudCreateFolder {
    pub destination_drivews_id: String,
    pub folders: Vec<IcloudItem>,
}

#[cfg(test)]
mod tests {
    use super::{IcloudRoot, IcloudWebservicesResponse};

    #[test]
    fn test_parse_icloud_drive_root_json() {
        let data = r#"{
          "assetQuota": 19603579,
          "dateCreated": "2019-06-10T14:17:49Z",
          "directChildrenCount": 3,
          "docwsid": "root",
          "drivewsid": "FOLDER::com.apple.CloudDocs::root",
          "etag": "w7",
          "fileCount": 22,
          "items": [
            {
              "assetQuota": 19603579,
              "dateCreated": "2021-02-05T08:30:58Z",
              "directChildrenCount": 22,
              "docwsid": "1E013608-C669-43DB-AC14-3D7A4E0A0500",
              "drivewsid": "FOLDER::com.apple.CloudDocs::1E013608-C669-43DB-AC14-3D7A4E0A0500",
              "etag": "sn",
              "fileCount": 22,
              "item_id": "CJWdk48eEAAiEB4BNgjGaUPbrBQ9ek4KBQAoAQ",
              "name": "Downloads",
              "parentId": "FOLDER::com.apple.CloudDocs::root",
              "shareAliasCount": 0,
              "shareCount": 0,
              "type": "FOLDER",
              "zone": "com.apple.CloudDocs"
            },
            {
              "dateCreated": "2019-06-10T14:17:54Z",
              "docwsid": "documents",
              "drivewsid": "FOLDER::com.apple.Keynote::documents",
              "etag": "1v",
              "maxDepth": "ANY",
              "name": "Keynote",
              "parentId": "FOLDER::com.apple.CloudDocs::root",
              "type": "APP_LIBRARY",
              "zone": "com.apple.Keynote"
            },
            {
              "assetQuota": 0,
              "dateCreated": "2024-01-06T02:35:08Z",
              "directChildrenCount": 0,
              "docwsid": "21E4A15E-DA77-472A-BAC8-B0C35A91F237",
              "drivewsid": "FOLDER::com.apple.CloudDocs::21E4A15E-DA77-472A-BAC8-B0C35A91F237",
              "etag": "w8",
              "fileCount": 0,
              "isChainedToParent": true,
              "item_id": "CJWdk48eEAAiECHkoV7ad0cqusiww1qR8jcoAQ",
              "name": "opendal",
              "parentId": "FOLDER::com.apple.CloudDocs::root",
              "shareAliasCount": 0,
              "shareCount": 0,
              "type": "FOLDER",
              "zone": "com.apple.CloudDocs"
            }
          ],
          "name": "",
          "numberOfItems": 16,
          "shareAliasCount": 0,
          "shareCount": 0,
          "status": "OK",
          "type": "FOLDER",
          "zone": "com.apple.CloudDocs"
        }"#;

        let response: IcloudRoot = serde_json::from_str(data).unwrap();
        assert_eq!(response.name, "");
        assert_eq!(response.type_field, "FOLDER");
        assert_eq!(response.zone, "com.apple.CloudDocs");
        assert_eq!(response.docwsid, "root");
        assert_eq!(response.drivewsid, "FOLDER::com.apple.CloudDocs::root");
        assert_eq!(response.etag, "w7");
        assert_eq!(response.file_count, 22);
    }

    #[test]
    fn test_parse_icloud_drive_folder_file() {
        let data = r#"{
          "assetQuota": 19603579,
          "dateCreated": "2021-02-05T08:34:21Z",
          "directChildrenCount": 22,
          "docwsid": "1E013608-C669-43DB-AC14-3D7A4E0A0500",
          "drivewsid": "FOLDER::com.apple.CloudDocs::1E013608-C669-43DB-AC14-3D7A4E0A0500",
          "etag": "w9",
          "fileCount": 22,
          "items": [
            {
            {
              "dateChanged": "2021-02-18T14:10:46Z",
              "dateCreated": "2021-02-10T07:01:34Z",
              "dateModified": "2021-02-10T07:01:34Z",
              "docwsid": "9605331E-7BF3-41A0-A128-A68FFA377C50",
              "drivewsid": "FILE::com.apple.CloudDocs::9605331E-7BF3-41A0-A128-A68FFA377C50",
              "etag": "5b::5a",
              "extension": "pdf",
              "item_id": "CJWdk48eEAAiEJYFMx5780GgoSimj_o3fFA",
              "lastOpenTime": "2021-02-10T10:28:42Z",
              "name": "1-11-ARP-notes",
              "parentId": "FOLDER::com.apple.CloudDocs::1E013608-C669-43DB-AC14-3D7A4E0A0500",
              "size": 639483,
              "type": "FILE",
              "zone": "com.apple.CloudDocs"
            }
            ],
          "name": "Downloads",
          "numberOfItems": 22,
          "parentId": "FOLDER::com.apple.CloudDocs::root",
          "shareAliasCount": 0,
          "shareCount": 0,
          "status": "OK",
          "type": "FOLDER",
          "zone": "com.apple.CloudDocs"
        }"#;

        let response = serde_json::from_str::<IcloudRoot>(data).unwrap();

        assert_eq!(response.name, "Downloads");
        assert_eq!(response.type_field, "FOLDER");
        assert_eq!(response.zone, "com.apple.CloudDocs");
        assert_eq!(response.docwsid, "1E013608-C669-43DB-AC14-3D7A4E0A0500");
        assert_eq!(
            response.drivewsid,
            "FOLDER::com.apple.CloudDocs::1E013608-C669-43DB-AC14-3D7A4E0A0500"
        );
        assert_eq!(response.etag, "w9");
        assert_eq!(response.file_count, 22);
    }

    #[test]
    fn test_parse_icloud_webservices() {
        let data = r#"
        {
          "hsaChallengeRequired": false,
          "hsaTrustedBrowser": true,
          "webservices": {
            "docws": {
              "pcsRequired": true,
              "status": "active",
              "url": "https://p219-docws.icloud.com.cn:443"
            },
            "drivews": {
              "pcsRequired": true,
              "status": "active",
              "url": "https://p219-drivews.icloud.com.cn:443"
            }
          }
        }
        "#;
        let response = serde_json::from_str::<IcloudWebservicesResponse>(data).unwrap();
        assert!(!response.hsa_challenge_required);
        assert!(response.hsa_trusted_browser);
        assert_eq!(
            response.webservices.docws.url,
            Some("https://p219-docws.icloud.com.cn:443".to_string())
        );
        assert_eq!(
            response.webservices.drivews.url,
            Some("https://p219-drivews.icloud.com.cn:443".to_string())
        );
    }
}
