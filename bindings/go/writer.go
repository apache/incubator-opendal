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

import (
	"context"
	"errors"
	"io"
	"runtime"
	"strings"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// Write writes the given bytes to the specified path and returns the metadata
// of the written object (such as etag, version, or last modified) as reported
// by the underlying service.
//
// Write is a wrapper around the C-binding function
// `opendal_operator_write_with_metadata`. Which fields of the returned Metadata
// are populated depends on the service.
//
// # Parameters
//
//   - path: The destination path where the bytes will be written.
//   - data: The byte slice containing the data to be written.
//   - opts: Optional write options.
//
// # Returns
//
//   - *Metadata: Metadata of the written object. Which fields are populated
//     depends on the service.
//   - error: An error if the write operation fails, or nil if successful.
//
// # Example
//
//	func exampleWrite(op *opendal.Operator) {
//		_, err := op.Write("test", []byte("Hello opendal go binding!"))
//		if err != nil {
//			log.Fatal(err)
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Write(path string, data []byte, opts ...WithWriteFn) (*Metadata, error) {
	if len(opts) == 0 {
		meta, err := ffiOperatorWriteWithMetadata.symbol(op.ctx)(op.inner, path, data, nil)
		if err != nil {
			return nil, err
		}
		return newMetadata(op.ctx, meta), nil
	}

	o := parseWriteOptions(opts...)
	cOpts, keepAlive, err := newOpendalWriteOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiWriteOptionsFree.symbol(op.ctx)(cOpts)
	meta, err := ffiOperatorWriteWithMetadata.symbol(op.ctx)(op.inner, path, data, cOpts)
	runtime.KeepAlive(keepAlive)
	if err != nil {
		return nil, err
	}
	return newMetadata(op.ctx, meta), nil
}

// WithWriteFn is a functional option for write operations.
type WithWriteFn func(*writeOptions)

// WriteWithAppend sets append mode for the write operation.
func WriteWithAppend(append bool) WithWriteFn {
	return func(o *writeOptions) {
		o.append = append
	}
}

// WriteWithCacheControl sets the Cache-Control header for the write operation.
func WriteWithCacheControl(cacheControl string) WithWriteFn {
	return func(o *writeOptions) {
		o.cacheControl = cacheControl
	}
}

// WriteWithContentType sets the Content-Type header for the write operation.
func WriteWithContentType(contentType string) WithWriteFn {
	return func(o *writeOptions) {
		o.contentType = contentType
	}
}

// WriteWithContentDisposition sets the Content-Disposition header for the write operation.
func WriteWithContentDisposition(contentDisposition string) WithWriteFn {
	return func(o *writeOptions) {
		o.contentDisposition = contentDisposition
	}
}

// WriteWithContentEncoding sets the Content-Encoding header for the write operation.
func WriteWithContentEncoding(contentEncoding string) WithWriteFn {
	return func(o *writeOptions) {
		o.contentEncoding = contentEncoding
	}
}

// WriteWithUserMetadata sets user metadata for the write operation.
func WriteWithUserMetadata(userMetadata map[string]string) WithWriteFn {
	return func(o *writeOptions) {
		o.userMetadata = userMetadata
	}
}

// WriteWithIfMatch sets the If-Match condition for the write operation.
func WriteWithIfMatch(ifMatch string) WithWriteFn {
	return func(o *writeOptions) {
		o.ifMatch = ifMatch
	}
}

// WriteWithIfNoneMatch sets the If-None-Match condition for the write operation.
func WriteWithIfNoneMatch(ifNoneMatch string) WithWriteFn {
	return func(o *writeOptions) {
		o.ifNoneMatch = ifNoneMatch
	}
}

// WriteWithIfNotExists sets whether the write operation should only succeed if the target does not exist.
func WriteWithIfNotExists(ifNotExists bool) WithWriteFn {
	return func(o *writeOptions) {
		o.ifNotExists = ifNotExists
	}
}

// WriteWithConcurrent sets concurrent write operations.
func WriteWithConcurrent(concurrent uint) WithWriteFn {
	return func(o *writeOptions) {
		o.concurrent = concurrent
	}
}

// WriteWithChunk sets the chunk size for buffered writes.
func WriteWithChunk(chunk uint) WithWriteFn {
	return func(o *writeOptions) {
		o.chunk = chunk
	}
}

type writeOptions struct {
	append             bool
	cacheControl       string
	contentType        string
	contentDisposition string
	contentEncoding    string
	userMetadata       map[string]string
	ifMatch            string
	ifNoneMatch        string
	ifNotExists        bool
	concurrent         uint
	chunk              uint
}

func parseWriteOptions(opts ...WithWriteFn) *writeOptions {
	o := &writeOptions{}
	for _, opt := range opts {
		opt(o)
	}
	return o
}

type writeOptionsKeepAlive struct {
	strings      [][]byte
	userMetadata []opendalWriteUserMetadataPair
}

func newOpendalWriteOptions(ctx context.Context, o *writeOptions) (*opendalWriteOptions, writeOptionsKeepAlive, error) {
	cOpts := ffiWriteOptionsNew.symbol(ctx)()
	keepAlive := writeOptionsKeepAlive{}
	ffiWriteOptionsSetAppend.symbol(ctx)(cOpts, o.append)

	// fail frees the C-allocated options before returning
	fail := func(err error) (*opendalWriteOptions, writeOptionsKeepAlive, error) {
		ffiWriteOptionsFree.symbol(ctx)(cOpts)
		return nil, writeOptionsKeepAlive{}, err
	}

	setString := func(value string, set func(*opendalWriteOptions, string) ([]byte, error)) error {
		if value == "" {
			return nil
		}
		data, err := set(cOpts, value)
		if err != nil {
			return err
		}
		keepAlive.strings = append(keepAlive.strings, data)
		return nil
	}

	if err := setString(o.cacheControl, ffiWriteOptionsSetCacheControl.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.contentType, ffiWriteOptionsSetContentType.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.contentDisposition, ffiWriteOptionsSetContentDisposition.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.contentEncoding, ffiWriteOptionsSetContentEncoding.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifMatch, ffiWriteOptionsSetIfMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifNoneMatch, ffiWriteOptionsSetIfNoneMatch.symbol(ctx)); err != nil {
		return fail(err)
	}

	ffiWriteOptionsSetIfNotExists.symbol(ctx)(cOpts, o.ifNotExists)
	if o.concurrent != 0 {
		ffiWriteOptionsSetConcurrent.symbol(ctx)(cOpts, o.concurrent)
	}
	if o.chunk != 0 {
		ffiWriteOptionsSetChunk.symbol(ctx)(cOpts, o.chunk)
	}
	if len(o.userMetadata) > 0 {
		keepAlive.userMetadata = make([]opendalWriteUserMetadataPair, 0, len(o.userMetadata))
		for key, value := range o.userMetadata {
			keyData, err := byteSliceFromString(key)
			if err != nil {
				return fail(err)
			}
			valueData, err := byteSliceFromString(value)
			if err != nil {
				return fail(err)
			}
			keepAlive.strings = append(keepAlive.strings, keyData, valueData)
			byteKey := &keyData[0]
			byteValue := &valueData[0]
			keepAlive.userMetadata = append(keepAlive.userMetadata, opendalWriteUserMetadataPair{key: byteKey, value: byteValue})
		}
		ffiWriteOptionsSetUserMetadata.symbol(ctx)(cOpts, keepAlive.userMetadata)
	}
	return cOpts, keepAlive, nil
}

// CreateDir creates a directory at the specified path.
//
// CreateDir is a wrapper around the C-binding function `opendal_operator_create_dir`.
// It provides a way to create directories in the storage system.
//
// # Parameters
//
//   - path: The path where the directory should be created.
//
// # Returns
//
//   - error: An error if the directory creation fails, or nil if successful.
//
// # Notes
//
// It is mandatory to include a trailing slash (/) in the path to indicate
// that it is a directory. Failing to do so may result in a `CodeNotADirectory`
// error being returned by OpenDAL.
//
// # Behavior
//
//   - Creating a directory that already exists will succeed without error.
//   - Directory creation is always recursive, similar to the `mkdir -p` command.
//
// # Example
//
//	func exampleCreateDir(op *opendal.Operator) {
//		err = op.CreateDir("test/")
//		if err != nil {
//			log.Fatal(err)
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
// The trailing slash in "test/" is important to indicate it's a directory.
func (op *Operator) CreateDir(path string) error {
	return ffiOperatorCreateDir.symbol(op.ctx)(op.inner, path)
}

// Writer returns a new Writer for the specified path.
//
// Writer is a wrapper around the C-binding function `opendal_operator_writer`.
// When options are provided, it uses `opendal_operator_writer_with`.
// It provides a way to obtain a writer for writing data to the storage system.
//
// # Parameters
//
//   - path: The destination path where data will be written.
//   - opts: Optional write options.
//
// # Returns
//
//   - *Writer: A pointer to a Writer instance, or an error if the operation fails.
//
// # Example
//
//	func exampleWriter(op *opendal.Operator) {
//		writer, err := op.Writer("test/")
//		if err != nil {
//			log.Fatal(err)
//		}
//		defer writer.Close()
//		_, err = writer.Write([]byte("Hello opendal writer!"))
//		if err != nil {
//			log.Fatal(err)
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Writer(path string, opts ...WithWriteFn) (*Writer, error) {
	if len(opts) == 0 {
		inner, err := ffiOperatorWriter.symbol(op.ctx)(op.inner, path)
		if err != nil {
			return nil, err
		}
		writer := &Writer{
			inner: inner,
			ctx:   op.ctx,
		}
		return writer, nil
	}

	o := parseWriteOptions(opts...)
	cOpts, keepAlive, err := newOpendalWriteOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiWriteOptionsFree.symbol(op.ctx)(cOpts)
	inner, err := ffiOperatorWriterWith.symbol(op.ctx)(op.inner, path, cOpts)
	runtime.KeepAlive(keepAlive)
	if err != nil {
		return nil, err
	}
	writer := &Writer{
		inner: inner,
		ctx:   op.ctx,
	}
	return writer, nil
}

const copyBufferSize = 256 * 1024

type Writer struct {
	inner *opendalWriter
	ctx   context.Context
}

// Write writes the given bytes to the specified path.
//
// Write is a wrapper around the C-binding function `opendal_operator_write`. It provides a simplified
// interface for writing data to the storage. Write can be called multiple times to write
// additional data to the same path.
//
// The maximum size of the data that can be written in a single call is 256KB.
//
// # Parameters
//
//   - path: The destination path where the bytes will be written.
//   - data: The byte slice containing the data to be written.
//
// # Returns
//
//   - error: An error if the write operation fails, or nil if successful.
//
// # Example
//
//	func exampleWrite(op *opendal.Operator) {
//		_, err = op.Write("test", []byte("Hello opendal go binding!"))
//		if err != nil {
//			log.Fatal(err)
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
func (w *Writer) Write(p []byte) (n int, err error) {
	return ffiWriterWrite.symbol(w.ctx)(w.inner, p)
}

// ReadFrom copies data from src to w until EOF or an error.
// It returns the number of bytes copied. At EOF, it returns a nil error.
//
// For a *Reader, ReadFrom copies native buffers.
// The reader and writer must use the same native library.
// It reads up to one buffer ahead of the current write.
// A write error stops the copy without waiting for a pending read.
// The reader can consume more bytes than the returned count.
// The count excludes bytes from a failed buffer.
//
// For other readers, ReadFrom uses one 256 KiB Go buffer per call.
// ReadFrom leaves both streams open. Call [Writer.Close] to complete the write
// and get its metadata.
func (w *Writer) ReadFrom(src io.Reader) (int64, error) {
	if r, ok := src.(*Reader); ok {
		return ffiWriterReadFrom.symbol(w.ctx)(w.inner, r.inner)
	}

	buf := make([]byte, copyBufferSize)

	// Hide WriteTo and ReadFrom to prevent recursive calls from io.CopyBuffer.
	reader := struct{ io.Reader }{Reader: src}
	writer := struct{ io.Writer }{Writer: w}
	return io.CopyBuffer(writer, reader, buf)
}

// Close finishes the write and releases the resources associated with the
// Writer, returning the metadata of the written object (such as etag, version,
// or last modified) as reported by the underlying service.
//
// It is important to call Close after writing all the data to ensure that the
// data is properly flushed and written to the storage. Otherwise, the data may
// be lost.
//
// Close is a wrapper around the C-binding function
// `opendal_writer_close_with_metadata`. Which fields of the returned Metadata
// are populated depends on the service.
func (w *Writer) Close() (*Metadata, error) {
	defer ffiWriterFree.symbol(w.ctx)(w.inner)
	meta, err := ffiWriterCloseWithMetadata.symbol(w.ctx)(w.inner)
	if err != nil {
		return nil, err
	}
	return newMetadata(w.ctx, meta), nil
}

var _ io.Writer = (*Writer)(nil)
var _ io.ReaderFrom = (*Writer)(nil)

var ffiOperatorWriteWithMetadata = newFFI(ffiOpts{
	sym:    "opendal_operator_write_with_metadata",
	rType:  &typeResultWrite,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, data []byte, opts *opendalWriteOptions) (*opendalMetadata, error) {
	return func(op *opendalOperator, path string, data []byte, opts *opendalWriteOptions) (*opendalMetadata, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		bytes := toOpendalBytes(data)
		var result resultWrite
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&bytes),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.meta, nil
	}
})

