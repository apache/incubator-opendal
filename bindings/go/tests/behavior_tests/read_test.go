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
	"sync/atomic"
	"time"

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func testsRead(cap *opendal.Capability) []behaviorTest {
	if !cap.Read() || !cap.Write() {
		return nil
	}
	tests := []behaviorTest{
		testReadFull,
		testReader,
		testReadNotExist,
		testReadWithDirPath,
		testReadWithSpecialChars,
		testReaderSeek,
		testReadWithRange,
		testReadWithRangeFrom,
		testReadWithContentLengthHint,
		testReadWithConcurrentChunkGap,
		testReaderWithConcurrentChunkGap,
		testReaderWriteTo,
		testReaderWriteToFileAfterSeek,
		testReaderWriteToResults,
		testReaderWriteToDestinationPanic,
		testReaderWriteToBackpressure,
	}
	if cap.WriteCanMulti() {
		tests = append(tests, testIOCopy)
		tests = append(tests, testReadWithWriteOptions)
	}
	if isCapEnabled(cap.ReadWithIfMatch, "read_with_if_match") {
		tests = append(tests, testReadWithIfMatch)
		tests = append(tests, testReaderWithIfMatch)
	}
	if isCapEnabled(cap.ReadWithIfNoneMatch, "read_with_if_none_match") {
		tests = append(tests, testReadWithIfNoneMatch)
	}
	if isCapEnabled(cap.ReadWithIfModifiedSince, "read_with_if_modified_since") {
		tests = append(tests, testReadWithIfModifiedSince)
	}
	if isCapEnabled(cap.ReadWithIfUnmodifiedSince, "read_with_if_unmodified_since") {
		tests = append(tests, testReadWithIfUnmodifiedSince)
	}
	if cap.ReadWithVersion() {
		tests = append(tests, testReadWithVersion)
		tests = append(tests, testReaderWithVersion)
	}
	return tests
}

type copyWriterFunc func([]byte) (int, error)

func (f copyWriterFunc) Write(p []byte) (int, error) { return f(p) }

func testReaderWriteTo(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	_, err := op.Write(path, content)
	assert.Nil(err)

	for _, method := range []string{"Copy", "CopyBuffer", "WriteTo"} {
		r, err := op.Reader(path)
		assert.Nil(err)
		var dst bytes.Buffer
		maxWrite := 0
		writer := copyWriterFunc(func(p []byte) (int, error) {
			maxWrite = max(maxWrite, len(p))
			return dst.Write(p)
		})
		var n int64
		switch method {
		case "Copy":
			n, err = io.Copy(writer, r)
		case "CopyBuffer":
			n, err = io.CopyBuffer(writer, r, make([]byte, 7))
		case "WriteTo":
			n, err = r.WriteTo(writer)
		}
		closeErr := r.Close()
		assert.Nil(err, method)
		assert.Nil(closeErr, method)
		assert.Equal(int64(size), n, method)
		assert.Equal(content, dst.Bytes(), method)
		assert.LessOrEqual(maxWrite, copyBufferSize, method)
	}
}

func testReaderWriteToFileAfterSeek(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	offset := int64(size / 3)
	_, err := op.Write(path, content)
	assert.Nil(err)
	r, err := op.Reader(path)
	assert.Nil(err)
	defer func() { assert.Nil(r.Close()) }()
	_, err = r.Seek(offset, io.SeekStart)
	assert.Nil(err)

	dst, err := os.CreateTemp("", "opendal-download-*")
	assert.Nil(err)
	defer func() { _ = os.Remove(dst.Name()) }()
	defer func() { assert.Nil(dst.Close()) }()
	n, err := io.CopyBuffer(dst, r, make([]byte, 7))
	assert.Nil(err)
	assert.Equal(int64(size)-offset, n)
	pos, err := r.Seek(0, io.SeekCurrent)
	assert.Nil(err)
	assert.Equal(int64(size), pos, "copy must leave the reader open at EOF")
	_, err = dst.Seek(0, io.SeekStart)
	assert.Nil(err, "copy must leave the destination open")
	got, err := io.ReadAll(dst)
	assert.Nil(err)
	assert.Equal(content[offset:], got)
}

