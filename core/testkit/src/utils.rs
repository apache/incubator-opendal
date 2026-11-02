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

use std::collections::HashMap;
use std::env;
use std::num::NonZeroU64;
use std::sync::LazyLock;
use std::time::Duration;

use opendal_core::Error;
use opendal_core::ErrorKind;
use opendal_core::Operator;
use opendal_core::Result;
use opendal_core::layers::CapabilityOverrideLayer;
use opendal_layer_logging::LoggingLayer;
use opendal_layer_retry::RetryLayer;
use opendal_layer_timeout::TimeoutLayer;
use sha2::Digest;
use sha2::Sha256;

const OPENDAL_TEST_CAPABILITY_OVERRIDES: &str = "OPENDAL_TEST_CAPABILITY_OVERRIDES";
const OPENDAL_TEST_UNSET_VALUE: &str = "__OPENDAL_TEST_UNSET__";

pub(crate) fn sha256_digest(data: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    let digest = Sha256::digest(data);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String must succeed");
    }
    output
}

/// TEST_RUNTIME is the runtime used for running tests.
pub static TEST_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

fn collect_config(
    prefix: &str,
    vars: impl IntoIterator<Item = (String, String)>,
) -> HashMap<String, String> {
    vars.into_iter()
        .filter_map(|(k, v)| {
            if v == OPENDAL_TEST_UNSET_VALUE {
                return None;
            }
            k.to_lowercase()
                .strip_prefix(prefix)
                .map(|k| (k.to_string(), v))
        })
        .collect()
}

/// Init a service with given scheme.
///
/// - Load scheme from `OPENDAL_TEST`
/// - Construct a new Operator with given root.
/// - Else, returns a `None` to represent no valid config for operator.
pub fn init_test_service() -> Result<Option<Operator>> {
    let _ = dotenvy::dotenv();

    let scheme = if let Ok(v) = env::var("OPENDAL_TEST") {
        v
    } else {
        return Ok(None);
    };

    let prefix = {
        let scheme_key = scheme.replace('-', "_");
        format!("opendal_{scheme_key}_")
    };

    let mut cfg = collect_config(&prefix, env::vars());

    // Use random root unless OPENDAL_DISABLE_RANDOM_ROOT is set to true.
    let disable_random_root = env::var("OPENDAL_DISABLE_RANDOM_ROOT").unwrap_or_default() == "true";
    if !disable_random_root {
        let root = format!(
            "{}{}/",
            cfg.get("root").cloned().unwrap_or_else(|| "/".to_string()),
            uuid::Uuid::new_v4()
        );
        cfg.insert("root".to_string(), root);
    }

    // string-based scheme uses a hyphen ('-') as the connector
    let scheme = scheme.replace('_', "-");
    let mut op = Operator::via_iter(scheme, cfg).expect("must succeed");

    if let Ok(overrides) = env::var(OPENDAL_TEST_CAPABILITY_OVERRIDES)
        && overrides != OPENDAL_TEST_UNSET_VALUE
    {
        op = op.layer(CapabilityOverrideLayer::from_overrides(&overrides)?);
    }

    let mut timeout = TimeoutLayer::new();
    if let Ok(value) = env::var("OPENDAL_TEST_IO_TIMEOUT")
        && value != OPENDAL_TEST_UNSET_VALUE
    {
        let seconds = value.parse::<NonZeroU64>().map_err(|err| {
            Error::new(
                ErrorKind::ConfigInvalid,
                "OPENDAL_TEST_IO_TIMEOUT must be a positive whole number of seconds",
            )
            .set_source(err)
        })?;
        timeout = timeout.with_io_timeout(Duration::from_secs(seconds.get()));
    }

    let op = op
        .layer(LoggingLayer::default())
        .layer(timeout)
        .layer(RetryLayer::new().with_max_times(4).with_jitter());

    Ok(Some(op))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_config_skips_unset_values() {
        let cfg = collect_config(
            "opendal_s3_",
            [
                (
                    "OPENDAL_S3_ENDPOINT".to_string(),
                    "http://localhost".to_string(),
                ),
                (
                    "OPENDAL_S3_ALLOW_ANONYMOUS".to_string(),
                    OPENDAL_TEST_UNSET_VALUE.to_string(),
                ),
                ("OPENDAL_S3_PASSWORD".to_string(), String::new()),
                ("OPENDAL_GCS_BUCKET".to_string(), "test".to_string()),
            ],
        );

        assert_eq!(
            cfg.get("endpoint").map(String::as_str),
            Some("http://localhost")
        );
        assert_eq!(cfg.get("password").map(String::as_str), Some(""));
        assert!(!cfg.contains_key("allow_anonymous"));
        assert!(!cfg.contains_key("bucket"));
    }
}
