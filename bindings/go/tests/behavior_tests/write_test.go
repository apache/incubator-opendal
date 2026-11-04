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
	"errors"
	"io"
	"os"

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
		testWriterReadFrom,
		testWriterAfterCopy,
		testWriterReadFromResults,
		testWriterReadFromBufferOwnership,
		testWriterReadFromSourcePanic,
	}
}

const copyBufferSize = 256 * 1024

type copyReaderFunc func([]byte) (int, error)

func (f copyReaderFunc) Read(p []byte) (int, error) { return f(p) }

func testWriterReadFrom(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().WriteCanMulti() {
		return
	}
	content := genFixedBytes(contentMaxSize(op.Info().GetCapability()))
	file, err := os.CreateTemp("", "opendal-upload-*")
	assert.Nil(err)
	defer func() { _ = os.Remove(file.Name()) }()
	defer func() { assert.Nil(file.Close()) }()
	_, err = file.Write(content)
	assert.Nil(err)

	for _, source := range []string{"file", "plain_reader"} {
		for _, method := range []string{"Copy", "CopyBuffer", "ReadFrom"} {
			_, err := file.Seek(0, io.SeekStart)
			assert.Nil(err)
			reader := bytes.NewReader(content)
			maxRead := 0
			var src io.Reader = copyReaderFunc(func(p []byte) (int, error) {
				maxRead = max(maxRead, len(p))
				return reader.Read(p)
			})
			if source == "file" {
				src = file
			}
			path := fixture.NewFilePath()
			w, err := op.Writer(path)
			assert.Nil(err)
			var n int64
			switch method {
			case "Copy":
				n, err = io.Copy(w, src)
			case "CopyBuffer":
				n, err = io.CopyBuffer(w, src, make([]byte, 7))
			case "ReadFrom":
				n, err = w.ReadFrom(src)
			}
			meta, closeErr := w.Close()
			assert.Nil(err, source+"/"+method)
			assert.Nil(closeErr)
			assert.NotNil(meta)
			assert.Equal(int64(len(content)), n)
			if source == "plain_reader" {
				assert.Equal(copyBufferSize, maxRead, "copy must dispatch to ReadFrom")
			} else {
				_, err = file.Seek(0, io.SeekStart)
				assert.Nil(err, "copy must leave the source open")
			}
			got, err := op.Read(path)
			assert.Nil(err)
			assert.Equal(content, got)
		}
	}
}

func testWriterAfterCopy(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	cap := op.Info().GetCapability()
	if !cap.WriteCanMulti() {
		return
	}
	contents := [][]byte{
		bytes.Repeat([]byte("a"), copyBufferSize+17),
		bytes.Repeat([]byte("b"), copyBufferSize+29),
	}
	markers := [][]byte{[]byte("prefix"), []byte("between"), []byte("tail")}
	want := append([]byte(nil), markers[0]...)
	for i, content := range contents {
		want = append(want, content...)
		want = append(want, markers[i+1]...)
	}
	if limit := cap.WriteTotalMaxSize(); limit > 0 && uint(len(want)) > limit {
		return
	}
	sources := make([]string, len(contents))
	for i, content := range contents {
		sources[i] = fixture.NewFilePath()
		_, err := op.Write(sources[i], content)
		assert.Nil(err)
	}

	for _, method := range []string{"ReadFrom", "WriteTo"} {
		for _, buffered := range []bool{false, true} {
			var options []opendal.WithWriteFn
			if buffered {
				options = append(options, opendal.WriteWithChunk(copyBufferSize*4))
			}
			path := fixture.NewFilePath()
			w, err := op.Writer(path, options...)
			assert.Nil(err)
			n, err := w.Write(markers[0])
			assert.Nil(err)
			assert.Equal(len(markers[0]), n)
			for i, source := range sources {
				r, err := op.Reader(source, opendal.ReaderWithChunk(64*1024))
				assert.Nil(err)
				var copied int64
				switch method {
				case "ReadFrom":
					copied, err = w.ReadFrom(r)
				case "WriteTo":
					copied, err = r.WriteTo(w)
				}
				assert.Nil(err, "%s, buffered=%t, copy=%d", method, buffered, i)
				assert.Equal(int64(len(contents[i])), copied)
				assert.Nil(r.Close())
				n, err = w.Write(markers[i+1])
				assert.Nil(err, "%s must leave the original writer usable, buffered=%t, copy=%d", method, buffered, i)
				assert.Equal(len(markers[i+1]), n)
			}
			meta, err := w.Close()
			assert.Nil(err, "%s, buffered=%t", method, buffered)
			assert.NotNil(meta)
			got, err := op.Read(path)
			assert.Nil(err)
			assert.Equal(want, got, "%s, buffered=%t", method, buffered)
			meta, err = op.Stat(path)
			assert.Nil(err)
			assert.Equal(uint64(len(want)), meta.ContentLength())
		}
	}
}

func testWriterReadFromResults(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourceErr := errors.New("source failed")
	for _, size := range []int{0, 3, copyBufferSize} {
		if size == 0 && !op.Info().GetCapability().WriteCanEmpty() {
			continue
		}
		for _, end := range []error{io.EOF, sourceErr} {
			path := fixture.NewFilePath()
			w, err := op.Writer(path)
			assert.Nil(err)
			reads := 0
			data := bytes.Repeat([]byte("s"), size)
			n, copyErr := w.ReadFrom(copyReaderFunc(func(p []byte) (int, error) {
				reads++
				if reads == 1 {
					return 0, nil
				}
				return copy(p, data), end
			}))
			_, closeErr := w.Close()
			wantErr := end
			if end == io.EOF {
				wantErr = nil
			}
			assert.Equal(wantErr, copyErr, "size=%d, end=%v", size, end)
			assert.Equal(int64(size), n)
			assert.Nil(closeErr)
			assert.Equal(2, reads, "a zero-byte read without an error is not EOF")
			got, err := op.Read(path)
			assert.Nil(err)
			assert.True(bytes.Equal(data, got), "stored data must match the source")
		}
	}
}

func testWriterReadFromBufferOwnership(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().WriteCanMulti() {
		return
	}
	path := fixture.NewFilePath()
	w, err := op.Writer(path, opendal.WriteWithChunk(copyBufferSize*4))
	assert.Nil(err)
	// Use different data in each buffer to detect a writer that keeps the input slice.
	data := make([]byte, copyBufferSize*3+19)
	for i := range data {
		data[i] = byte(i/copyBufferSize + 1)
	}
	n, copyErr := w.ReadFrom(bytes.NewReader(data))
	tail := []byte("caller owned tail")
	_, writeErr := w.Write(tail)
	want := append(append([]byte(nil), data...), tail...)
	clear(tail)
	clear(data)
	_, closeErr := w.Close()
	assert.Equal(int64(len(data)), n)
	assert.Nil(copyErr)
	assert.Nil(writeErr, "ReadFrom must leave the writer open")
	assert.Nil(closeErr)
	got, err := op.Read(path)
	assert.Nil(err)
	assert.Equal(want, got)
}

func testWriterReadFromSourcePanic(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().WriteCanEmpty() {
		return
	}
	path := fixture.NewFilePath()
	w, err := op.Writer(path)
	assert.Nil(err)
	value := errors.New("source panic")
	assert.PanicsWithValue(value, func() {
		_, _ = w.ReadFrom(copyReaderFunc(func([]byte) (int, error) { panic(value) }))
	})
	_, err = w.Close()
	assert.Nil(err)
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
