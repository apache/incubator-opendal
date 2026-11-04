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
	"io"
	"runtime"
	"time"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// Read reads the entire contents of the file at the specified path into a byte slice.
//
// Read is a wrapper around the C-binding function `opendal_operator_read`.
// When options are provided, it uses `opendal_operator_read_with`.
//
// # Parameters
//
//   - path: The path of the file to read.
//   - opts: Optional read options.
//
// # Returns
//
//   - []byte: The contents of the file as a byte slice.
//   - error: An error if the read operation fails, or nil if successful.
//
// # Notes
//
//   - Read allocates a new byte slice internally. For more precise memory control
//     or lazy reading, consider using the Reader() method instead.
//
// # Example
//
//	func exampleRead(op *opendal.Operator) {
//		data, err := op.Read("test")
//		if err != nil {
//			log.Fatal(err)
//		}
//		fmt.Printf("Read: %s\n", data)
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Read(path string, opts ...WithReadFn) ([]byte, error) {
	bytes, err := op.read(path, opts...)
	if err != nil {
		return nil, err
	}

	data := parseBytes(bytes)
	if len(data) > 0 {
		ffiBytesFree.symbol(op.ctx)(&bytes)
	}
	return data, nil
}

func (op *Operator) read(path string, opts ...WithReadFn) (opendalBytes, error) {
	if len(opts) == 0 {
		return ffiOperatorRead.symbol(op.ctx)(op.inner, path)
	}

	o := parseReadOptions(opts...)
	cOpts, keepAlive, err := newOpendalReadOptions(op.ctx, o)
	if err != nil {
		return opendalBytes{}, err
	}
	defer ffiReadOptionsFree.symbol(op.ctx)(cOpts)
	bytes, err := ffiOperatorReadWith.symbol(op.ctx)(op.inner, path, cOpts)
	runtime.KeepAlive(keepAlive)
	return bytes, err
}

// WithReadFn is a functional option for read operations.
type WithReadFn func(*readOptions)

// ReadWithRange sets the byte range to read, starting at offset and reading
// length bytes. To read a file with size n, offset must be in [0, n) and the
// effective range is [offset, offset+length).
func ReadWithRange(offset, length uint64) WithReadFn {
	return func(o *readOptions) {
		o.hasRange = true
		o.hasRangeLength = true
		o.rangeOffset = offset
		o.rangeLength = length
	}
}

// ReadWithRangeFrom sets the byte range to start at offset and read until the
// end of the file, i.e. the range [offset, n) for a file of size n.
func ReadWithRangeFrom(offset uint64) WithReadFn {
	return func(o *readOptions) {
		o.hasRange = true
		o.hasRangeLength = false
		o.rangeOffset = offset
		o.rangeLength = 0
	}
}

// ReadWithVersion sets the version of the object to read.
func ReadWithVersion(version string) WithReadFn {
	return func(o *readOptions) {
		o.version = version
	}
}

// ReadWithIfMatch sets the If-Match condition for the read operation.
func ReadWithIfMatch(ifMatch string) WithReadFn {
	return func(o *readOptions) {
		o.ifMatch = ifMatch
	}
}

// ReadWithIfNoneMatch sets the If-None-Match condition for the read operation.
func ReadWithIfNoneMatch(ifNoneMatch string) WithReadFn {
	return func(o *readOptions) {
		o.ifNoneMatch = ifNoneMatch
	}
}

// ReadWithIfModifiedSince sets the If-Modified-Since condition for the read operation.
func ReadWithIfModifiedSince(t time.Time) WithReadFn {
	return func(o *readOptions) {
		millis := t.UnixMilli()
		o.ifModifiedSince = &millis
	}
}

// ReadWithIfUnmodifiedSince sets the If-Unmodified-Since condition for the read operation.
func ReadWithIfUnmodifiedSince(t time.Time) WithReadFn {
	return func(o *readOptions) {
		millis := t.UnixMilli()
		o.ifUnmodifiedSince = &millis
	}
}

// ReadWithConcurrent sets the number of concurrent read tasks.
func ReadWithConcurrent(concurrent uint) WithReadFn {
	return func(o *readOptions) {
		o.concurrent = concurrent
	}
}

