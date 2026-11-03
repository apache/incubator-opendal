#!/usr/bin/env bash
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

set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
base_head="${BASE_HEAD:?original worktree HEAD required}"
product_head="${PRODUCT_HEAD:?exact accepted product required}"
log_dir="${LOG_DIR:?new log directory required}"
project=opendal-pr8350-main263
compose_file="$repo_root/fixtures/smb/docker-compose.yml"
image=ghcr.io/servercontainers/samba@sha256:ac7c406702a3bf4137fcd04ce031d87adbda1b819f698fe89a89b5ee46618dfb
started=0
export PATH="/home/xuanwo/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS=4
test ! -e "$log_dir"
mkdir -p "$log_dir"
cd "$repo_root"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
finish() {
    local result=$? cleanup_status
    trap - EXIT
    set +e
    if [ "$started" -eq 1 ]; then
        docker compose -p "$project" -f "$compose_file" logs --no-color > "$log_dir/fixture.log" 2>&1
        docker compose -p "$project" -f "$compose_file" down -v > "$log_dir/fixture-down.log" 2>&1
        cleanup_status=$?
        printf '%s\n' "$cleanup_status" > "$log_dir/fixture-down.status"
        if [ "$cleanup_status" -ne 0 ]; then result=2; fi
        docker container ls -aq --filter "label=com.docker.compose.project=$project" > "$log_dir/remaining-container.txt" 2>&1
        if [ "$?" -ne 0 ] || [ -s "$log_dir/remaining-container.txt" ]; then result=2; fi
        docker network ls -q --filter "label=com.docker.compose.project=$project" > "$log_dir/remaining-network.txt" 2>&1
        if [ "$?" -ne 0 ] || [ -s "$log_dir/remaining-network.txt" ]; then result=2; fi
        docker volume ls -q --filter "label=com.docker.compose.project=$project" > "$log_dir/remaining-volume.txt" 2>&1
        if [ "$?" -ne 0 ] || [ -s "$log_dir/remaining-volume.txt" ]; then result=2; fi
    fi
    git switch --detach "$base_head" > "$log_dir/restore.log" 2>&1
    cleanup_status=$?
    printf '%s\n' "$cleanup_status" > "$log_dir/restore.status"
    git rev-parse HEAD HEAD^{tree} > "$log_dir/restored-head.txt"
    git status --porcelain --untracked-files=all > "$log_dir/restored-status.txt"
    if [ "$cleanup_status" -ne 0 ] || [ -s "$log_dir/restored-status.txt" ]; then result=2; fi
    if ! ss -H -ltn 'sport = :1445' > "$log_dir/remaining-port.txt"; then result=2; fi
    if [ -s "$log_dir/remaining-port.txt" ]; then result=2; fi
    printf '%s\n' "$result" > "$log_dir/driver.exit"
    exit "$result"
}
run_stage() {
    local stage="$1"
    shift
    set +e
    "$@" > "$log_dir/$stage.log" 2>&1
    local stage_status=$?
    set -e
    printf '%s\n' "$stage_status" > "$log_dir/$stage.status"
    test "$stage_status" -eq 0
}
git rev-parse HEAD HEAD^{tree} > "$log_dir/carrier-head.txt"
test "$(git rev-parse HEAD^)" = "$product_head"
git diff --name-only "$product_head" HEAD > "$log_dir/task-files.txt"
test "$(cat "$log_dir/task-files.txt")" = dev/task_pr8350_port_validate.sh
sha256sum dev/task_pr8350_port_validate.sh > "$log_dir/driver.sha256"
git status --porcelain --untracked-files=all > "$log_dir/pre-status.txt"
test ! -s "$log_dir/pre-status.txt"
test -z "$(ss -H -ltn 'sport = :1445')"
test -z "$(docker ps -aq --filter "label=com.docker.compose.project=$project")"
trap finish EXIT
git switch --detach "$product_head" > "$log_dir/product-checkout.log" 2>&1
git rev-parse HEAD HEAD^{tree} > "$log_dir/source-head.txt"
cargo --version > "$log_dir/toolchain.txt"
rustc --version >> "$log_dir/toolchain.txt"
docker image inspect "$image" --format '{{json .RepoDigests}}' > "$log_dir/image-digest.txt"
test -z "$(ss -H -ltn 'sport = :1445')"
test -z "$(docker ps -aq --filter "label=com.docker.compose.project=$project")"
run_stage taplo taplo format --check
cd core
run_stage unit cargo test --locked -p opendal-service-smb --lib
rg -q 'test result: ok\. [1-9][0-9]* passed' "$log_dir/unit.log"
run_stage clippy cargo clippy --locked -p opendal-service-smb --all-targets --all-features -- -D warnings
run_stage facade-clippy cargo clippy --locked -p opendal --all-targets --features services-smb -- -D warnings
run_stage docs cargo test --locked -p opendal-service-smb --doc
run_stage fmt cargo fmt --all -- --check
run_stage msrv-package cargo +1.91.1 check --locked -p opendal-service-smb --all-targets
run_stage msrv-pair cargo +1.91.1 check --locked -p opendal --all-targets --features services-git,services-smb
run_stage behavior-compile cargo test --locked --features tests,services-smb --test behavior --no-run
started=1
run_stage fixture-setup docker compose -p "$project" -f "$compose_file" up -d --wait --pull never
export OPENDAL_TEST=smb
export OPENDAL_SMB_ENDPOINT=127.0.0.1:1445
export OPENDAL_SMB_SHARE=data
export OPENDAL_SMB_ROOT=/opendal/
export OPENDAL_SMB_USER=opendal
export OPENDAL_SMB_PASSWORD=opendal
run_stage samba-service cargo test --locked -p opendal-service-smb --test samba -- --ignored --nocapture
rg -q 'test result: ok\. 9 passed; 0 failed' "$log_dir/samba-service.log"
run_stage canonical cargo test --locked --features tests,services-smb --test behavior -- behavior --nocapture
rg -q 'test result: ok\. [1-9][0-9]* passed; 0 failed' "$log_dir/canonical.log"
cd "$repo_root"
run_stage tracked-clean git diff --exit-code
git status --porcelain --untracked-files=all > "$log_dir/product-final-status.txt"
test ! -s "$log_dir/product-final-status.txt"
