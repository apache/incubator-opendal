#!/bin/bash
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

# ---------------------------------------------------------------------------
# Default GooseFS launcher for OpenDAL CI / local dev.
#
# The stock GooseFS image entrypoint (`goosefs-start.sh local`) starts
# *five* JVMs: master, worker,
# job_master, job_worker and table_master. On the GitHub Actions
# ubuntu-latest runner, the three processes OpenDAL does NOT use
# (job_master + job_worker + table_master) still consume ~1.1 GiB RSS
# and an extra ~10-15s of start-up CPU. During the caller's 3-4 minute
# `cargo build` / `dotnet test-compile` window these extra JVMs can
# starve the worker of CPU, which has been empirically correlated with
# the `tcp connect error: Connection refused (os error 111)` failures
# we see on `127.0.0.1:9203` at data-plane write time.
#
# OpenDAL's goosefs service (see `core/services/goosefs/src/core.rs`)
# only talks to:
#   * master RPC on 9200  -- metadata (mkdir / ls / stat / rm)
#   * worker RPC on 9203  -- block I/O
#
# job_master / job_worker drive distributed copy / async persist / cross
# mount, and table_master backs Hive-style catalogs. None of those paths
# are exercised by OpenDAL. Dropping them halves the fixture's memory
# footprint (observed: 978 MiB vs. ~1.9 GiB for `local`) while keeping
# the externally visible behaviour identical.
#
# The script is structured so it can safely replace the image's default
# entrypoint (`docker-entrypoint.sh local`):
#
#   1. Apply the same `goosefs-site.properties` overrides that the image
#      entrypoint applies (bind hosts, worker/master hostnames).
#   2. Create the runtime directories the image expects to exist.
#   3. Format the journal on first boot (journal dir empty) -- exactly
#      what `docker-entrypoint.sh format_if_needed` does.
#   4. Launch `goosefs-start.sh -aN master` and
#      `goosefs-start.sh -aN worker NoMount` (`-a` returns asynchronously;
#      `-N` skips kill-before-start. Each component is a detached daemon
#      writing to /opt/goosefs/logs/).
#   5. `exec tail -F` on the two log files so the container stays alive
#      and `docker logs` shows the running state. When either JVM dies,
#      its log tail surfaces in `docker logs` and the compose healthcheck
#      (a real data-plane probe) will fail.
#
# Re-exec-safe: every step tolerates already-existing state, so the
# script can be restarted (e.g. by compose's `restart: on-failure`).
# ---------------------------------------------------------------------------

set -euo pipefail

# Force the JVM (master/worker/goosefs CLI) to resolve `localhost` to
# 127.0.0.1 instead of ::1. On Ubuntu 24.04 GitHub Actions runners the
# default JDK resolves `localhost` to ::1 first, so the master binds
# 9200/9212/9202 on ::1 while the worker / `fs` CLI dials 127.0.0.1 ->
# `Connection refused`, looping on `Failed to connect to ... :9212`
# until the compose healthcheck times out. Pinning the stack to IPv4
# keeps bind and dial on the same family without touching the host
# kernel.
export GOOSEFS_JAVA_OPTS="${GOOSEFS_JAVA_OPTS:-} -Djava.net.preferIPv4Stack=true"