// ReadWithChunk sets the chunk size for each read request.
func ReadWithChunk(chunk uint) WithReadFn {
	return func(o *readOptions) {
		o.chunk = chunk
	}
}

// ReadWithGap sets the gap size for merging nearby range reads.
func ReadWithGap(gap uint) WithReadFn {
	return func(o *readOptions) {
		o.gap = gap
	}
}

// ReadWithContentLengthHint sets the known content length of the object.
//
// This is an execution hint that allows OpenDAL to avoid extra metadata
// requests while planning reads. It must not be used as an object identity
// or consistency condition.
func ReadWithContentLengthHint(length uint64) WithReadFn {
	return func(o *readOptions) {
		o.contentLengthHint = &length
	}
}

// ReadWithOverrideContentType sets the Content-Type to send back (presign only).
func ReadWithOverrideContentType(contentType string) WithReadFn {
	return func(o *readOptions) {
		o.overrideContentType = contentType
	}
}

// ReadWithOverrideCacheControl sets the Cache-Control to send back (presign only).
func ReadWithOverrideCacheControl(cacheControl string) WithReadFn {
	return func(o *readOptions) {
		o.overrideCacheControl = cacheControl
	}
}

// ReadWithOverrideContentDisposition sets the Content-Disposition to send back (presign only).
func ReadWithOverrideContentDisposition(contentDisposition string) WithReadFn {
	return func(o *readOptions) {
		o.overrideContentDisposition = contentDisposition
	}
}

type readOptions struct {
	hasRange                   bool
	hasRangeLength             bool
	rangeOffset                uint64
	rangeLength                uint64
	version                    string
	ifMatch                    string
	ifNoneMatch                string
	ifModifiedSince            *int64
	ifUnmodifiedSince          *int64
	concurrent                 uint
	chunk                      uint
	gap                        uint
	contentLengthHint          *uint64
	overrideContentType        string
	overrideCacheControl       string
	overrideContentDisposition string
}

func parseReadOptions(opts ...WithReadFn) *readOptions {
	o := &readOptions{}
	for _, opt := range opts {
		opt(o)
	}
	return o
}

type readOptionsKeepAlive struct {
	strings [][]byte
}

