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

use std::sync::Arc;

use bytes::Buf;
use http::StatusCode;
use quick_xml::de;
use serde::Deserialize;

use super::core::parse_error;
use super::core::{AzfileCore, ErrorContext};
use opendal_core::raw::*;
use opendal_core::*;

pub struct AzfileLister {
    core: Arc<AzfileCore>,
    ctx: OperationContext,
    path: String,
    limit: Option<usize>,
}

impl AzfileLister {
    pub fn new(
        core: Arc<AzfileCore>,
        ctx: OperationContext,
        path: String,
        limit: Option<usize>,
    ) -> Self {
        Self {
            core,
            ctx,
            path,
            limit,
        }
    }
}

impl oio::PageList for AzfileLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .azfile_list(&self.ctx, &self.path, &self.limit, &ctx.token)
            .await?;

        let status = resp.status();

        if status != StatusCode::OK {
            if status == StatusCode::NOT_FOUND {
                ctx.done = true;
                return Ok(());
            }
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListDirectoriesAndFiles")),
                resp,
            ));
        }

        // Return self at the first page.
        if ctx.token.is_empty() && !ctx.done {
            let e = oio::Entry::new(&self.path, MetadataBuilder::dir().build());
            ctx.entries.push_back(e);
        }

        let bs = resp.into_body();

        let results: EnumerationResults =
            de::from_reader(bs.reader()).map_err(new_xml_deserialize_error)?;

        if results.next_marker.is_empty() {
            ctx.done = true;
        } else {
            ctx.token = results.next_marker;
        }

        for file in results.entries.file {
            let content_length = file.properties.content_length.ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "azfile list response does not contain file content length",
                )
            })?;
            let mut meta = MetadataBuilder::file(content_length);
            meta.etag(file.properties.etag)
                .last_modified(Timestamp::parse_rfc2822(&file.properties.last_modified)?);
            let path = self.path.clone().trim_start_matches('/').to_string() + &file.name;
            ctx.entries.push_back(oio::Entry::new(&path, meta.build()));
        }

        for dir in results.entries.directory {
            let mut meta = MetadataBuilder::dir();
            meta.etag(dir.properties.etag)
                .last_modified(Timestamp::parse_rfc2822(&dir.properties.last_modified)?);
            let path = self.path.clone().trim_start_matches('/').to_string() + &dir.name + "/";
            ctx.entries.push_back(oio::Entry::new(&path, meta.build()));
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct EnumerationResults {
    marker: Option<String>,
    prefix: Option<String>,
    max_results: Option<u32>,
    directory_id: Option<String>,
    entries: Entries,
    #[serde(default)]
    next_marker: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct Entries {
    #[serde(default)]
    file: Vec<File>,
    #[serde(default)]
    directory: Vec<Directory>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct File {
    #[serde(rename = "FileId")]
    file_id: String,
    name: String,
    properties: Properties,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct Directory {
    #[serde(rename = "FileId")]
    file_id: String,
    name: String,
    properties: Properties,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct Properties {
    #[serde(rename = "Content-Length")]
    content_length: Option<u64>,
    #[serde(rename = "CreationTime")]
    creation_time: String,
    #[serde(rename = "LastAccessTime")]
    last_access_time: String,
    #[serde(rename = "LastWriteTime")]
    last_write_time: String,
    #[serde(rename = "ChangeTime")]
    change_time: String,
    #[serde(rename = "Last-Modified")]
    last_modified: String,
    #[serde(rename = "Etag")]
    etag: String,
}

#[cfg(test)]
mod tests {
    use quick_xml::de::from_str;

    use super::*;

    #[test]
    fn test_parse_list_result() {
        let xml = r#"
<?xml version="1.0" encoding="utf-8"?>
<EnumerationResults ServiceEndpoint="https://myaccount.file.core.windows.net/" ShareName="myshare" ShareSnapshot="date-time" DirectoryPath="directory-path">
  <Marker>string-value</Marker>
  <Prefix>string-value</Prefix>
  <MaxResults>100</MaxResults>
  <DirectoryId>directory-id</DirectoryId>
  <Entries>
     <File>
        <Name>Rust By Example.pdf</Name>
        <FileId>13835093239654252544</FileId>
        <Properties>
            <Content-Length>5832374</Content-Length>
            <CreationTime>2023-09-25T12:43:05.8483527Z</CreationTime>
            <LastAccessTime>2023-09-25T12:43:05.8483527Z</LastAccessTime>
            <LastWriteTime>2023-09-25T12:43:08.6337775Z</LastWriteTime>
            <ChangeTime>2023-09-25T12:43:08.6337775Z</ChangeTime>
            <Last-Modified>Mon, 25 Sep 2023 12:43:08 GMT</Last-Modified>
            <Etag>\"0x8DBBDC4F8AC4AEF\"</Etag>
        </Properties>
    </File>
    <Directory>
        <Name>test_list_rich_dir</Name>
        <FileId>12105702186650959872</FileId>
        <Properties>
            <CreationTime>2023-10-15T12:03:40.7194774Z</CreationTime>
            <LastAccessTime>2023-10-15T12:03:40.7194774Z</LastAccessTime>
            <LastWriteTime>2023-10-15T12:03:40.7194774Z</LastWriteTime>
            <ChangeTime>2023-10-15T12:03:40.7194774Z</ChangeTime>
            <Last-Modified>Sun, 15 Oct 2023 12:03:40 GMT</Last-Modified>
            <Etag>\"0x8DBCD76C58C3E96\"</Etag>
        </Properties>
    </Directory>
  </Entries>
  <NextMarker />
</EnumerationResults>
        "#;

        let results: EnumerationResults = from_str(xml).unwrap();

        assert_eq!(results.entries.file[0].name, "Rust By Example.pdf");

        assert_eq!(
            results.entries.file[0].properties.etag,
            "\\\"0x8DBBDC4F8AC4AEF\\\""
        );

        assert_eq!(results.entries.directory[0].name, "test_list_rich_dir");

        assert_eq!(
            results.entries.directory[0].properties.etag,
            "\\\"0x8DBCD76C58C3E96\\\""
        );
    }
}