var ffiOperatorCreateDir = newFFI(ffiOpts{
	sym:    "opendal_operator_create_dir",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string) error {
	return func(op *opendalOperator, path string) error {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return err
		}
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
		)
		return parseError(ctx, e)
	}
})

var ffiOperatorWriter = newFFI(ffiOpts{
	sym:    "opendal_operator_writer",
	rType:  &typeResultOperatorWriter,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string) (*opendalWriter, error) {
	return func(op *opendalOperator, path string) (*opendalWriter, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultOperatorWriter
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.writer, nil
	}
})

var ffiOperatorWriterWith = newFFI(ffiOpts{
	sym:    "opendal_operator_writer_with",
	rType:  &typeResultOperatorWriter,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, opts *opendalWriteOptions) (*opendalWriter, error) {
	return func(op *opendalOperator, path string, opts *opendalWriteOptions) (*opendalWriter, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultOperatorWriter
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.writer, nil
	}
})

var ffiWriteOptionsNew = newFFI(ffiOpts{
	sym:   "opendal_write_options_new",
	rType: &ffi.TypePointer,
}, func(_ context.Context, ffiCall ffiCall) func() *opendalWriteOptions {
	return func() *opendalWriteOptions {
		var opts *opendalWriteOptions
		ffiCall(unsafe.Pointer(&opts))
		return opts
	}
})

