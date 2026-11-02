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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]

use std::collections::HashMap;

use http::Uri;
use http::response::Parts;
use opendal_core::{Error, ErrorKind, Result};

/// Configuration parsed from Azure storage connection string.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AzureStorageConfig {
    /// Storage account name.
    pub account_name: Option<String>,
    /// Storage account shared key.
    pub account_key: Option<String>,
    /// Shared access signature token.
    pub sas_token: Option<String>,
    /// Service endpoint.
    pub endpoint: Option<String>,
    /// OAuth client id.
    pub client_id: Option<String>,
    /// OAuth client secret.
    pub client_secret: Option<String>,
    /// OAuth tenant id.
    pub tenant_id: Option<String>,
    /// OAuth authority host.
    pub authority_host: Option<String>,
}

enum AzureStorageCredential {
    SharedAccessSignature(String),
    SharedKey(String, String),
}

/// Parses an [Azure connection string][1] into a configuration object.
///
/// The connection string doesn't have to specify all required parameters
/// because the user is still allowed to set them later directly on the object.
///
/// The function takes an AzureStorageService parameter because it determines
/// the fields used to parse the endpoint.
///
/// [1]: https://learn.microsoft.com/en-us/azure/storage/common/storage-configure-connection-string
pub fn azure_config_from_connection_string(
    conn_str: &str,
    storage: AzureStorageService,
) -> Result<AzureStorageConfig> {
    let key_values = parse_connection_string(conn_str)?;

    if storage == AzureStorageService::Blob {
        // Try to read development storage configuration.
        if let Some(development_config) = collect_blob_development_config(&key_values, &storage) {
            return Ok(AzureStorageConfig {
                account_name: Some(development_config.account_name),
                account_key: Some(development_config.account_key),
                endpoint: Some(development_config.endpoint),
                ..Default::default()
            });
        }
    }

    let mut config = AzureStorageConfig {
        account_name: key_values.get("AccountName").cloned(),
        endpoint: collect_endpoint(&key_values, &storage)?,
        ..Default::default()
    };

    if let Some(creds) = collect_credentials(&key_values) {
        set_credentials(&mut config, creds);
    };

    Ok(config)
}

/// The service that a connection string refers to. The type influences
/// interpretation of endpoint-related fields during parsing.
#[derive(PartialEq)]
pub enum AzureStorageService {
    /// Azure Blob Storage.
    Blob,

    /// Azure File Storage.
    File,

    /// Azure Data Lake Storage Gen2.
    /// Backed by Blob Storage but exposed through a different endpoint (`dfs`).
    Adls,
}

/// Try to extract the storage account name from an Azure endpoint.
///
/// Returns `None` if the endpoint doesn't match known Azure Storage suffixes.
pub fn azure_account_name_from_endpoint(endpoint: &str) -> Option<String> {
    /// Known Azure Storage endpoint suffixes.
    const KNOWN_ENDPOINT_SUFFIXES: &[&str] = &[
        "core.windows.net",       // Azure public cloud
        "core.usgovcloudapi.net", // Azure US Government
        "core.chinacloudapi.cn",  // Azure China
    ];

    let endpoint: &str = endpoint
        .strip_prefix("http://")
        .or_else(|| endpoint.strip_prefix("https://"))
        .unwrap_or(endpoint);

    let (account_name, service_endpoint) = endpoint.split_once('.')?;
    let (_storage_service, endpoint_suffix) = service_endpoint.split_once('.')?;

    if KNOWN_ENDPOINT_SUFFIXES.contains(&endpoint_suffix.trim_end_matches('/')) {
        Some(account_name.to_string())
    } else {
        None
    }
}