: "${GOOSEFS_HOME:=/opt/goosefs}"
# Use the IPv4 literal `127.0.0.1` (not the name `localhost`) for the
# master hostname on purpose. The master's *client-facing* RPC port
# 9200 is bound directly to whatever `goosefs.master.hostname` resolves
# to (there is no separate `goosefs.master.client.rpc.bind.host` in
# GooseFS 2.x). On Ubuntu 24.04 GitHub Actions runners and many Linux
# images, `/etc/hosts` resolves `localhost` to BOTH `::1` and
# `127.0.0.1`, and the JVM's default resolver may hand back `::1`
# first — even with `-Djava.net.preferIPv4Stack=true`, because that
# flag only suppresses *creating* IPv6 sockets, it does not change
# `InetAddress.getByName("localhost")`'s ordering when the name maps
# to both families. Result: master binds 9200 on `[::1]`, while the
# in-container worker (and the `fs` CLI used by the healthcheck) dials
# `127.0.0.1:9200` over IPv4 and gets `Connection refused` in a tight
# retry loop, never registers, and the data-plane healthcheck times
# out. Pinning the hostname to the v4 literal forces bind+dial onto
# the same loopback address and matches what `GOOSEFS_WORKER_HOSTNAME`
# already does for the worker side.
: "${GOOSEFS_MASTER_HOSTNAME:=127.0.0.1}"
# OpenDAL writes to the worker via `127.0.0.1:9203` from the host; see
# the long comment in docker-compose-goosefs.yml for why this must be
# the IPv4 literal and not the `localhost` name.
: "${GOOSEFS_WORKER_HOSTNAME:=127.0.0.1}"

SITE_FILE="${GOOSEFS_HOME}/conf/goosefs-site.properties"

# Insert-or-replace a key=value line in goosefs-site.properties. Mirrors
# `upsert_prop` from the upstream image's docker-entrypoint.sh.
upsert_prop() {
  local key="$1"
  local value="$2"
  # The `${key//./\\.}` part escapes the dots so the regex matches a
  # literal property key (e.g. `goosefs.master.hostname`) rather than
  # treating the dots as "any char".
  if grep -qE "^[#[:space:]]*${key//./\\.}[[:space:]]*=" "${SITE_FILE}"; then
    sed -i -E "s|^[#[:space:]]*${key//./\\.}[[:space:]]*=.*|${key}=${value}|" "${SITE_FILE}"
  else
    echo "${key}=${value}" >> "${SITE_FILE}"
  fi
}

apply_runtime_config() {
  upsert_prop "goosefs.master.hostname"        "${GOOSEFS_MASTER_HOSTNAME}"
  upsert_prop "goosefs.worker.hostname"        "${GOOSEFS_WORKER_HOSTNAME}"
  # Bind on 0.0.0.0 so the container's published ports are reachable
  # from the host namespace. Without this the JVM binds on the
  # container hostname only and `127.0.0.1:9203` on the host drops
  # SYNs.
  upsert_prop "goosefs.master.bind.host"       "0.0.0.0"
  upsert_prop "goosefs.master.web.bind.host"   "0.0.0.0"
  upsert_prop "goosefs.worker.bind.host"       "0.0.0.0"
  upsert_prop "goosefs.worker.web.bind.host"   "0.0.0.0"
  # The master also runs two *internal* RPC endpoints that default to
  # binding the resolved hostname only:
  #   * 9212 -- master <-> worker RPC (block heartbeat, register)
  #   * 9202 -- embedded Raft journal
  # Without these the worker logs `Failed to connect to ... :9212 ...
  # Connection refused` in a tight retry loop and never registers,
  # which makes the data-plane healthcheck (`fs copyFromLocal`) hang
  # the entire compose `--wait`. Bind both on 0.0.0.0 so loopback
  # dials from inside the container always succeed.
  upsert_prop "goosefs.master.rpc.bind.host"             "0.0.0.0"
  upsert_prop "goosefs.master.embedded.journal.bind.host" "0.0.0.0"
}

ensure_dirs() {
  local mono_dir page_dir ufs_dir
  mono_dir=$(grep -E '^goosefs\.worker\.file\.store\.dirs\.path' "${SITE_FILE}" 2>/dev/null | head -1 | cut -d'=' -f2- || true)
  page_dir=$(grep -E '^goosefs\.worker\.page\.store\.dirs'       "${SITE_FILE}" 2>/dev/null | grep -v 'page\.size' | head -1 | cut -d'=' -f2- || true)
  ufs_dir=$( grep -E '^goosefs\.master\.mount\.table\.root\.ufs' "${SITE_FILE}" 2>/dev/null | head -1 | cut -d'=' -f2- || true)

  [[ -n "${mono_dir}" ]] && mkdir -p "${mono_dir}" || true
  [[ -n "${page_dir}" ]] && mkdir -p "${page_dir}" || true
  # Only auto-create the UFS root when it looks like a local absolute
  # path. `cosn://` / `s3://` / etc. are handled by GooseFS itself.
  if [[ -n "${ufs_dir}" && "${ufs_dir}" =~ ^/ ]]; then
    mkdir -p "${ufs_dir}" || true
  fi
  mkdir -p "${GOOSEFS_HOME}/logs" "${GOOSEFS_HOME}/journal"
}