var ffiWriteOptionsFree = newFFI(ffiOpts{
	sym:    "opendal_write_options_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalWriteOptions) {
	return func(opts *opendalWriteOptions) {
		ffiCall(
			nil,
			unsafe.Pointer(&opts),
		)
	}
})

var ffiWriteOptionsSetAppend = newFFI(ffiOpts{
	sym:    "opendal_write_options_set_append",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint8},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalWriteOptions, append bool) {
	return func(opts *opendalWriteOptions, append bool) {
		var v uint8
		if append {
			v = 1
		}
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&v))
	}
})

func byteSliceFromString(value string) ([]byte, error) {
	if strings.IndexByte(value, 0) >= 0 {
		return nil, errors.New("string contains nul")
	}
	return append([]byte(value), 0), nil
}

func newWriteOptionsSetStringFFI(sym string) *FFI[func(*opendalWriteOptions, string) ([]byte, error)] {
	return newFFI(ffiOpts{
		sym:    contextKey(sym),
		rType:  &ffi.TypeVoid,
		aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
	}, func(_ context.Context, ffiCall ffiCall) func(*opendalWriteOptions, string) ([]byte, error) {
		return func(opts *opendalWriteOptions, value string) ([]byte, error) {
			data, err := byteSliceFromString(value)
			if err != nil {
				return nil, err
			}
			byteValue := &data[0]
			ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&byteValue))
			return data, nil
		}
	})
}

