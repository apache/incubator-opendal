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

memcached_tls_dir="${1:?a new fixture directory is required}"
memcached_fixture_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
test -d "$memcached_tls_dir"
test ! -e "$memcached_tls_dir/server"
if [ -n "$(docker ps -aq --filter label=com.docker.compose.project=opendal-memcached-tls)" ]; then
    echo "Memcached TLS fixture already exists" >&2
    exit 2
fi
chmod 700 "$memcached_tls_dir"
umask 077
mkdir "$memcached_tls_dir/server"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj '/CN=OpenDAL Memcached test CA' -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' -keyout "$memcached_tls_dir/ca.key" -out "$memcached_tls_dir/ca.pem"
openssl req -newkey rsa:2048 -nodes -subj '/CN=127.0.0.1' -keyout "$memcached_tls_dir/server/server.key" -out "$memcached_tls_dir/server/server.csr"
printf '%s\n' 'subjectAltName=IP:127.0.0.1' 'extendedKeyUsage=serverAuth' 'basicConstraints=CA:FALSE' > "$memcached_tls_dir/server.ext"
openssl x509 -req -days 1 -in "$memcached_tls_dir/server/server.csr" -CA "$memcached_tls_dir/ca.pem" -CAkey "$memcached_tls_dir/ca.key" -CAcreateserial -extfile "$memcached_tls_dir/server.ext" -out "$memcached_tls_dir/server/server.pem"
rm "$memcached_tls_dir/ca.key"
chmod 755 "$memcached_tls_dir/server"
chmod 644 "$memcached_tls_dir/server/server.pem" "$memcached_tls_dir/server/server.key"
# Preserve system roots for other TLS clients, including Cargo in the test step.
cat /etc/ssl/certs/ca-certificates.crt "$memcached_tls_dir/ca.pem" > "$memcached_tls_dir/roots.pem"
export MEMCACHED_TLS_CERT_DIR="$memcached_tls_dir/server"
docker compose -p opendal-memcached-tls -f "$memcached_fixture_dir/docker-compose-tls.yml" up -d --wait
for attempt in {1..20}; do
    if timeout 3 openssl s_client -brief -connect 127.0.0.1:11212 -verify_ip 127.0.0.1 -verify_return_error -CAfile "$memcached_tls_dir/ca.pem" < /dev/null; then
        exit 0
    fi
    sleep 0.25
done
exit 1