func newOpendalReadOptions(ctx context.Context, o *readOptions) (*opendalReadOptions, readOptionsKeepAlive, error) {
	cOpts := ffiReadOptionsNew.symbol(ctx)()
	keepAlive := readOptionsKeepAlive{}

	// fail frees the C-allocated options before returning
	fail := func(err error) (*opendalReadOptions, readOptionsKeepAlive, error) {
		ffiReadOptionsFree.symbol(ctx)(cOpts)
		return nil, readOptionsKeepAlive{}, err
	}

	setString := func(value string, set func(*opendalReadOptions, string) ([]byte, error)) error {
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

	if o.hasRange {
		if o.hasRangeLength {
			ffiReadOptionsSetRange.symbol(ctx)(cOpts, o.rangeOffset, o.rangeLength)
		} else {
			ffiReadOptionsSetRangeFrom.symbol(ctx)(cOpts, o.rangeOffset)
		}
	}
	if err := setString(o.version, ffiReadOptionsSetVersion.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifMatch, ffiReadOptionsSetIfMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifNoneMatch, ffiReadOptionsSetIfNoneMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if o.ifModifiedSince != nil {
		ffiReadOptionsSetIfModifiedSince.symbol(ctx)(cOpts, *o.ifModifiedSince)
	}
	if o.ifUnmodifiedSince != nil {
		ffiReadOptionsSetIfUnmodifiedSince.symbol(ctx)(cOpts, *o.ifUnmodifiedSince)
	}
	if o.concurrent != 0 {
		ffiReadOptionsSetConcurrent.symbol(ctx)(cOpts, o.concurrent)
	}
	if o.chunk != 0 {
		ffiReadOptionsSetChunk.symbol(ctx)(cOpts, o.chunk)
	}
	if o.gap != 0 {
		ffiReadOptionsSetGap.symbol(ctx)(cOpts, o.gap)
	}
	if o.contentLengthHint != nil {
		ffiReadOptionsSetContentLengthHint.symbol(ctx)(cOpts, *o.contentLengthHint)
	}
	if err := setString(o.overrideContentType, ffiReadOptionsSetOverrideContentType.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.overrideCacheControl, ffiReadOptionsSetOverrideCacheControl.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.overrideContentDisposition, ffiReadOptionsSetOverrideContentDisposition.symbol(ctx)); err != nil {
		return fail(err)
	}

	return cOpts, keepAlive, nil
}

// WithReaderFn is a functional option for creating a Reader.
type WithReaderFn func(*readerOptions)

// ReaderWithVersion sets the version of the object to read.
func ReaderWithVersion(version string) WithReaderFn {
	return func(o *readerOptions) {
		o.version = version
	}
}

// ReaderWithIfMatch sets the If-Match condition for the reader.
func ReaderWithIfMatch(ifMatch string) WithReaderFn {
	return func(o *readerOptions) {
		o.ifMatch = ifMatch
	}
}

// ReaderWithIfNoneMatch sets the If-None-Match condition for the reader.
func ReaderWithIfNoneMatch(ifNoneMatch string) WithReaderFn {
	return func(o *readerOptions) {
		o.ifNoneMatch = ifNoneMatch
	}
}

// ReaderWithIfModifiedSince sets the If-Modified-Since condition for the reader.
func ReaderWithIfModifiedSince(t time.Time) WithReaderFn {
	return func(o *readerOptions) {
		millis := t.UnixMilli()
		o.ifModifiedSince = &millis
	}
}

// ReaderWithIfUnmodifiedSince sets the If-Unmodified-Since condition for the reader.
func ReaderWithIfUnmodifiedSince(t time.Time) WithReaderFn {
	return func(o *readerOptions) {
		millis := t.UnixMilli()
		o.ifUnmodifiedSince = &millis
	}
}

// ReaderWithContentLengthHint sets the known content length of the object.
//
// This is an execution hint that allows OpenDAL to avoid extra metadata
// requests while planning reads. It must not be used as an object identity
// or consistency condition.
func ReaderWithContentLengthHint(length uint64) WithReaderFn {
	return func(o *readerOptions) {
		o.contentLengthHint = &length
	}
}

// ReaderWithConcurrent sets the number of concurrent read tasks.
func ReaderWithConcurrent(concurrent uint) WithReaderFn {
	return func(o *readerOptions) {
		o.concurrent = concurrent
	}
}

// ReaderWithChunk sets the chunk size for each read request.
func ReaderWithChunk(chunk uint) WithReaderFn {
	return func(o *readerOptions) {
		o.chunk = chunk
	}
}

// ReaderWithGap sets the gap size for merging nearby range reads.
func ReaderWithGap(gap uint) WithReaderFn {
	return func(o *readerOptions) {
		o.gap = gap
	}
}

// ReaderWithPrefetch sets the number of prefetched byte ranges buffered during concurrent reads.
func ReaderWithPrefetch(prefetch uint) WithReaderFn {
	return func(o *readerOptions) {
		o.prefetch = prefetch
	}
}

type readerOptions struct {
	version           string
	ifMatch           string
	ifNoneMatch       string
	ifModifiedSince   *int64
	ifUnmodifiedSince *int64
	contentLengthHint *uint64
	concurrent        uint
	chunk             uint
	gap               uint
	prefetch          uint
}

func parseReaderOptions(opts ...WithReaderFn) *readerOptions {
	o := &readerOptions{}
	for _, opt := range opts {
		opt(o)
	}
	return o
}

func newOpendalReaderOptions(ctx context.Context, o *readerOptions) (*opendalReaderOptions, readOptionsKeepAlive, error) {
	cOpts := ffiReaderOptionsNew.symbol(ctx)()
	keepAlive := readOptionsKeepAlive{}

	// fail frees the C-allocated options before returning
	fail := func(err error) (*opendalReaderOptions, readOptionsKeepAlive, error) {
		ffiReaderOptionsFree.symbol(ctx)(cOpts)
		return nil, readOptionsKeepAlive{}, err
	}

	setString := func(value string, set func(*opendalReaderOptions, string) ([]byte, error)) error {
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

	if err := setString(o.version, ffiReaderOptionsSetVersion.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifMatch, ffiReaderOptionsSetIfMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifNoneMatch, ffiReaderOptionsSetIfNoneMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if o.ifModifiedSince != nil {
		ffiReaderOptionsSetIfModifiedSince.symbol(ctx)(cOpts, *o.ifModifiedSince)
	}
	if o.ifUnmodifiedSince != nil {
		ffiReaderOptionsSetIfUnmodifiedSince.symbol(ctx)(cOpts, *o.ifUnmodifiedSince)
	}
	if o.contentLengthHint != nil {
		ffiReaderOptionsSetContentLengthHint.symbol(ctx)(cOpts, *o.contentLengthHint)
	}
	if o.concurrent != 0 {
		ffiReaderOptionsSetConcurrent.symbol(ctx)(cOpts, o.concurrent)
	}
	if o.chunk != 0 {
		ffiReaderOptionsSetChunk.symbol(ctx)(cOpts, o.chunk)
	}
	if o.gap != 0 {
		ffiReaderOptionsSetGap.symbol(ctx)(cOpts, o.gap)
	}
	if o.prefetch != 0 {
		ffiReaderOptionsSetPrefetch.symbol(ctx)(cOpts, o.prefetch)
	}

	return cOpts, keepAlive, nil
}

// Reader creates a new Reader for reading the contents of a file at the specified path.
//
// Reader is a wrapper around the C-binding function `opendal_operator_reader`.
// When options are provided, it uses `opendal_operator_reader_with`.
//
// # Parameters
//
//   - path: The path of the file to read.
//   - opts: Optional reader options such as conditional headers, version or
//     concurrency tuning.
//
// # Returns
//
//   - *Reader: A reader for accessing the file's contents. It implements `io.ReadCloser`.
//   - error: An error if the reader creation fails, or nil if successful.
//
// # Notes
//
//   - The returned reader allows for more controlled and efficient reading of large files.
//
// # Example
//
//	func exampleReader(op *opendal.Operator) {
//		r, err := op.Reader("path/to/file")
//		if err != nil {
//			log.Fatal(err)
//		}
//		defer r.Close()
//
//		size := 1024 // Read 1KB at a time
//		buffer := make([]byte, size)
//
//		for {
//			n, err := r.Read(buffer)
//			if err != nil {
//				log.Fatal(err)
//			}
//			fmt.Printf("Read %d bytes: %s\n", n, buffer[:n])
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Reader(path string, opts ...WithReaderFn) (*Reader, error) {
	if len(opts) == 0 {
		inner, err := ffiOperatorReader.symbol(op.ctx)(op.inner, path)
		if err != nil {
			return nil, err
		}
		return &Reader{inner: inner, ctx: op.ctx}, nil
	}

	o := parseReaderOptions(opts...)
	cOpts, keepAlive, err := newOpendalReaderOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiReaderOptionsFree.symbol(op.ctx)(cOpts)
	inner, err := ffiOperatorReaderWith.symbol(op.ctx)(op.inner, path, cOpts)
	runtime.KeepAlive(keepAlive)
	if err != nil {
		return nil, err
	}
	return &Reader{inner: inner, ctx: op.ctx}, nil
}

type Reader struct {
	inner *opendalReader
	ctx   context.Context
}

var _ io.ReadSeekCloser = (*Reader)(nil)
var _ io.WriterTo = (*Reader)(nil)

// Read reads data from the underlying storage into the provided buffer.
//
// This method implements the io.Reader interface for OperatorReader.
//
// # Parameters
//
//   - buf: A pre-allocated byte slice where the read data will be stored.
//     The length of buf determines the maximum number of bytes to read.
//
// # Returns
//
//   - int: The number of bytes read. Returns 0 if no data is available or the end of the file is reached.
//   - error: An error if the read operation fails, or nil if successful.
//     Note that this method does not return io.EOF; it returns nil at the end of the file.
//
// # Notes
//
//   - The caller is responsible for pre-allocating the buffer and determining its size.
//
// # Example
//
//	reader, err := op.Reader("path/to/file")
//	if err != nil {
//		log.Fatal(err)
//	}
//	defer reader.Close()
//
//	buf := make([]byte, 1024)
//	for {
//		n, err := reader.Read(buf)
//		if err != nil {
//			log.Fatal(err)
//		}
//		if n == 0 {
//			break // End of file
//		}
//		// Process buf[:n]
//	}
//
// Note: Always check the number of bytes read (n) as it may be less than len(buf).
func (r *Reader) Read(buf []byte) (int, error) {
	length := uint(len(buf))
	if length == 0 {
		return 0, nil
	}
	read := ffiReaderRead.symbol(r.ctx)
	var (
		totalSize uint
		size      uint
		err       error
	)
	for {
		size, err = read(r.inner, buf[totalSize:])
		totalSize += size
		if size == 0 || err != nil || totalSize >= length {
			break
		}
	}
	if totalSize == 0 && err == nil {
		err = io.EOF
	}
	return int(totalSize), err
}

// Seek sets the offset for the next Read operation on the reader.
//
// This method implements the io.Seeker interface for Reader.
//
// # Parameters
//
//   - offset: The offset from the origin (specified by whence).
//   - whence: The reference point for offset. Can be:
//   - io.SeekStart (0): Relative to the start of the file
//   - io.SeekCurrent (1): Relative to the current position
//   - io.SeekEnd (2): Relative to the end of the file
//
// # Returns
//
//   - int64: The new absolute position in the file after the seek operation.
//   - error: An error if the seek operation fails, or nil if successful.
//
// # Example
//
//	reader, err := op.Reader("path/to/file")
//	if err != nil {
//		log.Fatal(err)
//	}
//	defer reader.Close()
//
//	// Seek to the middle of the file
//	pos, err := reader.Seek(1000, io.SeekStart)
//	if err != nil {
//		log.Fatal(err)
//	}
//	fmt.Printf("New position: %d\n", pos)
//
//	// Seek relative to current position
//	pos, err = reader.Seek(100, io.SeekCurrent)
//	if err != nil {
//		log.Fatal(err)
//	}
//	fmt.Printf("New position: %d\n", pos)
//
// Note: The actual new position may differ from the requested position
// if the underlying storage system has restrictions on seeking.
func (r *Reader) Seek(offset int64, whence int) (int64, error) {
	return ffiReaderSeek.symbol(r.ctx)(r.inner, offset, whence)
}

// WriteTo copies data from r to dst until EOF or an error.
// It returns the number of bytes copied. At EOF, it returns a nil error.
//
// For a *Writer, WriteTo copies native buffers.
// The reader and writer must use the same native library.
// It reads up to one buffer ahead of the current write.
// A write error stops the copy without waiting for a pending read.
// The reader can consume more bytes than the returned count.
// The count excludes bytes from a failed buffer.
//
// For other writers, WriteTo uses one 256 KiB Go buffer per call.
// WriteTo leaves both streams open.
func (r *Reader) WriteTo(dst io.Writer) (int64, error) {
	if w, ok := dst.(*Writer); ok {
		return ffiReaderWriteTo.symbol(r.ctx)(r.inner, w.inner)
	}

	buf := make([]byte, copyBufferSize)

	// Hide WriteTo and ReadFrom to prevent recursive calls from io.CopyBuffer.
	reader := struct{ io.Reader }{Reader: r}
	writer := struct{ io.Writer }{Writer: dst}
	return io.CopyBuffer(writer, reader, buf)
}

// Close releases resources associated with the OperatorReader.
func (r *Reader) Close() error {
	ffiReaderFree.symbol(r.ctx)(r.inner)
	return nil
}

var ffiOperatorRead = newFFI(ffiOpts{
	sym:    "opendal_operator_read",
	rType:  &typeResultRead,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string) (opendalBytes, error) {
	return func(op *opendalOperator, path string) (opendalBytes, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return opendalBytes{}, err
		}
		var result resultRead
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
		)
		return result.data, parseError(ctx, result.error)
	}
})

var ffiOperatorReadWith = newFFI(ffiOpts{
	sym:    "opendal_operator_read_with",
	rType:  &typeResultRead,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, opts *opendalReadOptions) (opendalBytes, error) {
	return func(op *opendalOperator, path string, opts *opendalReadOptions) (opendalBytes, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return opendalBytes{}, err
		}
		var result resultRead
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&opts),
		)
		return result.data, parseError(ctx, result.error)
	}
})