/// Takes a semicolon-delimited Azure Storage connection string and returns
/// key-value pairs split from it.
fn parse_connection_string(conn_str: &str) -> Result<HashMap<String, String>> {
    conn_str
        .trim()
        .replace("\n", "")
        .split(';')
        .filter(|&field| !field.is_empty())
        .map(|field| {
            let (key, value) = field.trim().split_once('=').ok_or(Error::new(
                ErrorKind::ConfigInvalid,
                format!("Invalid connection string, expected '=' in field: {field}"),
            ))?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

fn collect_blob_development_config(
    key_values: &HashMap<String, String>,
    storage: &AzureStorageService,
) -> Option<DevelopmentStorageConfig> {
    debug_assert!(
        storage == &AzureStorageService::Blob,
        "Azurite Development Storage only supports Blob Storage"
    );

    // Azurite defaults.
    const AZURITE_DEFAULT_STORAGE_ACCOUNT_NAME: &str = "devstoreaccount1";
    const AZURITE_DEFAULT_STORAGE_ACCOUNT_KEY: &str =
        "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==";

    const AZURITE_DEFAULT_BLOB_URI: &str = "http://127.0.0.1:10000";

    if key_values.get("UseDevelopmentStorage") != Some(&"true".to_string()) {
        return None; // Not using development storage
    }

    let account_name = key_values
        .get("AccountName")
        .cloned()
        .unwrap_or(AZURITE_DEFAULT_STORAGE_ACCOUNT_NAME.to_string());
    let account_key = key_values
        .get("AccountKey")
        .cloned()
        .unwrap_or(AZURITE_DEFAULT_STORAGE_ACCOUNT_KEY.to_string());
    let development_proxy_uri = key_values
        .get("DevelopmentStorageProxyUri")
        .cloned()
        .unwrap_or(AZURITE_DEFAULT_BLOB_URI.to_string());

    Some(DevelopmentStorageConfig {
        endpoint: format!("{development_proxy_uri}/{account_name}"),
        account_name,
        account_key,
    })
}

/// Helper struct to hold development storage aka Azurite configuration.
struct DevelopmentStorageConfig {
    account_name: String,
    account_key: String,
    endpoint: String,
}

/// Parses an endpoint from the key-value pairs if possible.
///
/// Users are still able to later supplement configuration with an endpoint,
/// so endpoint-related fields aren't enforced.
fn collect_endpoint(
    key_values: &HashMap<String, String>,
    storage: &AzureStorageService,
) -> Result<Option<String>> {
    match storage {
        AzureStorageService::Blob => collect_or_build_endpoint(key_values, "BlobEndpoint", "blob"),
        AzureStorageService::File => collect_or_build_endpoint(key_values, "FileEndpoint", "file"),
        AzureStorageService::Adls => {
            // ADLS doesn't have a dedicated endpoint field and we can only
            // build it from parts.
            if let Some(dfs_endpoint) = collect_endpoint_from_parts(key_values, "dfs")? {
                Ok(Some(dfs_endpoint.clone()))
            } else {
                Ok(None)
            }
        }
    }
}

fn collect_credentials(key_values: &HashMap<String, String>) -> Option<AzureStorageCredential> {
    if let Some(sas_token) = key_values.get("SharedAccessSignature") {
        Some(AzureStorageCredential::SharedAccessSignature(
            sas_token.clone(),
        ))
    } else if let (Some(account_name), Some(account_key)) =
        (key_values.get("AccountName"), key_values.get("AccountKey"))
    {
        Some(AzureStorageCredential::SharedKey(
            account_name.clone(),
            account_key.clone(),
        ))
    } else {
        // We default to no authentication. This is not an error because e.g.
        // Azure Active Directory configuration is typically not passed via
        // connection strings.
        // Users may also set credentials manually on the configuration.
        None
    }
}

fn set_credentials(config: &mut AzureStorageConfig, creds: AzureStorageCredential) {
    match creds {
        AzureStorageCredential::SharedAccessSignature(sas_token) => {
            config.sas_token = Some(sas_token);
        }
        AzureStorageCredential::SharedKey(account_name, account_key) => {
            config.account_name = Some(account_name);
            config.account_key = Some(account_key);
        }
    }
}

fn collect_or_build_endpoint(
    key_values: &HashMap<String, String>,
    endpoint_key: &str,
    service_name: &str,
) -> Result<Option<String>> {
    if let Some(endpoint) = key_values.get(endpoint_key) {
        Ok(Some(endpoint.clone()))
    } else if let Some(built_endpoint) = collect_endpoint_from_parts(key_values, service_name)? {
        Ok(Some(built_endpoint.clone()))
    } else {
        Ok(None)
    }
}

fn collect_endpoint_from_parts(
    key_values: &HashMap<String, String>,
    storage_endpoint_name: &str,
) -> Result<Option<String>> {
    let (account_name, endpoint_suffix) = match (
        key_values.get("AccountName"),
        key_values.get("EndpointSuffix"),
    ) {
        (Some(name), Some(suffix)) => (name, suffix),
        _ => return Ok(None), // Can't build an endpoint if one of them is missing
    };

    let protocol = key_values
        .get("DefaultEndpointsProtocol")
        .map(String::as_str)
        .unwrap_or("https"); // Default to HTTPS if not specified
    if protocol != "http" && protocol != "https" {
        return Err(Error::new(
            ErrorKind::ConfigInvalid,
            format!("Invalid DefaultEndpointsProtocol: {protocol}"),
        ));
    }

    Ok(Some(format!(
        "{protocol}://{account_name}.{storage_endpoint_name}.{endpoint_suffix}"
    )))
}

/// Add response context to error.
///
/// This helper function will:
///
/// - remove sensitive or useless headers from parts.
/// - fetch uri if parts extensions contains `Uri`.
/// - censor sensitive SAS URI query parameters
pub fn with_azure_error_response_context(mut err: Error, mut parts: Parts) -> Error {
    if let Some(uri) = parts.extensions.get::<Uri>() {
        err = err.with_context("uri", censor_sas_uri(uri));
    }

    // The following headers may contains sensitive information.
    parts.headers.remove("Set-Cookie");
    parts.headers.remove("WWW-Authenticate");
    parts.headers.remove("Proxy-Authenticate");

    err = err.with_context("response", format!("{parts:?}"));

    err
}

fn censor_sas_uri(uri: &Uri) -> String {
    if let Some(query) = uri.query() {
        // There is a large set of query parameters specified for SAS URIs.
        // Some of them may be useful to an attacker, but the most important part is the signature.
        // Without a signature, an attacker will not be able to replay the request.
        // For now, just remove the signature.
        //
        // https://learn.microsoft.com/en-us/rest/api/storageservices/create-account-sas
        // https://learn.microsoft.com/en-us/rest/api/storageservices/create-service-sas
        // https://learn.microsoft.com/en-us/rest/api/storageservices/create-user-delegation-sas
        //
        let path = uri.path();
        let new_query: String = query
            .split("&")
            .filter(|p| !p.starts_with("sig="))
            .collect::<Vec<_>>()
            .join("&");
        let mut parts = uri.clone().into_parts();
        parts.path_and_query = Some(format!("{path}?{new_query}").try_into().unwrap());
        Uri::from_parts(parts).unwrap().to_string()
    } else {
        uri.to_string()
    }
}

#[cfg(test)]
mod tests {
    use http::Uri;

    use super::censor_sas_uri;

    use super::{
        AzureStorageConfig, AzureStorageService, azure_account_name_from_endpoint,
        azure_config_from_connection_string,
    };

    #[test]
    fn test_azure_config_from_connection_string() {
        #[allow(unused_mut)]
        let mut test_cases = vec![
            ("minimal fields",
                (AzureStorageService::Blob, "BlobEndpoint=https://testaccount.blob.core.windows.net/"),
                Some(AzureStorageConfig{
                    endpoint: Some("https://testaccount.blob.core.windows.net/".to_string()),
                    ..Default::default()
                }),
            ),
            ("basic creds and blob endpoint",
                (AzureStorageService::Blob, "AccountName=testaccount;AccountKey=testkey;BlobEndpoint=https://testaccount.blob.core.windows.net/"),
                Some(AzureStorageConfig{
                    account_name: Some("testaccount".to_string()),
                    account_key: Some("testkey".to_string()),
                    endpoint: Some("https://testaccount.blob.core.windows.net/".to_string()),
                     ..Default::default()
                    }),
            ),
            ("SAS token",
                (AzureStorageService::Blob, "SharedAccessSignature=blablabla"),
                Some(AzureStorageConfig{
                    sas_token: Some("blablabla".to_string()),
                    ..Default::default()
                }),
            ),
            ("endpoint from parts",
                (AzureStorageService::Blob, "AccountName=testaccount;EndpointSuffix=core.windows.net;DefaultEndpointsProtocol=https"),
                Some(AzureStorageConfig{
                    endpoint: Some("https://testaccount.blob.core.windows.net".to_string()),
                    account_name: Some("testaccount".to_string()),
                    ..Default::default()
                }),
            ),
            ("endpoint from parts and no protocol",
                (AzureStorageService::Blob, "AccountName=testaccount;EndpointSuffix=core.windows.net"),
                Some(AzureStorageConfig{
                    // Defaults to https
                    endpoint: Some("https://testaccount.blob.core.windows.net".to_string()),
                    account_name: Some("testaccount".to_string()),
                    ..Default::default()
                }),
            ),
            ("prefers sas over key",
                (AzureStorageService::Blob, "AccountName=testaccount;AccountKey=testkey;SharedAccessSignature=sas_token"),
                Some(AzureStorageConfig{
                    sas_token: Some("sas_token".to_string()),
                    account_name: Some("testaccount".to_string()),
                    ..Default::default()
                }),
            ),
            ("development storage",
                (AzureStorageService::Blob, "UseDevelopmentStorage=true",),
                Some(AzureStorageConfig{
                    account_name: Some("devstoreaccount1".to_string()),
                    account_key: Some("Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==".to_string()),
                    endpoint: Some("http://127.0.0.1:10000/devstoreaccount1".to_string()),
                    ..Default::default()
                }),
            ),
            ("development storage with custom account values",
                (AzureStorageService::Blob, "UseDevelopmentStorage=true;AccountName=myAccount;AccountKey=myKey"),
                Some(AzureStorageConfig {
                    endpoint: Some("http://127.0.0.1:10000/myAccount".to_string()),
                    account_name: Some("myAccount".to_string()),
                    account_key: Some("myKey".to_string()),
                    ..Default::default()
                }),
            ),
            ("development storage with custom uri",
                (AzureStorageService::Blob, "UseDevelopmentStorage=true;DevelopmentStorageProxyUri=http://127.0.0.1:12345"),
                Some(AzureStorageConfig {
                    endpoint: Some("http://127.0.0.1:12345/devstoreaccount1".to_string()),
                    account_name: Some("devstoreaccount1".to_string()),
                    account_key: Some("Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==".to_string()),
                    ..Default::default()
                }),
            ),
            ("unknown key is ignored",
                (AzureStorageService::Blob, "SomeUnknownKey=123;BlobEndpoint=https://testaccount.blob.core.windows.net/"),
                Some(AzureStorageConfig{
                    endpoint: Some("https://testaccount.blob.core.windows.net/".to_string()),
                    ..Default::default()
                }),
            ),
            ("leading and trailing `;`",
                (AzureStorageService::Blob, ";AccountName=testaccount;"),
                Some(AzureStorageConfig {
                    account_name: Some("testaccount".to_string()),
                    ..Default::default()
                }),
            ),
            ("line breaks",
                (AzureStorageService::Blob, r#"
                    AccountName=testaccount;
                    AccountKey=testkey;
                    EndpointSuffix=core.windows.net;
                    DefaultEndpointsProtocol=https"#),
                Some(AzureStorageConfig {
                    account_name: Some("testaccount".to_string()),
                    account_key: Some("testkey".to_string()),
                    endpoint: Some("https://testaccount.blob.core.windows.net".to_string()),
                    ..Default::default()
                }),
            ),
            ("missing equals",
                (AzureStorageService::Blob, "AccountNameexample;AccountKey=example;EndpointSuffix=core.windows.net;DefaultEndpointsProtocol=https",),
                None, // This should fail due to missing '='
            ),
            ("with invalid protocol",
                (AzureStorageService::Blob, "DefaultEndpointsProtocol=ftp;AccountName=example;EndpointSuffix=core.windows.net",),
                None, // This should fail due to invalid protocol
            ),
        ];

        test_cases.push(
            ("adls endpoint from parts",
                (AzureStorageService::Adls, "AccountName=testaccount;EndpointSuffix=core.windows.net;DefaultEndpointsProtocol=https"),
                Some(AzureStorageConfig{
                    account_name: Some("testaccount".to_string()),
                    endpoint: Some("https://testaccount.dfs.core.windows.net".to_string()),
                    ..Default::default()
                }),
            )
        );

        test_cases.extend(vec![
            (
                "file endpoint from field",
                (
                    AzureStorageService::File,
                    "FileEndpoint=https://testaccount.file.core.windows.net",
                ),
                Some(AzureStorageConfig {
                    endpoint: Some("https://testaccount.file.core.windows.net".to_string()),
                    ..Default::default()
                }),
            ),
            (
                "file endpoint from parts",
                (
                    AzureStorageService::File,
                    "AccountName=testaccount;EndpointSuffix=core.windows.net",
                ),
                Some(AzureStorageConfig {
                    account_name: Some("testaccount".to_string()),
                    endpoint: Some("https://testaccount.file.core.windows.net".to_string()),
                    ..Default::default()
                }),
            ),
        ]);

        test_cases.push((
            "azdls development storage",
            (AzureStorageService::Adls, "UseDevelopmentStorage=true"),
            Some(AzureStorageConfig::default()), // Azurite doesn't support ADLSv2, so we ignore this case
        ));

        for (name, (storage, conn_str), expected) in test_cases {
            let actual = azure_config_from_connection_string(conn_str, storage);

            if let Some(expected) = expected {
                assert_azure_storage_config_eq(&actual.expect(name), &expected, name);
            } else {
                assert!(actual.is_err(), "Expected error for case: {name}");
            }
        }
    }

    #[test]
    fn test_azure_account_name_from_endpoint() {
        let test_cases = vec![
            ("https://account.blob.core.windows.net", Some("account")),
            (
                "https://account.blob.core.usgovcloudapi.net",
                Some("account"),
            ),
            (
                "https://account.blob.core.chinacloudapi.cn",
                Some("account"),
            ),
            ("https://account.dfs.core.windows.net", Some("account")),
            ("https://account.blob.core.windows.net/", Some("account")),
            ("https://account.blob.unknown.suffix.com", None),
            ("http://blob.core.windows.net", None),
        ];
        for (endpoint, expected_account_name) in test_cases {
            let account_name = azure_account_name_from_endpoint(endpoint);
            assert_eq!(
                account_name,
                expected_account_name.map(|s| s.to_string()),
                "Endpoint: {endpoint}"
            );
        }
    }

    #[test]
    fn test_azure_uri_context_removes_sig() {
        let uri: Uri = "https://foo.bar/path?foo=foo&sig=SENSITIVE&bar=bar"
            .parse()
            .unwrap();
        let expected = "https://foo.bar/path?foo=foo&bar=bar";
        assert_eq!(censor_sas_uri(&uri), expected);
    }

    /// Helper function to compare AzureStorageConfig fields manually.
    fn assert_azure_storage_config_eq(
        actual: &AzureStorageConfig,
        expected: &AzureStorageConfig,
        name: &str,
    ) {
        assert_eq!(
            actual.account_name, expected.account_name,
            "account_name mismatch: {name}"
        );
        assert_eq!(
            actual.account_key, expected.account_key,
            "account_key mismatch: {name}"
        );
        assert_eq!(
            actual.endpoint, expected.endpoint,
            "endpoint mismatch: {name}"
        );
        assert_eq!(
            actual.sas_token, expected.sas_token,
            "sas_token mismatch: {name}"
        );
    }
}