var ffiWriteOptionsSetCacheControl = newWriteOptionsSetStringFFI("opendal_write_options_set_cache_control")
var ffiWriteOptionsSetContentType = newWriteOptionsSetStringFFI("opendal_write_options_set_content_type")
var ffiWriteOptionsSetContentDisposition = newWriteOptionsSetStringFFI("opendal_write_options_set_content_disposition")
var ffiWriteOptionsSetContentEncoding = newWriteOptionsSetStringFFI("opendal_write_options_set_content_encoding")
var ffiWriteOptionsSetIfMatch = newWriteOptionsSetStringFFI("opendal_write_options_set_if_match")
var ffiWriteOptionsSetIfNoneMatch = newWriteOptionsSetStringFFI("opendal_write_options_set_if_none_match")

var ffiWriteOptionsSetIfNotExists = newFFI(ffiOpts{
	sym:    "opendal_write_options_set_if_not_exists",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint8},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalWriteOptions, ifNotExists bool) {
	return func(opts *opendalWriteOptions, ifNotExists bool) {
		var v uint8
		if ifNotExists {
			v = 1
		}
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&v))
	}
})

var ffiWriteOptionsSetConcurrent = newFFI(ffiOpts{
	sym:    "opendal_write_options_set_concurrent",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalWriteOptions, concurrent uint) {
	return func(opts *opendalWriteOptions, concurrent uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&concurrent))
	}
})