var ffiReadOptionsNew = newFFI(ffiOpts{
	sym:   "opendal_read_options_new",
	rType: &ffi.TypePointer,
}, func(_ context.Context, ffiCall ffiCall) func() *opendalReadOptions {
	return func() *opendalReadOptions {
		var opts *opendalReadOptions
		ffiCall(unsafe.Pointer(&opts))
		return opts
	}
})

var ffiReadOptionsFree = newFFI(ffiOpts{
	sym:    "opendal_read_options_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions) {
	return func(opts *opendalReadOptions) {
		ffiCall(nil, unsafe.Pointer(&opts))
	}
})

var ffiReadOptionsSetRange = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_range",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint64, &ffi.TypeUint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, offset, length uint64) {
	return func(opts *opendalReadOptions, offset, length uint64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&offset), unsafe.Pointer(&length))
	}
})

var ffiReadOptionsSetRangeFrom = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_range_from",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, offset uint64) {
	return func(opts *opendalReadOptions, offset uint64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&offset))
	}
})

func newReadOptionsSetStringFFI(sym string) *FFI[func(*opendalReadOptions, string) ([]byte, error)] {
	return newFFI(ffiOpts{
		sym:    contextKey(sym),
		rType:  &ffi.TypeVoid,
		aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
	}, func(_ context.Context, ffiCall ffiCall) func(*opendalReadOptions, string) ([]byte, error) {
		return func(opts *opendalReadOptions, value string) ([]byte, error) {
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

var ffiReadOptionsSetVersion = newReadOptionsSetStringFFI("opendal_read_options_set_version")
var ffiReadOptionsSetIfMatch = newReadOptionsSetStringFFI("opendal_read_options_set_if_match")
var ffiReadOptionsSetIfNoneMatch = newReadOptionsSetStringFFI("opendal_read_options_set_if_none_match")
var ffiReadOptionsSetOverrideContentType = newReadOptionsSetStringFFI("opendal_read_options_set_override_content_type")
var ffiReadOptionsSetOverrideCacheControl = newReadOptionsSetStringFFI("opendal_read_options_set_override_cache_control")
var ffiReadOptionsSetOverrideContentDisposition = newReadOptionsSetStringFFI("opendal_read_options_set_override_content_disposition")

var ffiReadOptionsSetIfModifiedSince = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_if_modified_since",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeSint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, millis int64) {
	return func(opts *opendalReadOptions, millis int64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&millis))
	}
})

