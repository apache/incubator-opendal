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

# Multi-arch Swift All-In-One (SAIO) image
#
# Based on upstream: https://opendev.org/openstack/swift/src/branch/master/Dockerfile
# Modified to support both amd64 and arm64 (Apple Silicon).
#
# Build for current platform:
#   podman build -t opendal/swift-saio .
#
# Build multi-arch:
#   docker buildx build --platform linux/amd64,linux/arm64 -t opendal/swift-saio .

FROM alpine:3.16.2

# TARGETARCH is automatically set by buildx/podman (amd64 or arm64).
ARG TARGETARCH

ENV S6_LOGGING=1
ENV S6_VERSION=1.21.4.0
ENV SOCKLOG_VERSION=3.0.1-1
ENV BUILD_DIR="/tmp"
ENV ENV="/etc/profile"

# Map Docker arch names to s6-overlay arch names:
#   amd64  -> amd64
#   arm64  -> aarch64
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        echo "aarch64" > /tmp/s6_arch; \
    else \
        echo "$TARGETARCH" > /tmp/s6_arch; \
    fi

# Clone Swift source
RUN apk add --no-cache git && \
    git clone --depth 1 https://opendev.org/openstack/swift.git /opt/swift && \
    apk del git

# Download s6-overlay and socklog-overlay for the target architecture
RUN S6_ARCH=$(cat /tmp/s6_arch) && \
    wget -q -O /tmp/s6-overlay.tar.gz \
        "https://github.com/just-containers/s6-overlay/releases/download/v${S6_VERSION}/s6-overlay-${S6_ARCH}.tar.gz" && \
    wget -q -O /tmp/s6-overlay.tar.gz.sig \
        "https://github.com/just-containers/s6-overlay/releases/download/v${S6_VERSION}/s6-overlay-${S6_ARCH}.tar.gz.sig" && \
    wget -q -O /tmp/socklog-overlay.tar.gz \
        "https://github.com/just-containers/socklog-overlay/releases/download/v${SOCKLOG_VERSION}/socklog-overlay-${S6_ARCH}.tar.gz"

RUN mkdir /etc/swift && \
    echo "================   starting swift_needs  ===================" && \
    /opt/swift/docker/install_scripts/00_swift_needs.sh && \
    echo "================   starting apk_install_prereqs  ===================" && \
    /opt/swift/docker/install_scripts/10_apk_install_prereqs.sh && \
    echo "================   starting apk_install_py3  ===================" && \
    /opt/swift/docker/install_scripts/21_apk_install_py3.sh && \
    echo "================   starting swift_install  ===================" && \
    /opt/swift/docker/install_scripts/50_swift_install.sh && \
    echo "================   installing s6-overlay  ===================" && \
    gpg --import /opt/swift/docker/s6-gpg-pub-key && \
    gpg --verify /tmp/s6-overlay.tar.gz.sig /tmp/s6-overlay.tar.gz && \
    gunzip -c /tmp/s6-overlay.tar.gz | tar -xf - -C / && \
    gunzip -c /tmp/socklog-overlay.tar.gz | tar -xf - -C / && \
    rm -rf /tmp/s6-overlay* /tmp/socklog-overlay* /tmp/s6_arch && \
    echo "================   starting pip_uninstall_dev  ===================" && \
    /opt/swift/docker/install_scripts/60_pip_uninstall_dev.sh && \
    echo "================   starting apk_uninstall_dev  ===================" && \
    /opt/swift/docker/install_scripts/99_apk_uninstall_dev.sh

# The upstream Dockerfile uses "COPY docker/rootfs /" since it builds
# from the Swift source tree. We cloned into /opt/swift, so copy from there.
RUN cp -a /opt/swift/docker/rootfs/* /

ENTRYPOINT ["/init"]
