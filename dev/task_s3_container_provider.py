#!/usr/bin/env python3
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import uuid
from pathlib import Path


EXPECTED_BODY = b"opendal-s3-container-provider-proof\n"
RELATIVE_PREFIX = "task-s3-container-provider"
FIXTURE_ENV = {
    "root": "OPENDAL_S3_ROOT",
    "bucket": "OPENDAL_S3_BUCKET",
    "endpoint": "OPENDAL_S3_ENDPOINT",
    "access_key_id": "OPENDAL_S3_ACCESS_KEY_ID",
    "secret_access_key": "OPENDAL_S3_SECRET_ACCESS_KEY",
    "region": "OPENDAL_S3_REGION",
}
NATIVE_ALLOWLIST = (
    "running ",
    "test task_s3_container_provider_public_api",
    "test result:",
    "TASK_",
    "failures:",
    "---- task_s3",
    "thread 'task_s3",
    "error:",
    "error[",
)


class StageError(Exception):
    def __init__(self, stage: str, code: str):
        self.stage = stage
        self.code = code


def write_text(path: Path, value: str) -> None:
    path.write_text(value.rstrip("\n") + "\n", encoding="utf-8")


def github_mask(value: str) -> None:
    if value:
        print(f"::add-mask::{value}", flush=True)


def run_capture(command: list[str], *, cwd: Path | None = None, env=None, timeout=60):
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=timeout,
        check=False,
    )


def aws_error_code(stderr: str, returncode: int) -> str:
    match = re.search(r"An error occurred \(([A-Za-z0-9_.-]{1,80})\)", stderr)
    if match:
        return match.group(1)
    match = re.search(r"\((AccessDenied|InvalidClientTokenId|ExpiredToken|TokenRefreshRequired)\)", stderr)
    if match:
        return match.group(1)
    return f"exit_{returncode}"


def fixture_configuration() -> dict[str, str]:
    values = {field: os.environ.get(name, "") for field, name in FIXTURE_ENV.items()}
    if any(not value for field, value in values.items() if field != "root"):
        raise StageError("fixture", "missing")
    return values


def aws_environment(fixture: dict[str, str]) -> dict[str, str]:
    env = os.environ.copy()
    for name in (
        "AWS_PROFILE",
        "AWS_SESSION_TOKEN",
        "AWS_SECURITY_TOKEN",
        "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
        "AWS_CONTAINER_CREDENTIALS_FULL_URI",
        "AWS_CONTAINER_AUTHORIZATION_TOKEN",
        "AWS_CONTAINER_AUTHORIZATION_TOKEN_FILE",
        "AWS_WEB_IDENTITY_TOKEN_FILE",
        "AWS_ROLE_ARN",
    ):
        env.pop(name, None)

    env.update(
        {
            "AWS_ACCESS_KEY_ID": fixture["access_key_id"],
            "AWS_SECRET_ACCESS_KEY": fixture["secret_access_key"],
            "AWS_REGION": fixture["region"],
            "AWS_DEFAULT_REGION": fixture["region"],
            "AWS_EC2_METADATA_DISABLED": "true",
            "AWS_MAX_ATTEMPTS": "1",
            "AWS_RETRY_MODE": "standard",
            "AWS_PAGER": "",
            "PAGER": "cat",
        }
    )
    return env


