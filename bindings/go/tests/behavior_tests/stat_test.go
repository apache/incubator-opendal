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
	"fmt"
	"strings"
	"time"

	"github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func testsStat(cap *opendal.Capability) []behaviorTest {
	if !cap.Write() || !cap.Stat() {
		return nil
	}
	tests := []behaviorTest{
		testStatFile,
		testStatDir,
		testStatNestedParentDir,
		testStatWithSpecialChars,
		testStatNotCleanedPath,
		testStatNotExist,
		testStatRoot,
		testStatFileMetadata,
		testStatDirMetadata,
	}
	if isCapEnabled(cap.StatWithIfMatch, "stat_with_if_match") {
		tests = append(tests, testStatWithIfMatch)
	}
	if isCapEnabled(cap.StatWithIfNoneMatch, "stat_with_if_none_match") {
		tests = append(tests, testStatWithIfNoneMatch)
	}
	if isCapEnabled(cap.StatWithIfModifiedSince, "stat_with_if_modified_since") {
		tests = append(tests, testStatWithIfModifiedSince)
	}
	if isCapEnabled(cap.StatWithIfUnmodifiedSince, "stat_with_if_unmodified_since") {
		tests = append(tests, testStatWithIfUnmodifiedSince)
	}
	return tests
}

func assertOptionalMetaString(assert *require.Assertions, name string, accessor func() (string, bool)) {
	value, ok := accessor()
	if ok {
		assert.NotEmpty(value, "%s reported as present must have a non-empty value", name)
	} else {
		assert.Empty(value, "%s reported as absent must return an empty value", name)
	}
}

func testStatFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err)

	meta, err := op.Stat(path)
	assert.Nil(err)
	assert.True(meta.IsFile())
	assert.Equal(meta.ContentLength(), uint64(size))

	if op.Info().GetCapability().CreateDir() {
		_, err := op.Stat(fmt.Sprintf("%s/", path))
		assert.NotNil(err)
		assert.Equal(opendal.CodeNotFound, assertErrorCode(err))
	}
}

func testStatDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().CreateDir() {
		return
	}

	path := fixture.NewDirPath()
	assert.Nil(op.CreateDir(path))

	meta, err := op.Stat(path)
	assert.Nil(err)
	assert.True(meta.IsDir())

	meta, err = op.Stat(strings.TrimSuffix(path, "/"))
	if err != nil {
		assert.Equal(opendal.CodeNotFound, assertErrorCode(err))
	} else {
		assert.True(meta.IsDir())
	}
}

func testStatNestedParentDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().CreateDir() {
		return
	}

	parent := fixture.NewDirPath()
	path, content, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(parent)
	assert.Nil(err)
	assert.True(meta.IsDir())
}

func testStatWithSpecialChars(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFileWithPath(uuid.NewString() + " !@#$%^&()_+-=;',.txt")

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	assert.True(meta.IsFile())
	assert.Equal(uint64(size), meta.ContentLength())
}

func testStatNotCleanedPath(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(fmt.Sprintf("//%s", path))
	assert.Nil(err)
	assert.True(meta.IsFile())
	assert.Equal(uint64(size), meta.ContentLength())
}

func testStatNotExist(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewFilePath()

	_, err := op.Stat(path)
	assert.NotNil(err)
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))

	if op.Info().GetCapability().CreateDir() {
		_, err := op.Stat(fmt.Sprintf("%s/", path))
		assert.NotNil(err)
		assert.Equal(opendal.CodeNotFound, assertErrorCode(err))
	}
}

func testStatRoot(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	meta, err := op.Stat("")
	assert.Nil(err)
	assert.True(meta.IsDir())

	meta, err = op.Stat("/")
	assert.Nil(err)
	assert.True(meta.IsDir())

}

