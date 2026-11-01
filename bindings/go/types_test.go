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

package opendal

import "testing"

func TestToOpendalBytesUsesNullDataForEmptyInput(t *testing.T) {
	got := toOpendalBytes(make([]byte, 0, 8))
	if got.data != nil || got.len != 0 || got.capacity != 0 {
		t.Fatalf("toOpendalBytes(empty) = %+v, want zero value", got)
	}
}