func testReaderWriteToResults(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewFilePath()
	_, err := op.Write(path, []byte("abcdef"))
	assert.Nil(err)
	destinationErr := errors.New("destination failed")
	for _, tc := range []struct {
		name string
		err  error
	}{
		{"short", io.ErrShortWrite},
		{"partial_error", destinationErr},
		{"full_error", destinationErr},
		{"destination_EOF", io.EOF},
		{"negative", io.ErrShortWrite},
		{"too_large", io.ErrShortWrite},
	} {
		r, err := op.Reader(path)
		assert.Nil(err)
		writes, wantN := 0, 0
		n, err := r.WriteTo(copyWriterFunc(func(p []byte) (int, error) {
			writes++
			switch tc.name {
			case "negative":
				return -1, nil
			case "too_large":
				return len(p) + 1, nil
			case "full_error":
				wantN = len(p)
			default:
				wantN = len(p) / 2
			}
			if tc.name == "short" {
				return wantN, nil
			}
			return wantN, tc.err
		}))
		closeErr := r.Close()
		if tc.name == "negative" || tc.name == "too_large" {
			assert.Error(err, tc.name)
		} else {
			assert.Equal(tc.err, err, tc.name)
		}
		assert.Equal(int64(wantN), n, tc.name)
		assert.Equal(1, writes, tc.name)
		assert.Nil(closeErr)
	}

	if op.Info().GetCapability().WriteCanEmpty() {
		path := fixture.NewFilePath()
		_, err := op.Write(path, nil)
		assert.Nil(err)
		r, err := op.Reader(path)
		assert.Nil(err)
		writes := 0
		n, err := r.WriteTo(copyWriterFunc(func(p []byte) (int, error) {
			writes++
			return len(p), nil
		}))
		closeErr := r.Close()
		assert.Nil(err)
		assert.Zero(n)
		assert.Zero(writes)
		assert.Nil(closeErr)
	}
}

func testReaderWriteToDestinationPanic(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewFilePath()
	_, err := op.Write(path, []byte("abc"))
	assert.Nil(err)
	r, err := op.Reader(path)
	assert.Nil(err)
	defer func() { assert.Nil(r.Close()) }()
	value := errors.New("destination panic")
	assert.PanicsWithValue(value, func() {
		_, _ = r.WriteTo(copyWriterFunc(func([]byte) (int, error) { panic(value) }))
	})
}

func testReaderWriteToBackpressure(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	_, err := op.Write(path, content)
	assert.Nil(err)
	r, err := op.Reader(path)
	assert.Nil(err)
	defer func() { assert.Nil(r.Close()) }()

	entered, release := make(chan struct{}), make(chan struct{})
	type result struct {
		n   int64
		err error
	}
	done := make(chan result, 1)
	var writes atomic.Int32
	var dst bytes.Buffer
	go func() {
		n, err := r.WriteTo(copyWriterFunc(func(p []byte) (int, error) {
			if writes.Add(1) == 1 {
				close(entered)
			}
			<-release
			return dst.Write(p)
		}))
		done <- result{n, err}
	}()

	select {
	case <-entered:
	case res := <-done:
		close(release)
		assert.FailNow("copy returned without writing", "%d bytes, %v", res.n, res.err)
	}
	var early *result
	select {
	case res := <-done:
		early = &res
	case <-time.After(20 * time.Millisecond):
	}
	blockedWrites := writes.Load()
	close(release)
	var res result
	if early != nil {
		res = *early
	} else {
		res = <-done
	}
	assert.Nil(early, "WriteTo must wait for the destination write")
	assert.Equal(int32(1), blockedWrites, "WriteTo must serialize destination writes")
	assert.Nil(res.err)
	assert.Equal(int64(size), res.n)
	assert.Equal(content, dst.Bytes())
}

