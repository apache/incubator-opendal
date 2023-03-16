#!/bin/bash
#
# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements. See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License. You may obtain a copy of the License at
# http://www.apache.org/licenses/LICENSE-2.0
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

set -e

if [ -z ${OPENDAL_VERSION} ]; then
    echo "OPENDAL_VERSION is unset";
    exit 1
else
    echo "var is set to '$OPENDAL_VERSION'";
fi

# tar source code
release_version=${OPENDAL_VERSION}
# Corresponding git repository branch
git_branch=release-${OPENDAL_VERSION}-rc1

rm -rf dist
mkdir -p dist/

echo "> Start package"
git archive --format=tar.gz --output="dist/apache-incubator-opendal-$release_version-src.tar.gz" --prefix="apache-incubator-opendal-$release_version-src/"  $git_branch

echo "> Generate signature"
for i in dist/*.tar.gz; do echo $i; gpg --armor --output $i.asc --detach-sig $i ; done
echo "> Check signature"
for i in *.tar.gz; do echo $i; gpg --verify $i.asc $i ; done
echo "> Generate sha512sum"
for i in dist/*.tar.gz; do echo $i; sha512sum $i > $i.sha512 ; done
echo "> Check sha512sum"
for i in dist/*.tar.gz; do echo $i; sha512sum --check $i.sha512; done
echo "> Check license"
docker run -it --rm -v $(pwd):/github/workspace -u $(id -u):$(id -g) ghcr.io/korandoru/hawkeye-native check