var ffiReadOptionsSetIfUnmodifiedSince = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_if_unmodified_since",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeSint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, millis int64) {
	return func(opts *opendalReadOptions, millis int64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&millis))
	}
})

var ffiReadOptionsSetConcurrent = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_concurrent",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, concurrent uint) {
	return func(opts *opendalReadOptions, concurrent uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&concurrent))
	}
})

var ffiReadOptionsSetChunk = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_chunk",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, chunk uint) {
	return func(opts *opendalReadOptions, chunk uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&chunk))
	}
})

var ffiReadOptionsSetGap = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_gap",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, gap uint) {
	return func(opts *opendalReadOptions, gap uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&gap))
	}
})

var ffiReadOptionsSetContentLengthHint = newFFI(ffiOpts{
	sym:    "opendal_read_options_set_content_length_hint",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReadOptions, length uint64) {
	return func(opts *opendalReadOptions, length uint64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&length))
	}
})

var ffiOperatorReader = newFFI(ffiOpts{
	sym:    "opendal_operator_reader",
	rType:  &typeResultOperatorReader,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string) (*opendalReader, error) {
	return func(op *opendalOperator, path string) (*opendalReader, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultOperatorReader
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.reader, nil
	}
})

var ffiOperatorReaderWith = newFFI(ffiOpts{
	sym:    "opendal_operator_reader_with",
	rType:  &typeResultOperatorReader,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, opts *opendalReaderOptions) (*opendalReader, error) {
	return func(op *opendalOperator, path string, opts *opendalReaderOptions) (*opendalReader, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultOperatorReader
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.reader, nil
	}
})

