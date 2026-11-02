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
task_repo=$(git rev-parse --show-toplevel)
task_log=${1:?absolute evidence directory required}
mkdir -p "$task_log"
task_log=$(cd "$task_log" && pwd)
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
export CARGO_BUILD_JOBS=4
export RUST_LOG=off
export CARGO_TERM_COLOR=never
cd "$task_repo"
git rev-parse HEAD HEAD^{tree} > "$task_log/source.txt"
git status --porcelain > "$task_log/source-status.txt"
test ! -s "$task_log/source-status.txt"
rustc --version > "$task_log/rustc.txt"
cargo --version > "$task_log/cargo.txt"

run() {
    local task_name=$1
    shift
    local task_exit=0
    "$@" > "$task_log/$task_name.log" 2>&1 || task_exit=$?
    printf '%s\n' "$task_exit" > "$task_log/$task_name.exit"
    return "$task_exit"
}

finish() {
    local task_exit=$?
    git -C "$task_repo" rev-parse HEAD HEAD^{tree} > "$task_log/post-source.txt"
    git -C "$task_repo" status --porcelain > "$task_log/post-status.txt"
    printf '%s\n' "$task_exit" > "$task_log/driver.exit"
    (cd "$task_log" && sha256sum ./*.txt ./*.log ./*.exit > SHA256SUMS)
}
trap finish EXIT

cd "$task_repo/core"
run unit cargo test --locked -p opendal-service-git --lib -- --nocapture
run doc cargo test --locked -p opendal-service-git --doc
run service-clippy cargo clippy --locked -p opendal-service-git --all-targets -- -D warnings
run facade-clippy cargo clippy --locked -p opendal --features services-git --all-targets -- -D warnings
run live timeout 900 cargo test --locked -p opendal --features services-git --test task_git_service_live -- --nocapture
run fmt cargo fmt --all -- --check
cd "$task_repo"
run diff-check git diff --check
run tracked-clean git diff --exit-code HEAD