func testReadWithConcurrentChunkGap(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	bs, err := op.Read(path,
		opendal.ReadWithConcurrent(2),
		opendal.ReadWithChunk(1024*1024),
		opendal.ReadWithGap(4096),
	)
	assert.Nil(err)
	assert.Equal(size, uint(len(bs)), "read size")
	assert.Equal(content, bs, "read content")
}

func testReadWithWriteOptions(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewFilePath()
	content := genFixedBytes(1024 * 1024)
	offset, length := genOffsetLength(uint(len(content)))

	_, err := op.Write(path, content, opendal.WriteWithChunk(256*1024), opendal.WriteWithConcurrent(2))
	assert.Nil(err)

	bs, err := op.Read(path,
		opendal.ReadWithRange(uint64(offset), uint64(length)),
		opendal.ReadWithConcurrent(2),
		opendal.ReadWithChunk(128*1024),
		opendal.ReadWithGap(4096),
	)
	assert.Nil(err)
	assert.Equal(length, int64(len(bs)), "read range size")
	assert.Equal(content[offset:offset+length], bs, "read range content")
}

func testReadWithIfModifiedSince(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	lastModified := meta.LastModified()

	bs, err := op.Read(path, opendal.ReadWithIfModifiedSince(lastModified.Add(-time.Second)))
	assert.Nil(err, "read with if-modified-since before last modified must succeed")
	assert.Equal(content, bs, "read content")

	_, err = op.Read(path, opendal.ReadWithIfModifiedSince(lastModified))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))
}

func testReadWithIfUnmodifiedSince(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	lastModified := meta.LastModified()

	bs, err := op.Read(path, opendal.ReadWithIfUnmodifiedSince(lastModified.Add(time.Second)))
	assert.Nil(err, "read with if-unmodified-since after last modified must succeed")
	assert.Equal(content, bs, "read content")

	_, err = op.Read(path, opendal.ReadWithIfUnmodifiedSince(lastModified.Add(-time.Second)))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))
}

func testReadWithVersion(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	version, ok := meta.Version()
	if !ok {
		return
	}

	data, err := op.Read(path, opendal.ReadWithVersion(version))
	assert.Nil(err)
	assert.Equal(content, data, "read content")

	// After overwriting, the previous version data is still readable.
	_, err = op.Write(path, []byte("1"))
	assert.Nil(err, "overwrite must succeed")
	second, err := op.Read(path, opendal.ReadWithVersion(version))
	assert.Nil(err)
	assert.Equal(content, second, "read old version content")
}

func testReadWithRange(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	offset, length := genOffsetLength(size)

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	bs, err := op.Read(path, opendal.ReadWithRange(uint64(offset), uint64(length)))
	assert.Nil(err)
	assert.Equal(length, int64(len(bs)), "read range size")
	assert.Equal(content[offset:offset+length], bs, "read range content")
}

func testReadWithRangeFrom(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	offset, _ := genOffsetLength(size)

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	bs, err := op.Read(path, opendal.ReadWithRangeFrom(uint64(offset)))
	assert.Nil(err)
	assert.Equal(int64(size)-offset, int64(len(bs)), "read range-from size")
	assert.Equal(content[offset:], bs, "read range-from content")
}

func testReadWithContentLengthHint(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	// An accurate hint must not change the result of a full read.
	bs, err := op.Read(path, opendal.ReadWithContentLengthHint(uint64(size)))
	assert.Nil(err)
	assert.Equal(size, uint(len(bs)), "read size")
	assert.Equal(content, bs, "read content")

	// The hint is an execution hint only; it must also work combined with a range.
	offset, length := genOffsetLength(size)
	bs, err = op.Read(path,
		opendal.ReadWithRange(uint64(offset), uint64(length)),
		opendal.ReadWithContentLengthHint(uint64(size)),
	)
	assert.Nil(err)
	assert.Equal(length, int64(len(bs)), "read range size")
	assert.Equal(content[offset:offset+length], bs, "read range content")
}

func testReadWithIfMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	etag, ok := meta.ETag()
	if !ok {
		return
	}

	_, err = op.Read(path, opendal.ReadWithIfMatch("\"invalid_etag\""))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(path, opendal.ReadWithIfMatch(etag))
	assert.Nil(err, "read with matching etag must succeed")
	assert.Equal(content, bs, "read content")
}

func testReadWithIfNoneMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, _ := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	etag, ok := meta.ETag()
	if !ok {
		return
	}

	_, err = op.Read(path, opendal.ReadWithIfNoneMatch(etag))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(path, opendal.ReadWithIfNoneMatch("\"invalid_etag\""))
	assert.Nil(err, "read with non-matching etag must succeed")
	assert.Equal(content, bs, "read content")
}

func testReadFull(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	bs, err := op.Read(path)
	assert.Nil(err)
	assert.Equal(size, uint(len(bs)), "read size")
	assert.Equal(content, bs, "read content")
}

func testReader(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	r, err := op.Reader(path)
	assert.Nil(err)
	defer r.Close()
	bs := make([]byte, size)
	n, err := r.Read(bs)
	assert.Nil(err)
	assert.Equal(size, uint(n), "read size")
	assert.Equal(content, bs[:n], "read content")
}

func testReadNotExist(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewFilePath()

	_, err := op.Read(path)
	assert.NotNil(err)
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))
}

func testReadWithDirPath(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().CreateDir() {
		return
	}

	path := fixture.NewDirPath()

	assert.Nil(op.CreateDir(path), "create must succeed")

	_, err := op.Read(path)
	assert.NotNil(err)
	assert.Equal(opendal.CodeIsADirectory, assertErrorCode(err))
}

func testReadWithSpecialChars(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFileWithPath(uuid.NewString() + " !@#$%^&()_+-=;',.txt")

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	bs, err := op.Read(path)
	assert.Nil(err)
	assert.Equal(size, uint(len(bs)))
	assert.Equal(content, bs)
}

func testIOCopy(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	_, err := op.Write(path, content)
	assert.Nil(err)

	// Use a separate operator to test copying between operators.
	other, _, err := newOperator()
	assert.Nil(err)
	defer other.Close()
	for _, method := range []string{"Copy", "CopyBuffer", "ReadFrom", "WriteTo", "wrapped_reader", "wrapped_writer"} {
		r, err := op.Reader(path)
		assert.Nil(err)
		offset := int64(size / 3)
		_, err = r.Seek(offset, io.SeekStart)
		assert.Nil(err)
		pathCopy := fixture.NewFilePath()
		w, err := other.Writer(pathCopy)
		assert.Nil(err)
		_, err = w.Write([]byte("prefix"))
		assert.Nil(err)
		var n int64
		switch method {
		case "Copy":
			n, err = io.Copy(w, r)
		case "CopyBuffer":
			n, err = io.CopyBuffer(w, r, make([]byte, 7))
		case "ReadFrom":
			n, err = w.ReadFrom(r)
		case "WriteTo":
			n, err = r.WriteTo(w)
		case "wrapped_reader":
			n, err = w.ReadFrom(struct{ io.Reader }{r})
		case "wrapped_writer":
			n, err = r.WriteTo(struct{ io.Writer }{w})
		}
		assert.Nil(err, method)
		assert.Equal(int64(size)-offset, n, method)
		pos, err := r.Seek(0, io.SeekCurrent)
		assert.Nil(err)
		assert.Equal(int64(size), pos)
		n, err = w.ReadFrom(r)
		assert.Nil(err, "copying a reader already at EOF succeeds")
		assert.Zero(n)
		_, err = w.Write([]byte("tail"))
		assert.Nil(err, "copy must leave the writer open")
		assert.Nil(r.Close())
		meta, err := w.Close()
		assert.Nil(err)
		assert.NotNil(meta)
		got, err := other.Read(pathCopy)
		assert.Nil(err)
		want := append(append([]byte("prefix"), content[offset:]...), []byte("tail")...)
		assert.Equal(want, got, method)
	}
}