func testStatFileMetadata(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	cap := op.Info().GetCapability()

	writeOpts := make([]opendal.WithWriteFn, 0, 5)
	writeWithCacheControl := isCapEnabled(cap.WriteWithCacheControl, "write_with_cache_control")
	writeWithContentDisposition := isCapEnabled(cap.WriteWithContentDisposition, "write_with_content_disposition")
	writeWithContentEncoding := isCapEnabled(cap.WriteWithContentEncoding, "write_with_content_encoding")
	writeWithContentType := isCapEnabled(cap.WriteWithContentType, "write_with_content_type")
	writeWithUserMetadata := isCapEnabled(cap.WriteWithUserMetadata, "write_with_user_metadata")

	if writeWithCacheControl {
		writeOpts = append(writeOpts, opendal.WriteWithCacheControl("max-age=60"))
	}
	if writeWithContentDisposition {
		writeOpts = append(writeOpts, opendal.WriteWithContentDisposition("attachment; filename=hello.txt"))
	}
	if writeWithContentEncoding {
		writeOpts = append(writeOpts, opendal.WriteWithContentEncoding("gzip"))
	}
	if writeWithContentType {
		writeOpts = append(writeOpts, opendal.WriteWithContentType("text/plain"))
	}
	userMetadata := map[string]string{
		"language": "go",
		"project":  "opendal",
	}
	if writeWithUserMetadata {
		writeOpts = append(writeOpts, opendal.WriteWithUserMetadata(userMetadata))
	}

	before := time.Now().Add(-time.Hour)
	var err error
	if len(writeOpts) == 0 {
		_, err = op.Write(path, content)
		assert.Nil(err, "write must succeed")
	} else {
		_, err = op.Write(path, content, writeOpts...)
		assert.Nil(err, "write with metadata must succeed")
	}

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")

	assert.True(meta.IsFile(), "written object must be a file")
	assert.False(meta.IsDir(), "written object must not be a dir")
	assert.Equal(opendal.EntryModeFile, meta.Mode(), "mode must report file")
	assert.False(meta.IsDeleted(), "freshly written object must not be deleted")
	assert.Equal(uint64(size), meta.ContentLength(), "content length must match written size")

	if lm := meta.LastModified(); !lm.IsZero() {
		assert.False(lm.Before(before), "last_modified must be recent, got %v", lm)
		assert.False(lm.After(time.Now().Add(time.Minute)), "last_modified must not be in the future, got %v", lm)
	}

	if writeWithCacheControl {
		cacheControl, ok := meta.CacheControl()
		assert.True(ok, "cache control must exist")
		assert.Equal("max-age=60", cacheControl)
	} else {
		assertOptionalMetaString(assert, "cache control", meta.CacheControl)
	}
	if writeWithContentDisposition {
		contentDisposition, ok := meta.ContentDisposition()
		assert.True(ok, "content disposition must exist")
		assert.Equal("attachment; filename=hello.txt", contentDisposition)
	} else {
		assertOptionalMetaString(assert, "content disposition", meta.ContentDisposition)
	}
	if writeWithContentEncoding {
		contentEncoding, ok := meta.ContentEncoding()
		assert.True(ok, "content encoding must exist")
		assert.Equal("gzip", contentEncoding)
	} else {
		assertOptionalMetaString(assert, "content encoding", meta.ContentEncoding)
	}
	if writeWithContentType {
		contentType, ok := meta.ContentType()
		assert.True(ok, "content type must exist")
		assert.Equal("text/plain", contentType)
	} else {
		assertOptionalMetaString(assert, "content type", meta.ContentType)
	}
	assertOptionalMetaString(assert, "content md5", meta.ContentMD5)
	assertOptionalMetaString(assert, "etag", meta.ETag)
	assertOptionalMetaString(assert, "version", meta.Version)

	if isCurrent, ok := meta.IsCurrent(); ok {
		assert.True(isCurrent, "a live object must be reported as the current version")
	}
	if writeWithUserMetadata {
		assert.Equal(userMetadata, meta.UserMetadata())
	} else if um := meta.UserMetadata(); um != nil {
		assert.Equal(um, meta.UserMetadata(), "user metadata accessor must return equal copies")
	}
}

func testStatWithIfMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	cap := op.Info().GetCapability()
	if !isCapEnabled(cap.StatWithIfMatch, "stat_with_if_match") {
		return
	}

	path, content, _ := fixture.NewFile()

	// Write with options to exercise the write-with-options path.
	writeOpts := make([]opendal.WithWriteFn, 0, 1)
	if isCapEnabled(cap.WriteWithContentType, "write_with_content_type") {
		writeOpts = append(writeOpts, opendal.WriteWithContentType("text/plain"))
	}
	_, err := op.Write(path, content, writeOpts...)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	etag, ok := meta.ETag()
	assert.True(ok, "etag must exist")

	// Stat with a matching ETag must succeed.
	meta, err = op.Stat(path, opendal.StatWithIfMatch(etag))
	assert.Nil(err, "stat with matching if_match must succeed")
	assert.Equal(uint64(len(content)), meta.ContentLength())

	// Stat with a non-matching ETag must fail with ConditionNotMatch.
	_, err = op.Stat(path, opendal.StatWithIfMatch("\"invalid_etag\""))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))
}

func testStatWithIfNoneMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	cap := op.Info().GetCapability()
	if !isCapEnabled(cap.StatWithIfNoneMatch, "stat_with_if_none_match") {
		return
	}

	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	etag, ok := meta.ETag()
	assert.True(ok, "etag must exist")

	// Stat with a non-matching ETag must succeed and return metadata.
	meta, err = op.Stat(path, opendal.StatWithIfNoneMatch("\"invalid_etag\""))
	assert.Nil(err, "stat with non-matching if_none_match must succeed")
	assert.Equal(uint64(size), meta.ContentLength())

	// Stat with a matching ETag must fail with ConditionNotMatch.
	_, err = op.Stat(path, opendal.StatWithIfNoneMatch(etag))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))
}

func testStatWithIfModifiedSince(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	cap := op.Info().GetCapability()
	if !isCapEnabled(cap.StatWithIfModifiedSince, "stat_with_if_modified_since") {
		return
	}

	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.True(meta.IsFile(), "written object must be a file")
	assert.Equal(uint64(len(content)), meta.ContentLength())

	lastModified := meta.LastModified()
	assert.False(lastModified.IsZero(), "last_modified must exist")

	meta, err = op.Stat(path, opendal.StatWithIfModifiedSince(lastModified.Add(-time.Second)))
	assert.Nil(err, "stat with older if_modified_since must succeed")
	assert.Equal(lastModified, meta.LastModified())

	_, err = op.Stat(path, opendal.StatWithIfModifiedSince(lastModified.Add(time.Second)))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))
}

func testStatWithIfUnmodifiedSince(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	cap := op.Info().GetCapability()
	if !isCapEnabled(cap.StatWithIfUnmodifiedSince, "stat_with_if_unmodified_since") {
		return
	}

	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")
	assert.True(meta.IsFile(), "written object must be a file")
	assert.Equal(uint64(len(content)), meta.ContentLength())

	lastModified := meta.LastModified()
	assert.False(lastModified.IsZero(), "last_modified must exist")

	_, err = op.Stat(path, opendal.StatWithIfUnmodifiedSince(lastModified.Add(-time.Second)))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	meta, err = op.Stat(path, opendal.StatWithIfUnmodifiedSince(lastModified.Add(time.Second)))
	assert.Nil(err, "stat with newer if_unmodified_since must succeed")
	assert.Equal(lastModified, meta.LastModified())
}

func testStatDirMetadata(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().CreateDir() {
		return
	}

	path := fixture.NewDirPath()
	assert.Nil(op.CreateDir(path), "create dir must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err, "stat must succeed")

	assert.True(meta.IsDir(), "created object must be a dir")
	assert.False(meta.IsFile(), "created object must not be a file")
	assert.Equal(opendal.EntryModeDir, meta.Mode(), "mode must report dir")
	assert.False(meta.IsDeleted(), "freshly created dir must not be deleted")
}
