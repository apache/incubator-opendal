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

module opendal_example_guide

go 1.26.0

require (
	github.com/apache/opendal-go-services/memory v0.1.16
	github.com/apache/opendal/bindings/go v0.1.16
)

require (
	github.com/ebitengine/purego v0.10.1 // indirect
	github.com/jupiterrider/ffi v0.7.0 // indirect
	github.com/klauspost/compress v1.17.9 // indirect
	golang.org/x/sys v0.48.0 // indirect
)

// Build the example against the binding in this repository rather than a
// published release, so a breaking change in the binding fails CI here.
replace github.com/apache/opendal/bindings/go => ../..