format_if_needed() {
  # Only format on first boot; re-formatting would drop all UFS metadata
  # and break named-volume persistence between `docker compose down`
  # (without -v) and `up` cycles.
  if [[ -z "$(ls -A "${GOOSEFS_HOME}/journal" 2>/dev/null || true)" ]]; then
    echo "[start-default] empty journal -> formatMaster/formatWorker ..."
    "${GOOSEFS_HOME}/bin/goosefs" formatMaster || true
    "${GOOSEFS_HOME}/bin/goosefs" formatWorker || true
  fi
}

# Some GooseFS images (Alpine + musl) ship a glibc jemalloc and
# goosefs-config.sh LD_PRELOADs it. That makes musl tools (`ps`,
# `hostname`, `nohup`) abort with "Error relocating ... symbol not
# found", so `goosefs-start.sh` dies at the pre-start stop step.
# Java (glibc) does not need the preload. Probe with a musl binary
# and only disable the .so when the preload is actually incompatible,
# so glibc-based images keep jemalloc.
disable_incompatible_jemalloc() {
  local jemalloc="${GOOSEFS_HOME}/lib/native/libjemalloc.so"
  [[ -f "${jemalloc}" ]] || return 0
  if LD_PRELOAD="${jemalloc}" /bin/true >/dev/null 2>&1; then
    return 0
  fi
  if mv "${jemalloc}" "${jemalloc}.disabled" 2>/dev/null; then
    echo "[start-default] disabled libjemalloc.so (LD_PRELOAD incompatible with this libc)"
  else
    echo "[start-default] WARN: libjemalloc.so is incompatible with this libc but could not be renamed" >&2
  fi
  unset LD_PRELOAD || true
}

main() {
  echo "[start-default] applying runtime config ..."
  apply_runtime_config
  ensure_dirs
  disable_incompatible_jemalloc
  format_if_needed

  # goosefs-start.sh parses flags with getopts *before* the ACTION
  # positional arg, so `-aN master` is the correct order. `-a` starts
  # the daemon and returns; `-N` skips kill-before-start (Alpine
  # BusyBox `ps` does not support the GNU `ps -Aww` that `killAll`
  # uses, so the pre-start stop is a no-op or a hard failure).
  # `worker NoMount` tells the worker not to try to mount a RamFS
  # (the image doesn't ship with `sudo`); the worker uses /dev/shm
  # directly instead.
  echo "[start-default] launching master ..."
  "${GOOSEFS_HOME}/bin/goosefs-start.sh" -aN master
  echo "[start-default] launching worker ..."
  "${GOOSEFS_HOME}/bin/goosefs-start.sh" -aN worker NoMount

  # Give both JVMs a moment to create their log files so `tail -F`
  # doesn't print a confusing "No such file" warning. `tail -F`
  # (capital-F) handles file rotation / creation gracefully anyway,
  # this just keeps the output tidy.
  local tries=0
  while [[ ! -f "${GOOSEFS_HOME}/logs/master.log" && ${tries} -lt 30 ]]; do
    sleep 1
    tries=$((tries + 1))
  done

  echo "[start-default] GooseFS master+worker started; tailing logs ..."
  exec tail -F \
    "${GOOSEFS_HOME}/logs/master.log" \
    "${GOOSEFS_HOME}/logs/worker.log" \
    2>/dev/null
}

main "$@"