var ffiReaderOptionsNew = newFFI(ffiOpts{
	sym:   "opendal_reader_options_new",
	rType: &ffi.TypePointer,
}, func(_ context.Context, ffiCall ffiCall) func() *opendalReaderOptions {
	return func() *opendalReaderOptions {
		var opts *opendalReaderOptions
		ffiCall(unsafe.Pointer(&opts))
		return opts
	}
})

var ffiReaderOptionsFree = newFFI(ffiOpts{
	sym:    "opendal_reader_options_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions) {
	return func(opts *opendalReaderOptions) {
		ffiCall(nil, unsafe.Pointer(&opts))
	}
})

func newReaderOptionsSetStringFFI(sym string) *FFI[func(*opendalReaderOptions, string) ([]byte, error)] {
	return newFFI(ffiOpts{
		sym:    contextKey(sym),
		rType:  &ffi.TypeVoid,
		aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
	}, func(_ context.Context, ffiCall ffiCall) func(*opendalReaderOptions, string) ([]byte, error) {
		return func(opts *opendalReaderOptions, value string) ([]byte, error) {
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

var ffiReaderOptionsSetVersion = newReaderOptionsSetStringFFI("opendal_reader_options_set_version")
var ffiReaderOptionsSetIfMatch = newReaderOptionsSetStringFFI("opendal_reader_options_set_if_match")
var ffiReaderOptionsSetIfNoneMatch = newReaderOptionsSetStringFFI("opendal_reader_options_set_if_none_match")

var ffiReaderOptionsSetIfModifiedSince = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_if_modified_since",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeSint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, millis int64) {
	return func(opts *opendalReaderOptions, millis int64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&millis))
	}
})

