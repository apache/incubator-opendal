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

use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use opendal::ErrorKind;
use opendal::Operator;
use opendal::reqsign_core::ProvideCredentialChain;
use opendal::services::S3;
use opendal::services::reqsign_aws_v4::Credential;
use opendal::services::reqsign_aws_v4::ECSCredentialProvider;
use serde::Deserialize;
use serde::Serialize;

const EXPECTED_BODY: &[u8] = b"opendal-s3-container-provider-proof\n";
const CASE_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct RuntimeCredentials {
    access_key_id: String,
    secret_access_key: String,
    session_token: String,
    expiration: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct MetadataResponse<'a> {
    access_key_id: &'a str,
    secret_access_key: &'a str,
    token: &'a str,
    expiration: &'a str,
}

#[derive(Default)]
struct ServerState {
    full_accepted: usize,
    relative_accepted: usize,
    rejected: usize,
    other: usize,
}

struct MetadataServer {
    base_url: String,
    state: Arc<Mutex<ServerState>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MetadataServer {
    fn start(credentials: RuntimeCredentials, expected_token: String) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;

        let state = Arc::new(Mutex::new(ServerState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_state = state.clone();
        let thread_stop = stop.clone();
        let credentials = Arc::new(credentials);

        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        handle_connection(stream, &thread_state, &credentials, &expected_token);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            base_url: format!("http://{address}"),
            state,
            stop,
            handle: Some(handle),
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn counts(&self) -> (usize, usize, usize, usize) {
        let state = self.state.lock().expect("metadata state lock poisoned");
        (
            state.full_accepted,
            state.relative_accepted,
            state.rejected,
            state.other,
        )
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for MetadataServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn handle_connection(
    mut stream: TcpStream,
    state: &Arc<Mutex<ServerState>>,
    credentials: &RuntimeCredentials,
    expected_token: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(size) => {
                request.extend_from_slice(&buffer[..size]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if request.len() > 16 * 1024 {
                    break;
                }
            }
            Err(_) => return,
        }
    }

    let request = String::from_utf8_lossy(&request);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or_default();
    let authorization = request.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then(|| value.trim())
    });

    let (status, body) = if authorization != Some(expected_token) {
        state.lock().expect("metadata state lock poisoned").rejected += 1;
        ("403 Forbidden", b"denied".to_vec())
    } else if path == "/full" {
        state
            .lock()
            .expect("metadata state lock poisoned")
            .full_accepted += 1;
        (
            "200 OK",
            serde_json::to_vec(&MetadataResponse {
                access_key_id: &credentials.access_key_id,
                secret_access_key: &credentials.secret_access_key,
                token: &credentials.session_token,
                expiration: &credentials.expiration,
            })
            .unwrap_or_default(),
        )
    } else if path == "/relative" {
        state
            .lock()
            .expect("metadata state lock poisoned")
            .relative_accepted += 1;
        (
            "200 OK",
            serde_json::to_vec(&MetadataResponse {
                access_key_id: &credentials.access_key_id,
                secret_access_key: &credentials.secret_access_key,
                token: &credentials.session_token,
                expiration: &credentials.expiration,
            })
            .unwrap_or_default(),
        )
    } else {
        state.lock().expect("metadata state lock poisoned").other += 1;
        ("403 Forbidden", b"denied".to_vec())
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(&body);
}

fn required_env(name: &'static str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("required task environment is missing: {name}"))
}

fn write_private_file(path: &str, content: &[u8]) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path)?;
    file.write_all(content)
}

fn build_operator(provider: ECSCredentialProvider) -> Result<Operator, ErrorKind> {
    let chain = ProvideCredentialChain::<Credential>::new().push(provider);
    let builder = S3::default()
        .root(&required_env("OPENDAL_S3_ROOT"))
        .bucket(&required_env("OPENDAL_S3_BUCKET"))
        .endpoint(&required_env("OPENDAL_S3_ENDPOINT"))
        .region(&required_env("OPENDAL_S3_REGION"))
        .credential_provider_chain(chain);

    Operator::new(builder).map_err(|error| error.kind())
}

async fn read_expected(operator: &Operator, key: &str) -> Result<bool, ErrorKind> {
    let body = operator
        .read(key)
        .await
        .map_err(|error| error.kind())?
        .to_vec();
    Ok(body == EXPECTED_BODY)
}

async fn run_case() {
    let credentials_path = required_env("OPENDAL_TASK_S3_CREDENTIAL_FILE");
    let token_file = required_env("OPENDAL_TASK_S3_TOKEN_FILE");
    let relative_key = required_env("OPENDAL_TASK_S3_RELATIVE_KEY");

    let credentials: RuntimeCredentials = serde_json::from_slice(
        &fs::read(&credentials_path).expect("runtime credential file must be readable"),
    )
    .expect("runtime credential file must contain the expected fields");

    let valid_token = uuid::Uuid::new_v4().to_string();
    let wrong_token = uuid::Uuid::new_v4().to_string();
    write_private_file(&token_file, valid_token.as_bytes())
        .expect("metadata token file must be writable");

    let mut server = MetadataServer::start(credentials, valid_token.clone())
        .expect("metadata server must start");

    let full = build_operator(
        ECSCredentialProvider::new()
            .with_endpoint(format!("{}/full", server.base_url()))
            .with_auth_token(valid_token.clone()),
    )
    .unwrap_or_else(|kind| {
        println!("TASK_failure_stage=full_operator_build error_kind={kind:?}");
        panic!("full endpoint operator build failed");
    });
    let full_success = read_expected(&full, &relative_key)
        .await
        .unwrap_or_else(|kind| {
            println!("TASK_failure_stage=full_endpoint_read error_kind={kind:?}");
            panic!("full endpoint S3 read failed");
        });

    let relative = build_operator(
        ECSCredentialProvider::new()
            .with_relative_uri("/relative")
            .with_metadata_uri_override(server.base_url())
            .with_auth_token_file(token_file.clone()),
    )
    .unwrap_or_else(|kind| {
        println!("TASK_failure_stage=relative_operator_build error_kind={kind:?}");
        panic!("relative URI operator build failed");
    });
    let relative_success = read_expected(&relative, &relative_key)
        .await
        .unwrap_or_else(|kind| {
            println!("TASK_failure_stage=relative_uri_read error_kind={kind:?}");
            panic!("relative URI S3 read failed");
        });

    let wrong = build_operator(
        ECSCredentialProvider::new()
            .with_endpoint(format!("{}/wrong", server.base_url()))
            .with_auth_token(wrong_token),
    )
    .unwrap_or_else(|kind| {
        println!("TASK_failure_stage=wrong_nonce_operator_build error_kind={kind:?}");
        panic!("wrong nonce operator build failed");
    });
    let wrong_result = read_expected(&wrong, &relative_key).await;
    let wrong_failed = wrong_result.is_err();
    let wrong_kind = wrong_result
        .err()
        .map(|kind| format!("{kind:?}"))
        .unwrap_or_else(|| "none".to_string());

    let (full_requests, relative_requests, rejected_requests, other_requests) = server.counts();
    server.shutdown();

    println!("TASK_case=s3_container_provider_public_api");
    println!("TASK_full_endpoint_success={full_success}");
    println!("TASK_relative_uri_token_file_success={relative_success}");
    println!("TASK_wrong_nonce_failed={wrong_failed}");
    println!("TASK_wrong_nonce_error_kind={wrong_kind}");
    println!("TASK_metadata_full_requests={full_requests}");
    println!("TASK_metadata_relative_requests={relative_requests}");
    println!("TASK_metadata_rejected_requests={rejected_requests}");
    println!("TASK_metadata_other_requests={other_requests}");

    let passed = full_success
        && relative_success
        && wrong_failed
        && full_requests >= 1
        && relative_requests >= 1
        && rejected_requests >= 1
        && other_requests == 0;
    println!("TASK_case_passed={passed}");
    assert!(passed, "container provider public API case must pass");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_s3_container_provider_public_api() {
    match tokio::time::timeout(CASE_TIMEOUT, run_case()).await {
        Ok(()) => {}
        Err(_) => {
            println!("TASK_case=s3_container_provider_public_api");
            println!("TASK_case_passed=false");
            panic!("container provider public API case timed out");
        }
    }
}
