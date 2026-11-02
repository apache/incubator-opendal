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

package opendal_test

import (
	"bytes"
	"io"
	"net/http"
	"strconv"
	"time"

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/stretchr/testify/require"
)

func testsPresign(cap *opendal.Capability) []behaviorTest {
	if !cap.Presign() {
		return nil
	}

	tests := make([]behaviorTest, 0, 6)
	if cap.PresignWrite() && cap.Stat() {
		tests = append(tests, testPresignWrite)
	}
	if cap.PresignRead() && cap.Write() {
		tests = append(tests, testPresignRead)
		tests = append(tests, testPresignReadWithRange)
	}
	if cap.PresignStat() && cap.Write() {
		tests = append(tests, testPresignStat)
		if isCapEnabled(cap.StatWithOverrideContentType, "stat_with_override_content_type") {
			tests = append(tests, testPresignStatWithOverrideContentType)
		}
	}
	if cap.PresignDelete() {
		tests = append(tests, testPresignDelete)
	}
	return tests
}

func testPresignWrite(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	req, err := op.PresignWrite(path, time.Hour)
	assert.Nil(err)

	req.ContentLength = int64(len(content))
	req.Body = io.NopCloser(bytes.NewReader(content))

	resp, err := http.DefaultClient.Do(req)
	assert.Nil(err)
	defer resp.Body.Close()
	_, err = io.Copy(io.Discard, resp.Body)
	assert.Nil(err)
	assert.GreaterOrEqual(resp.StatusCode, 200)
	assert.Less(resp.StatusCode, 300)

	meta, err := op.Stat(path)
	assert.Nil(err)
	assert.EqualValues(size, meta.ContentLength())
}

func testPresignRead(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err)

	req, err := op.PresignRead(path, time.Hour)
	assert.Nil(err)

	resp, err := http.DefaultClient.Do(req)
	assert.Nil(err)
	defer resp.Body.Close()

	bs, err := io.ReadAll(resp.Body)
	assert.Nil(err)
	assert.Equal(int(size), len(bs))

	assert.Equal(content, bs)
}

func testPresignReadWithRange(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	const (
		offset = 3
		length = 7
	)

	path := fixture.NewFilePath()
	content := genFixedBytes(32)

	_, err := op.Write(path, content)
	assert.Nil(err)

	req, err := op.PresignRead(path, time.Hour, opendal.ReadWithRange(offset, length))
	assert.Nil(err)

	resp, err := http.DefaultClient.Do(req)
	assert.Nil(err)
	defer resp.Body.Close()

	bs, err := io.ReadAll(resp.Body)
	assert.Nil(err)
	assert.GreaterOrEqual(resp.StatusCode, 200)
	assert.Less(resp.StatusCode, 300)
	assert.Equal(int(length), len(bs))
	assert.Equal(content[offset:offset+length], bs)
}

func testPresignStat(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err)

	req, err := op.PresignStat(path, time.Hour)
	assert.Nil(err)

	resp, err := http.DefaultClient.Do(req)
	assert.Nil(err)
	defer resp.Body.Close()
	assert.Equal(http.StatusOK, resp.StatusCode)

	lengthHeader := resp.Header.Get("Content-Length")
	assert.NotEmpty(lengthHeader)
	length, err := strconv.ParseUint(lengthHeader, 10, 64)
	assert.Nil(err)
	assert.EqualValues(size, length)
}

func testPresignStatWithOverrideContentType(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()
	contentType := "application/octet-stream"

	_, err := op.Write(path, content)
	assert.Nil(err)

	req, err := op.PresignStat(path, time.Hour, opendal.StatWithOverrideContentType(contentType))
	assert.Nil(err)

	resp, err := http.DefaultClient.Do(req)
	assert.Nil(err)
	defer resp.Body.Close()
	assert.Equal(http.StatusOK, resp.StatusCode)
	assert.Equal(contentType, resp.Header.Get("Content-Type"))
}

func testPresignDelete(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err)

	req, err := op.PresignDelete(path, time.Hour)
	assert.Nil(err)

	resp, err := http.DefaultClient.Do(req)
	assert.Nil(err)
	defer resp.Body.Close()
	_, err = io.Copy(io.Discard, resp.Body)
	assert.Nil(err)
	assert.GreaterOrEqual(resp.StatusCode, 200)
	assert.Less(resp.StatusCode, 300)

	exists, err := op.IsExist(path)
	assert.Nil(err)
	assert.False(exists)
}
