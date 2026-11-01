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

set -euo pipefail

# Release script for OpenDAL OCaml bindings to opam

VERSION=${1:-""}
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.48.0"
    exit 1
fi

echo "🚀 Releasing OpenDAL OCaml bindings version $VERSION"

# Step 1: Update version in dune-project
echo "📝 Updating version in dune-project..."
sed -i "s/(version [^)]*)/(version $VERSION)/" dune-project

# Step 2: Regenerate opam file
echo "🔄 Regenerating opam file..."
dune build

# Step 3: Run tests
echo "🧪 Running tests..."
dune runtest

# Step 4: Build documentation
echo "📚 Building documentation..."
dune build @doc

# Step 5: Create a source distribution
echo "📦 Creating source distribution..."
dune-release tag v$VERSION
dune-release distrib

# Step 6: Publish to opam-repository (requires manual PR)
echo "🌐 Creating opam-repository PR..."
echo "Run the following command to submit to opam-repository:"
echo "dune-release publish distrib --verbose"
echo ""
echo "Or manually create a PR to opam-repository with the generated opam file:"
echo "https://github.com/ocaml/opam-repository"

echo "✅ Release preparation complete!"
echo ""
echo "Next steps:"
echo "1. Review the generated tarball in _build/"
echo "2. Test installation: opam install ./"
echo "3. Submit PR to opam-repository"
echo "4. Tag the release in git: git tag v$VERSION && git push origin v$VERSION" 