var ffiWriteOptionsSetChunk = newFFI(ffiOpts{
	sym:    "opendal_write_options_set_chunk",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalWriteOptions, chunk uint) {
	return func(opts *opendalWriteOptions, chunk uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&chunk))
	}
})

var ffiWriteOptionsSetUserMetadata = newFFI(ffiOpts{
	sym:    "opendal_write_options_set_user_metadata",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalWriteOptions, userMetadata []opendalWriteUserMetadataPair) {
	return func(opts *opendalWriteOptions, userMetadata []opendalWriteUserMetadataPair) {
		var ptr *opendalWriteUserMetadataPair
		if len(userMetadata) > 0 {
			ptr = &userMetadata[0]
		}
		length := uint(len(userMetadata))
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&ptr), unsafe.Pointer(&length))
	}
})

var ffiWriterFree = newFFI(ffiOpts{
	sym:    "opendal_writer_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(w *opendalWriter) {
	return func(w *opendalWriter) {
		ffiCall(
			nil,
			unsafe.Pointer(&w),
		)
	}
})

var ffiWriterWrite = newFFI(ffiOpts{
	sym:    "opendal_writer_write",
	rType:  &typeResultWriterWrite,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(r *opendalWriter, buf []byte) (size int, err error) {
	return func(r *opendalWriter, buf []byte) (size int, err error) {
		bytes := toOpendalBytes(buf)
		var result resultWriterWrite
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&r),
			unsafe.Pointer(&bytes),
		)
		if result.error != nil {
			return 0, parseError(ctx, result.error)
		}
		return int(result.size), nil
	}
})

var ffiWriterReadFrom = newFFI(ffiOpts{
	sym:    "opendal_writer_read_from",
	rType:  &typeResultStreamCopy,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(*opendalWriter, *opendalReader) (int64, error) {
	return func(writer *opendalWriter, reader *opendalReader) (int64, error) {
		var result resultStreamCopy
		ffiCall(unsafe.Pointer(&result), unsafe.Pointer(&writer), unsafe.Pointer(&reader))
		return int64(result.size), parseError(ctx, result.error)
	}
})

var ffiWriterCloseWithMetadata = newFFI(ffiOpts{
	sym:    "opendal_writer_close_with_metadata",
	rType:  &typeResultWrite,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(r *opendalWriter) (*opendalMetadata, error) {
	return func(r *opendalWriter) (*opendalMetadata, error) {
		var result resultWrite
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&r),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.meta, nil
	}
})
