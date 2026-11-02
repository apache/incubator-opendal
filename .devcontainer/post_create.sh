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

set -e

# Make sure trustdb has been created.
sudo gpg --list-keys || true

# Update apt repo
sudo apt update

# Setup for ruby binding
sudo apt install -y ruby-dev libclang-dev

# Setup for python binding
sudo apt install -y python3-dev python3-pip python3-venv

# Setup for nodejs binding
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash - && sudo apt-get install -y nodejs
sudo corepack enable
corepack prepare pnpm@latest --activate

# Setup for java binding
sudo apt install -y default-jdk
echo "export JAVA_HOME=$(readlink -f /usr/bin/java | sed "s:/bin/java::")" | sudo tee /etc/profile.d/java_home.sh
sudo ln -s /usr/lib/jvm/default-java /usr/lib/jvm/default || true

# Setup for C binding
sudo apt install -y libgtest-dev cmake

# Setup for Zig binding
sudo apt install -y wget
tmp=$(mktemp -d)
wget -q https://github.com/marler8997/zigup/releases/download/v2023_07_27/zigup.ubuntu-latest-x86_64.zip -O $tmp/zigup.ubuntu-latest-x86_64.zip
sudo unzip -o $tmp/zigup.ubuntu-latest-x86_64.zip -d /usr/bin
sudo chmod +x /usr/bin/zigup
sudo zigup 0.11.0

# Setup for Haskell binding
sudo apt install -y ghc cabal-install
cabal update

# Setup for PHP binding
sudo apt install -y software-properties-common
echo "deb https://packages.sury.org/php/ $(lsb_release -sc) main" | sudo tee /etc/apt/sources.list.d/sury-php.list
wget -qO - https://packages.sury.org/php/apt.gpg | sudo apt-key add -
sudo apt update -y
sudo apt install -y php8.2 php8.2-dev

# Setup for OCaml binding
sudo apt install -y opam
opam init --auto-setup --yes --disable-sandboxing # container can't use sandboxing
opam install -y dune ounit2 ocamlformat

# Setup for Cpp binding
sudo apt install -y ninja-build

# Setup for D binding
sudo apt install -y dmd dub

# Setup just for development tasks
cargo install just

# Setup uv for python project management
curl -LsSf https://astral.sh/uv/install.sh | sh