var ffiReaderOptionsSetIfUnmodifiedSince = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_if_unmodified_since",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeSint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, millis int64) {
	return func(opts *opendalReaderOptions, millis int64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&millis))
	}
})

var ffiReaderOptionsSetContentLengthHint = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_content_length_hint",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeUint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, length uint64) {
	return func(opts *opendalReaderOptions, length uint64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&length))
	}
})

var ffiReaderOptionsSetConcurrent = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_concurrent",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, concurrent uint) {
	return func(opts *opendalReaderOptions, concurrent uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&concurrent))
	}
})

var ffiReaderOptionsSetChunk = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_chunk",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, chunk uint) {
	return func(opts *opendalReaderOptions, chunk uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&chunk))
	}
})

var ffiReaderOptionsSetGap = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_gap",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, gap uint) {
	return func(opts *opendalReaderOptions, gap uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&gap))
	}
})

var ffiReaderOptionsSetPrefetch = newFFI(ffiOpts{
	sym:    "opendal_reader_options_set_prefetch",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalReaderOptions, prefetch uint) {
	return func(opts *opendalReaderOptions, prefetch uint) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&prefetch))
	}
})

var ffiReaderFree = newFFI(ffiOpts{
	sym:    "opendal_reader_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(r *opendalReader) {
	return func(r *opendalReader) {
		ffiCall(
			nil,
			unsafe.Pointer(&r),
		)
	}
})

var ffiReaderRead = newFFI(ffiOpts{
	sym:    "opendal_reader_read",
	rType:  &typeResultReaderRead,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(r *opendalReader, buf []byte) (size uint, err error) {
	return func(r *opendalReader, buf []byte) (size uint, err error) {
		var length = len(buf)
		if length == 0 {
			return 0, nil
		}
		bytePtr := &buf[0]
		var result resultReaderRead
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&r),
			unsafe.Pointer(&bytePtr),
			unsafe.Pointer(&length),
		)
		if result.error != nil {
			return 0, parseError(ctx, result.error)
		}
		return result.size, nil
	}
})

var ffiReaderWriteTo = newFFI(ffiOpts{
	sym:    "opendal_reader_write_to",
	rType:  &typeResultStreamCopy,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(*opendalReader, *opendalWriter) (int64, error) {
	return func(reader *opendalReader, writer *opendalWriter) (int64, error) {
		var result resultStreamCopy
		ffiCall(unsafe.Pointer(&result), unsafe.Pointer(&reader), unsafe.Pointer(&writer))
		return int64(result.size), parseError(ctx, result.error)
	}
})

var ffiReaderSeek = newFFI(ffiOpts{
	sym:    "opendal_reader_seek",
	rType:  &typeResultReaderSeek,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(r *opendalReader, offset int64, whence int) (int64, error) {
	return func(r *opendalReader, offset int64, whence int) (int64, error) {
		var result resultReaderSeek
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&r),
			unsafe.Pointer(&offset),
			unsafe.Pointer(&whence),
		)
		if result.error != nil {
			return 0, parseError(ctx, result.error)
		}
		return int64(result.pos), nil
	}
})