func testReaderSeek(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()
	offset, length := genOffsetLength(size)

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	r, err := op.Reader(path)
	assert.Nil(err)
	defer r.Close()

	pos, err := r.Seek(offset, io.SeekStart)
	assert.Nil(err, "seek must succeed")
	assert.Equal(int64(offset), pos, "seek start offset")
	bs := make([]byte, length)
	n, err := r.Read(bs)
	assert.Nil(err, "read must succeed")
	assert.Equal(length, int64(n), "read size")
	assert.Equal(content[offset:offset+length], bs[:n], "read content")

	pos, err = r.Seek(-length, io.SeekCurrent)
	assert.Nil(err, "seek must succeed")
	assert.Equal(offset, pos, "seek current offset")
	bs = make([]byte, length)
	n, err = r.Read(bs)
	assert.Nil(err, "read must succeed")
	assert.Equal(length, int64(n), "read size")
	assert.Equal(content[offset:offset+length], bs[:n], "read content")

	pos, err = r.Seek(-length, io.SeekEnd)
	assert.Nil(err, "seek must succeed")
	assert.Equal(int64(size)-length, pos, "seek end offset")
	bs = make([]byte, length)
	n, err = r.Read(bs)
	assert.Nil(err, "read must succeed")
	assert.Equal(length, int64(n), "read size")
	assert.Equal(content[int64(size)-length:size], bs[:n], "read content")
}

func testReaderWithConcurrentChunkGap(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	r, err := op.Reader(path,
		opendal.ReaderWithConcurrent(2),
		opendal.ReaderWithChunk(1024*1024),
		opendal.ReaderWithGap(4096),
		opendal.ReaderWithPrefetch(2),
	)
	assert.Nil(err)
	defer r.Close()

	bs := make([]byte, size)
	n, err := io.ReadFull(r, bs)
	assert.Nil(err)
	assert.Equal(size, uint(n), "read size")
	assert.Equal(content, bs[:n], "read content")
}

func testReaderWithIfMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	etag, ok := meta.ETag()
	if !ok {
		return
	}

	// Some backends defer the request to the first read: reader creation then
	// succeeds and the condition failure surfaces from Read(). The C reader maps
	// read-time errors to CodeUnexpected, so only the eager (creation) path
	// carries the precise CodeConditionNotMatch.
	r, err := op.Reader(path, opendal.ReaderWithIfMatch("\"invalid_etag\""))
	if err != nil {
		assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))
	} else {
		_, readErr := r.Read(make([]byte, size))
		assert.NotNil(readErr, "reader with non-matching etag must fail on read")
		assert.Nil(r.Close(), "close reader must succeed")
	}

	r, err = op.Reader(path, opendal.ReaderWithIfMatch(etag))
	assert.Nil(err, "reader with matching etag must succeed")
	defer r.Close()
	bs := make([]byte, size)
	n, err := io.ReadFull(r, bs)
	assert.Nil(err)
	assert.Equal(content, bs[:n], "read content")
}

func testReaderWithVersion(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path, content, size := fixture.NewFile()

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	meta, err := op.Stat(path)
	assert.Nil(err)
	version, ok := meta.Version()
	if !ok {
		return
	}

	r, err := op.Reader(path, opendal.ReaderWithVersion(version))
	assert.Nil(err)
	defer r.Close()
	bs := make([]byte, size)
	n, err := io.ReadFull(r, bs)
	assert.Nil(err)
	assert.Equal(content, bs[:n], "read version content")
}
