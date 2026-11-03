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
	opendal "github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func testsWrite(cap *opendal.Capability) []behaviorTest {
	if !cap.Write() || !cap.Stat() || !cap.Read() {
		return nil
	}
	return []behaviorTest{
		testWriteOnly,
		testWriteWithEmptyContent,
		testWriteWithDirPath,
		testWriteWithSpecialChars,
		testWriteOverwrite,
		testWriteWithCacheControl,
		testWriteWithContentType,
		testWriteWithContentDisposition,
		testWriteWithContentEncoding,
		testWriteWithUserMetadata,
		testWriteWithIfMatch,
		testWriteWithIfNoneMatch,
		testWriteWithIfNotExists,
		testWriterWrite,
		testWriteWithChunkAndConcurrent,
		testWriterWithAppend,
		testWriteReturnsMetadata,
		testWriterCloseReturnsMetadata,
	}
}

func testWriteOnly(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(size), meta.ContentLength())
}

func testWriteWithEmptyContent(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().WriteCanEmpty() {
		return
	}

	path := fixture.NewFilePath()
	_, err := op.Write(path, []byte{})
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(0), meta.ContentLength())
}

func testWriteWithDirPath(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewDirPath()

	_, err := op.Write(path, []byte("1"))
	assert.NotNil(err)
	assert.Equal(opendal.CodeIsADirectory, assertErrorCode(err))
}

func testWriteWithSpecialChars(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFileWithPath(uuid.NewString() + " !@#$%^&()_+-=;',.txt")

	_, err := op.Write(path, content)
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(size), meta.ContentLength())
}

func testWriteOverwrite(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().WriteCanMulti() {
		return
	}

	path := fixture.NewFilePath()
	size := uint(5 * 1024 * 1024)
	contentOne, contentTwo := genFixedBytes(size), genFixedBytes(size)

	_, err := op.Write(path, contentOne)
	assert.Nil(err)
	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal(contentOne, bs, "read content_one")

	_, err = op.Write(path, contentTwo)
	assert.Nil(err)
	bs, err = op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.NotEqual(contentOne, bs, "content_one must be overwrote")
	assert.Equal(contentTwo, bs, "read content_two")
}

func testWriteWithCacheControl(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithCacheControl, "write_with_cache_control") {
		return
	}

	path := fixture.NewFilePath()
	content := []byte("hello")
	_, err := op.Write(path, content, opendal.WriteWithCacheControl("max-age=60"))
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(len(content)), meta.ContentLength())
	cacheControl, ok := meta.CacheControl()
	assert.True(ok, "cache control must exist")
	assert.Equal("max-age=60", cacheControl)
}

func testWriteWithContentType(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithContentType, "write_with_content_type") {
		return
	}

	path := fixture.NewFilePath()
	content := []byte("hello")
	_, err := op.Write(path, content, opendal.WriteWithContentType("text/plain"))
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(len(content)), meta.ContentLength())
	contentType, ok := meta.ContentType()
	assert.True(ok, "content type must exist")
	assert.Equal("text/plain", contentType)
}

func testWriteWithContentDisposition(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithContentDisposition, "write_with_content_disposition") {
		return
	}

	path := fixture.NewFilePath()
	content := []byte("hello")
	_, err := op.Write(path, content, opendal.WriteWithContentDisposition("attachment; filename=hello.txt"))
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(len(content)), meta.ContentLength())
	contentDisposition, ok := meta.ContentDisposition()
	assert.True(ok, "content disposition must exist")
	assert.Equal("attachment; filename=hello.txt", contentDisposition)
}

func testWriteWithContentEncoding(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithContentEncoding, "write_with_content_encoding") {
		return
	}

	path := fixture.NewFilePath()
	content := []byte("hello")
	_, err := op.Write(path, content, opendal.WriteWithContentEncoding("gzip"))
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(len(content)), meta.ContentLength())
	contentEncoding, ok := meta.ContentEncoding()
	assert.True(ok, "content encoding must exist")
	assert.Equal("gzip", contentEncoding)
}

func testWriteWithUserMetadata(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithUserMetadata, "write_with_user_metadata") {
		return
	}

	path := fixture.NewFilePath()
	content := []byte("hello")
	_, err := op.Write(path, content, opendal.WriteWithUserMetadata(map[string]string{
		"language": "go",
		"project":  "opendal",
	}))
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(len(content)), meta.ContentLength())
	assert.Equal(map[string]string{
		"language": "go",
		"project":  "opendal",
	}, meta.UserMetadata())
}

func testWriteWithIfMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithIfMatch, "write_with_if_match") {
		return
	}

	path := fixture.NewFilePath()
	_, err := op.Write(path, []byte("hello"))
	assert.Nil(err)
	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	etag, ok := meta.ETag()
	assert.True(ok, "etag must exist")

	_, err = op.Write(path, []byte("world"), opendal.WriteWithIfMatch(etag))
	assert.Nil(err)
	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal([]byte("world"), bs)
}

func testWriteWithIfNoneMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithIfNoneMatch, "write_with_if_none_match") {
		return
	}

	path := fixture.NewFilePath()
	_, err := op.Write(path, []byte("hello"))
	assert.Nil(err)
	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	etag, ok := meta.ETag()
	assert.True(ok, "etag must exist")

	_, err = op.Write(path, []byte("world"), opendal.WriteWithIfNoneMatch(etag))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal([]byte("hello"), bs)
}

func testWriteWithIfNotExists(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteWithIfNotExists, "write_with_if_not_exists") {
		return
	}

	path := fixture.NewFilePath()
	_, err := op.Write(path, []byte("hello"), opendal.WriteWithIfNotExists(true))
	assert.Nil(err)
	_, err = op.Write(path, []byte("world"), opendal.WriteWithIfNotExists(true))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal([]byte("hello"), bs)
}

func testWriterWrite(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().WriteCanMulti() {
		return
	}

	path := fixture.NewFilePath()
	size := uint(5 * 1024 * 1024)
	contentA := genFixedBytes(size)
	contentB := genFixedBytes(size)

	w, err := op.Writer(path)
	assert.Nil(err)
	_, err = w.Write(contentA)
	assert.Nil(err)
	_, err = w.Write(contentB)
	assert.Nil(err)
	_, err = w.Close()
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.Equal(uint64(size*2), meta.ContentLength())

	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal(uint64(size*2), uint64(len(bs)), "read size")
	assert.Equal(contentA, bs[:size], "read contentA")
	assert.Equal(contentB, bs[size:], "read contentB")
}

func testWriteWithChunkAndConcurrent(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteCanMulti, "write_can_multi") {
		return
	}

	path := fixture.NewFilePath()
	content := genFixedBytes(1024 * 1024)
	_, err := op.Write(path, content, opendal.WriteWithChunk(256*1024), opendal.WriteWithConcurrent(2))
	assert.Nil(err)

	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal(content, bs)
}

func testWriterWithAppend(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !isCapEnabled(op.Info().GetCapability().WriteCanAppend, "write_can_append") {
		return
	}

	path := fixture.NewFilePath()

	w, err := op.Writer(path, opendal.WriteWithAppend(true))
	assert.Nil(err)
	_, err = w.Write([]byte("hello"))
	assert.Nil(err)
	_, err = w.Close()
	assert.Nil(err)

	w, err = op.Writer(path, opendal.WriteWithAppend(true))
	assert.Nil(err)
	_, err = w.Write([]byte(" world"))
	assert.Nil(err)
	_, err = w.Close()
	assert.Nil(err)

	bs, err := op.Read(path)
	assert.Nil(err, "read must succeed")
	assert.Equal([]byte("hello world"), bs)
}

func testWriteReturnsMetadata(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	// Write returns the metadata of the written object. Which fields are
	// populated is service-dependent (e.g. content length may be 0), so only
	// assert that metadata is returned and the content round-trips.
	meta, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")
	assert.NotNil(meta, "write must return metadata")

	data, err := op.Read(path)
	assert.Nil(err)
	assert.Equal(content, data, "written content")
}

func testWriterCloseReturnsMetadata(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewFilePath()
	content := []byte("hello opendal write metadata")

	w, err := op.Writer(path)
	assert.Nil(err)
	_, err = w.Write(content)
	assert.Nil(err)

	// Close returns the metadata of the written object; fields are
	// service-dependent, so only assert that metadata is returned.
	meta, err := w.Close()
	assert.Nil(err, "close must succeed")
	assert.NotNil(meta, "close must return metadata")

	data, err := op.Read(path)
	assert.Nil(err)
	assert.Equal(content, data, "written content")
}