def run_aws(aws: str, args: list[str], env: dict[str, str], *, timeout: int = 120):
    try:
        return run_capture([aws, *args], env=env, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise StageError("aws", "timeout") from None


def mint_temporary_credentials(aws: str, env: dict[str, str]) -> dict[str, str]:
    proc = run_aws(
        aws,
        ["sts", "get-session-token", "--duration-seconds", "900", "--output", "json"],
        env,
    )
    if proc.returncode != 0:
        raise StageError("sts", aws_error_code(proc.stderr, proc.returncode))

    try:
        response = json.loads(proc.stdout)
        credentials = response["Credentials"]
        minted = {
            "AccessKeyId": credentials["AccessKeyId"],
            "SecretAccessKey": credentials["SecretAccessKey"],
            "SessionToken": credentials["SessionToken"],
            "Expiration": credentials["Expiration"],
        }
    except (KeyError, TypeError, ValueError):
        raise StageError("sts", "invalid_response") from None

    if any(not value for value in minted.values()):
        raise StageError("sts", "incomplete_credentials")

    github_mask(minted["AccessKeyId"])
    github_mask(minted["SecretAccessKey"])
    github_mask(minted["SessionToken"])
    return minted


def write_private_file(path: Path, content: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as file:
        file.write(content)


def normalize_root(value: str) -> str:
    return "/".join(part for part in value.split("/") if part)


def endpoint_args(fixture: dict[str, str]) -> list[str]:
    return ["--endpoint-url", fixture["endpoint"]]


def seed_object(
    aws: str,
    env: dict[str, str],
    fixture: dict[str, str],
    full_key: str,
    body_file: Path,
) -> None:
    proc = run_aws(
        aws,
        [
            "s3api",
            "put-object",
            "--bucket",
            fixture["bucket"],
            "--key",
            full_key,
            "--body",
            str(body_file),
            *endpoint_args(fixture),
        ],
        env,
    )
    if proc.returncode != 0:
        raise StageError("seed", aws_error_code(proc.stderr, proc.returncode))


def native_environment(
    fixture: dict[str, str],
    metadata_response_file: Path,
    token_file: Path,
    relative_key: str,
) -> dict[str, str]:
    env = os.environ.copy()
    for name in list(env):
        if (
            name.startswith("AWS_")
            or name.startswith("OPENDAL_TASK_S3_")
            or name in (FIXTURE_ENV["access_key_id"], FIXTURE_ENV["secret_access_key"])
        ):
            env.pop(name, None)

    env.update(
        {
            FIXTURE_ENV["root"]: fixture["root"],
            FIXTURE_ENV["bucket"]: fixture["bucket"],
            FIXTURE_ENV["endpoint"]: fixture["endpoint"],
            FIXTURE_ENV["region"]: fixture["region"],
            "OPENDAL_TASK_S3_METADATA_RESPONSE_FILE": str(metadata_response_file),
            "OPENDAL_TASK_S3_TOKEN_FILE": str(token_file),
            "OPENDAL_TASK_S3_RELATIVE_KEY": relative_key,
            "CARGO_BUILD_JOBS": "4",
            "CARGO_TERM_COLOR": "never",
            "RUST_LOG": "off",
            "RUST_BACKTRACE": "0",
        }
    )
    return env


def sanitize_native_output(output: str) -> list[str]:
    lines = []
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith(NATIVE_ALLOWLIST):
            lines.append(stripped)
    return lines


def run_native(
    repo: Path,
    env: dict[str, str],
    native_log: Path,
    native_exit: Path,
) -> tuple[int, str]:
    cargo = shutil.which("cargo")
    if cargo is None:
        write_text(native_log, "native.stage=not_run\nnative.code=cargo_missing")
        write_text(native_exit, "not_run")
        return 127, "not_run"

    command = [
        cargo,
        "test",
        "--manifest-path",
        str(repo / "core" / "Cargo.toml"),
        "--locked",
        "--test",
        "task_s3_container_provider",
        "--features",
        "tests,services-s3",
        "--",
        "--nocapture",
        "--test-threads=1",
    ]

    try:
        proc = run_capture(command, cwd=repo, env=env, timeout=600)
        returncode = proc.returncode
        output = proc.stdout + proc.stderr
    except subprocess.TimeoutExpired as error:
        returncode = 124
        output = error.stdout or ""
        if isinstance(output, bytes):
            output = output.decode("utf-8", "replace")

    sanitized = sanitize_native_output(output)
    if not sanitized:
        sanitized = ["native.output=no_allowlisted_lines"]
    write_text(native_log, "\n".join(sanitized))
    write_text(native_exit, str(returncode))

    footer = next(
        (line for line in reversed(sanitized) if line.startswith("test result:")),
        "absent",
    )
    return returncode, footer


def list_exact_key(
    aws: str,
    env: dict[str, str],
    fixture: dict[str, str],
    full_key: str,
) -> list[tuple[str, str]]:
    proc = run_aws(
        aws,
        [
            "s3api",
            "list-object-versions",
            "--bucket",
            fixture["bucket"],
            "--prefix",
            full_key,
            "--output",
            "json",
            *endpoint_args(fixture),
        ],
        env,
    )
    if proc.returncode != 0:
        raise StageError("cleanup_list", aws_error_code(proc.stderr, proc.returncode))

    try:
        response = json.loads(proc.stdout)
    except ValueError:
        raise StageError("cleanup_list", "invalid_response") from None

    exact = []
    for section in ("Versions", "DeleteMarkers"):
        for item in response.get(section, []):
            if item.get("Key") == full_key:
                exact.append((full_key, str(item.get("VersionId", "null"))))
    return list(dict.fromkeys(exact))


def delete_exact_versions(
    aws: str,
    env: dict[str, str],
    fixture: dict[str, str],
    objects: list[tuple[str, str]],
) -> None:
    for key, version in objects:
        proc = run_aws(
            aws,
            [
                "s3api",
                "delete-object",
                "--bucket",
                fixture["bucket"],
                "--key",
                key,
                "--version-id",
                version,
                *endpoint_args(fixture),
            ],
            env,
        )
        if proc.returncode != 0:
            raise StageError("cleanup_delete", aws_error_code(proc.stderr, proc.returncode))


def cleanup_exact_object(
    aws: str,
    env: dict[str, str],
    fixture: dict[str, str],
    full_key: str,
) -> str:
    try:
        objects = list_exact_key(aws, env, fixture, full_key)
        delete_exact_versions(aws, env, fixture, objects)
        remaining = list_exact_key(aws, env, fixture, full_key)
        if remaining:
            return f"failed:remaining_{len(remaining)}"
        return f"complete exact_versions_and_markers_deleted={len(objects)}"
    except StageError as error:
        return f"failed:{error.stage}:{error.code}"


def source_identity(repo: Path, evidence_dir: Path) -> tuple[str, str, bool]:
    proc = run_capture(["git", "rev-parse", "HEAD", "HEAD^{tree}"], cwd=repo)
    if proc.returncode != 0:
        raise StageError("source", "identity_failed")
    head, tree = proc.stdout.strip().splitlines()
    status = run_capture(["git", "status", "--porcelain"], cwd=repo)
    if status.returncode != 0:
        raise StageError("source", "status_failed")
    clean = not status.stdout.strip()
    write_text(evidence_dir / "source.head-tree", f"{head}\n{tree}")
    write_text(evidence_dir / "source.status", "clean" if clean else "dirty")
    return head, tree, clean


def write_manifest(evidence_dir: Path) -> None:
    entries = []
    for path in sorted(evidence_dir.iterdir()):
        if path.is_file() and path.name != "manifest.sha256":
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            entries.append(f"{digest}  {path.name}")
    write_text(evidence_dir / "manifest.sha256", "\n".join(entries))


def main() -> int:
    repo = Path(__file__).resolve().parents[1]
    runner_temp_value = os.environ.get("RUNNER_TEMP")
    if not runner_temp_value:
        print("TASK_driver_status=failed")
        print("TASK_failure_stage=environment")
        print("TASK_failure_code=runner_temp_missing")
        return 2

    runner_temp = Path(runner_temp_value)
    evidence_dir = Path(
        os.environ.get(
            "OPENDAL_TASK_EVIDENCE_DIR",
            str(runner_temp / "opendal-s3-container-provider-evidence"),
        )
    )
    try:
        evidence_dir.mkdir(parents=True, exist_ok=True)
    except OSError:
        print("TASK_driver_status=failed")
        print("TASK_failure_stage=environment")
        print("TASK_failure_code=evidence_dir_create_failed")
        return 2

    write_text(evidence_dir / "native.exit", "not_run")
    write_text(evidence_dir / "native.log", "native.stage=not_run")
    write_text(evidence_dir / "native.cases", "0")
    write_text(evidence_dir / "source.head-tree", "unknown\nunknown")
    write_text(evidence_dir / "source.status", "unknown")
    write_text(evidence_dir / "cleanup.status", "not_required")
    write_text(evidence_dir / "private-cleanup.status", "not_started")
    write_text(evidence_dir / "driver.status", "running")

    private_dir = runner_temp / f"opendal-s3-container-provider-private-{uuid.uuid4()}"
    try:
        private_dir.mkdir(mode=0o700)
    except OSError:
        write_text(evidence_dir / "driver.status", "failed:environment:private_dir")
        print("TASK_driver_status=failed")
        print("TASK_failure_stage=environment")
        print("TASK_failure_code=private_dir_create_failed")
        return 2

    native_exit = "not_run"
    native_footer = "absent"
    native_cases = 0
    cleanup_status = "not_required"
    seed_attempted = False
    driver_failure: StageError | None = None
    relative_key = ""
    head = "unknown"
    tree = "unknown"

    try:
        head, tree, clean = source_identity(repo, evidence_dir)
        if not clean:
            raise StageError("source", "dirty")

        fixture = fixture_configuration()
        github_mask(fixture["access_key_id"])
        github_mask(fixture["secret_access_key"])

        aws = shutil.which("aws")
        if aws is None:
            raise StageError("aws", "cli_missing")
        aws_env = aws_environment(fixture)

        minted = mint_temporary_credentials(aws, aws_env)
        metadata_response_file = private_dir / "metadata-response.json"
        token_file = private_dir / "metadata-token"
        body_file = private_dir / "expected-body"
        write_private_file(
            metadata_response_file,
            json.dumps(
                {
                    "AccessKeyId": minted["AccessKeyId"],
                    "SecretAccessKey": minted["SecretAccessKey"],
                    "Token": minted["SessionToken"],
                    "Expiration": minted["Expiration"],
                },
                separators=(",", ":"),
            ).encode("utf-8"),
        )
        write_private_file(body_file, EXPECTED_BODY)

        relative_key = f"{RELATIVE_PREFIX}/{uuid.uuid4()}.bin"
        normalized_root = normalize_root(fixture["root"])
        full_key = f"{normalized_root}/{relative_key}" if normalized_root else relative_key
        write_text(evidence_dir / "relative-key.txt", relative_key)

        if list_exact_key(aws, aws_env, fixture, full_key):
            raise StageError("cleanup_preflight", "existing_exact_key")
        write_text(evidence_dir / "cleanup.preflight", "complete")

        seed_attempted = True
        seed_object(aws, aws_env, fixture, full_key, body_file)

        native_env = native_environment(fixture, metadata_response_file, token_file, relative_key)
        native_code, native_footer = run_native(
            repo,
            native_env,
            evidence_dir / "native.log",
            evidence_dir / "native.exit",
        )
        native_exit = str(native_code)

        native_log = (evidence_dir / "native.log").read_text(encoding="utf-8")
        match = re.search(r"test result:.*?(\d+) passed; (\d+) failed", native_log)
        if match:
            native_cases = int(match.group(1)) + int(match.group(2))
            write_text(evidence_dir / "native.cases", str(native_cases))
        if native_code != 0:
            raise StageError("native", f"exit_{native_code}")
        if native_footer == "absent":
            raise StageError("native", "footer_absent")
        if native_cases != 1:
            raise StageError("native", f"case_count_{native_cases}")
        if "TASK_case_passed=true" not in native_log:
            raise StageError("native", "case_not_passed")
    except StageError as error:
        driver_failure = error
    except Exception as error:
        driver_failure = StageError("driver", type(error).__name__)
    finally:
        if seed_attempted:
            try:
                fixtures = fixture_configuration()
                aws = shutil.which("aws")
                if aws is None:
                    cleanup_status = "failed:aws:cli_missing"
                else:
                    normalized_root = normalize_root(fixtures["root"])
                    full_key = (
                        f"{normalized_root}/{relative_key}" if normalized_root else relative_key
                    )
                    cleanup_status = cleanup_exact_object(
                        aws,
                        aws_environment(fixtures),
                        fixtures,
                        full_key,
                    )
            except StageError as error:
                cleanup_status = f"failed:{error.stage}:{error.code}"
            except Exception as error:
                cleanup_status = f"failed:driver:{type(error).__name__}"
        write_text(evidence_dir / "cleanup.status", cleanup_status)

        try:
            shutil.rmtree(private_dir)
            private_cleanup = "complete"
        except OSError:
            private_cleanup = "failed"
        write_text(evidence_dir / "private-cleanup.status", private_cleanup)

    driver_ok = (
        driver_failure is None
        and native_exit == "0"
        and native_cases == 1
        and native_footer.startswith("test result: ok.")
        and cleanup_status.startswith("complete")
        and private_cleanup == "complete"
    )

    if driver_ok:
        write_text(evidence_dir / "driver.status", "success")
        print("TASK_native_exit=0")
        print(f"TASK_native_cases={native_cases}")
        print(f"TASK_cleanup_status={cleanup_status}")
        print(f"TASK_source_head={head}")
        print(f"TASK_source_tree={tree}")
        print(f"TASK_native_footer={native_footer}")
        print("TASK_driver_status=success")
        write_manifest(evidence_dir)
        return 0

    failure_stage = driver_failure.stage if driver_failure else "native_or_cleanup"
    failure_code = driver_failure.code if driver_failure else "failed"
    write_text(evidence_dir / "driver.status", f"failed:{failure_stage}:{failure_code}")
    print(f"TASK_native_exit={native_exit}")
    print(f"TASK_native_cases={native_cases}")
    print(f"TASK_cleanup_status={cleanup_status}")
    print(f"TASK_source_head={head}")
    print(f"TASK_source_tree={tree}")
    print(f"TASK_native_footer={native_footer}")
    print(f"TASK_failure_stage={failure_stage}")
    print(f"TASK_failure_code={failure_code}")
    print("TASK_driver_status=failed")
    write_manifest(evidence_dir)
    return 1


if __name__ == "__main__":
    sys.exit(main())
