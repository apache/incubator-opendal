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
	"runtime"
	"time"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// Stat retrieves metadata for the specified path.
//
// Stat is a wrapper around the C-binding function `opendal_operator_stat`.
// When options are provided, it uses `opendal_operator_stat_with`.
//
// # Parameters
//
//   - path: The path of the file or directory to get metadata for.
//   - opts: Optional stat options.
//
// # Returns
//
//   - *Metadata: Metadata of the specified path.
//   - error: An error if the operation fails, or nil if successful.
//
// # Notes
//
//   - If the path does not exist, an error with code opendal.CodeNotFound will be returned.
//
// # Example
//
//	func exampleStat(op *opendal.Operator) {
//		meta, err := op.Stat("/path/to/file")
//		if err != nil {
//			if e, ok := err.(*opendal.Error); ok && e.Code() == opendal.CodeNotFound {
//				fmt.Println("File not found")
//				return
//			}
//			log.Fatalf("Stat operation failed: %v", err)
//		}
//		fmt.Printf("File size: %d bytes\n", meta.ContentLength())
//		fmt.Printf("Last modified: %v\n", meta.LastModified())
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Stat(path string, opts ...WithStatFn) (*Metadata, error) {
	if len(opts) == 0 {
		meta, err := ffiOperatorStat.symbol(op.ctx)(op.inner, path)
		if err != nil {
			return nil, err
		}
		return newMetadata(op.ctx, meta), nil
	}

	o := parseStatOptions(opts...)
	cOpts, keepAlive, err := newOpendalStatOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiStatOptionsFree.symbol(op.ctx)(cOpts)
	meta, err := ffiOperatorStatWith.symbol(op.ctx)(op.inner, path, cOpts)
	runtime.KeepAlive(keepAlive)
	if err != nil {
		return nil, err
	}
	return newMetadata(op.ctx, meta), nil
}

// WithStatFn is a functional option for stat operations.
type WithStatFn func(*statOptions)

// StatWithVersion sets the version of the object to stat.
func StatWithVersion(version string) WithStatFn {
	return func(o *statOptions) {
		o.version = version
	}
}

// StatWithIfMatch sets the If-Match condition for the stat operation.
func StatWithIfMatch(ifMatch string) WithStatFn {
	return func(o *statOptions) {
		o.ifMatch = ifMatch
	}
}

// StatWithIfNoneMatch sets the If-None-Match condition for the stat operation.
func StatWithIfNoneMatch(ifNoneMatch string) WithStatFn {
	return func(o *statOptions) {
		o.ifNoneMatch = ifNoneMatch
	}
}

// StatWithIfModifiedSince sets the If-Modified-Since condition for the stat operation.
func StatWithIfModifiedSince(t time.Time) WithStatFn {
	return func(o *statOptions) {
		o.ifModifiedSince = &t
	}
}

// StatWithIfUnmodifiedSince sets the If-Unmodified-Since condition for the stat operation.
func StatWithIfUnmodifiedSince(t time.Time) WithStatFn {
	return func(o *statOptions) {
		o.ifUnmodifiedSince = &t
	}
}

// StatWithOverrideContentType sets the response Content-Type header override.
//
// This option is only meaningful when used along with presign.
func StatWithOverrideContentType(contentType string) WithStatFn {
	return func(o *statOptions) {
		o.overrideContentType = contentType
	}
}

// StatWithOverrideCacheControl sets the response Cache-Control header override.
//
// This option is only meaningful when used along with presign.
func StatWithOverrideCacheControl(cacheControl string) WithStatFn {
	return func(o *statOptions) {
		o.overrideCacheControl = cacheControl
	}
}

// StatWithOverrideContentDisposition sets the response Content-Disposition header override.
//
// This option is only meaningful when used along with presign.
func StatWithOverrideContentDisposition(contentDisposition string) WithStatFn {
	return func(o *statOptions) {
		o.overrideContentDisposition = contentDisposition
	}
}

type statOptions struct {
	version                    string
	ifMatch                    string
	ifNoneMatch                string
	ifModifiedSince            *time.Time
	ifUnmodifiedSince          *time.Time
	overrideContentType        string
	overrideCacheControl       string
	overrideContentDisposition string
}

func parseStatOptions(opts ...WithStatFn) *statOptions {
	o := &statOptions{}
	for _, opt := range opts {
		opt(o)
	}
	return o
}

func newOpendalStatOptions(ctx context.Context, o *statOptions) (*opendalStatOptions, [][]byte, error) {
	cOpts := ffiStatOptionsNew.symbol(ctx)()
	var keepAlive [][]byte

	fail := func(err error) (*opendalStatOptions, [][]byte, error) {
		ffiStatOptionsFree.symbol(ctx)(cOpts)
		return nil, nil, err
	}

	setString := func(value string, set func(*opendalStatOptions, string) ([]byte, error)) error {
		if value == "" {
			return nil
		}
		data, err := set(cOpts, value)
		if err != nil {
			return err
		}
		keepAlive = append(keepAlive, data)
		return nil
	}

	if err := setString(o.version, ffiStatOptionsSetVersion.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifMatch, ffiStatOptionsSetIfMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.ifNoneMatch, ffiStatOptionsSetIfNoneMatch.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.overrideContentType, ffiStatOptionsSetOverrideContentType.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.overrideCacheControl, ffiStatOptionsSetOverrideCacheControl.symbol(ctx)); err != nil {
		return fail(err)
	}
	if err := setString(o.overrideContentDisposition, ffiStatOptionsSetOverrideContentDisposition.symbol(ctx)); err != nil {
		return fail(err)
	}
	if o.ifModifiedSince != nil {
		ffiStatOptionsSetIfModifiedSince.symbol(ctx)(cOpts, o.ifModifiedSince.UnixMilli())
	}
	if o.ifUnmodifiedSince != nil {
		ffiStatOptionsSetIfUnmodifiedSince.symbol(ctx)(cOpts, o.ifUnmodifiedSince.UnixMilli())
	}
	return cOpts, keepAlive, nil
}

// IsExist checks if a file or directory exists at the specified path.
//
// This method provides a convenient way to determine the existence of a resource
// without fetching its full metadata.
//
// # Parameters
//
//   - path: The path of the file or directory to check.
//
// # Returns
//
//   - bool: true if the resource exists, false otherwise.
//   - error: An error if the check operation fails, or nil if the check is successful.
//     Note that a false return value with a nil error indicates that the resource does not exist.
//
// # Example
//
//	exists, err := op.IsExist("path/to/file")
//	if err != nil {
//		log.Fatalf("Error checking existence: %v", err)
//	}
//	if exists {
//		fmt.Println("The file exists")
//	} else {
//		fmt.Println("The file does not exist")
//	}
func (op *Operator) IsExist(path string) (bool, error) {
	return ffiOperatorIsExist.symbol(op.ctx)(op.inner, path)
}

var ffiOperatorStat = newFFI(ffiOpts{
	sym:    "opendal_operator_stat",
	rType:  &typeResultStat,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string) (*opendalMetadata, error) {
	return func(op *opendalOperator, path string) (*opendalMetadata, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultStat
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.meta, nil
	}
})

var ffiOperatorStatWith = newFFI(ffiOpts{
	sym:    "opendal_operator_stat_with",
	rType:  &typeResultStat,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, opts *opendalStatOptions) (*opendalMetadata, error) {
	return func(op *opendalOperator, path string, opts *opendalStatOptions) (*opendalMetadata, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultStat
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.meta, nil
	}
})

var ffiOperatorIsExist = newFFI(ffiOpts{
	sym:    "opendal_operator_is_exist",
	rType:  &typeResultIsExist,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string) (bool, error) {
	return func(op *opendalOperator, path string) (bool, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return false, err
		}
		var result resultIsExist
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
		)
		if result.error != nil {
			return false, parseError(ctx, result.error)
		}
		return result.is_exist == 1, nil
	}
})

var ffiStatOptionsNew = newFFI(ffiOpts{
	sym:   "opendal_stat_options_new",
	rType: &ffi.TypePointer,
}, func(_ context.Context, ffiCall ffiCall) func() *opendalStatOptions {
	return func() *opendalStatOptions {
		var opts *opendalStatOptions
		ffiCall(unsafe.Pointer(&opts))
		return opts
	}
})

var ffiStatOptionsFree = newFFI(ffiOpts{
	sym:    "opendal_stat_options_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalStatOptions) {
	return func(opts *opendalStatOptions) {
		ffiCall(nil, unsafe.Pointer(&opts))
	}
})

func newStatOptionsSetStringFFI(sym string) *FFI[func(*opendalStatOptions, string) ([]byte, error)] {
	return newFFI(ffiOpts{
		sym:    contextKey(sym),
		rType:  &ffi.TypeVoid,
		aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
	}, func(_ context.Context, ffiCall ffiCall) func(*opendalStatOptions, string) ([]byte, error) {
		return func(opts *opendalStatOptions, value string) ([]byte, error) {
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

var ffiStatOptionsSetVersion = newStatOptionsSetStringFFI("opendal_stat_options_set_version")
var ffiStatOptionsSetIfMatch = newStatOptionsSetStringFFI("opendal_stat_options_set_if_match")
var ffiStatOptionsSetIfNoneMatch = newStatOptionsSetStringFFI("opendal_stat_options_set_if_none_match")
var ffiStatOptionsSetOverrideContentType = newStatOptionsSetStringFFI("opendal_stat_options_set_override_content_type")
var ffiStatOptionsSetOverrideCacheControl = newStatOptionsSetStringFFI("opendal_stat_options_set_override_cache_control")
var ffiStatOptionsSetOverrideContentDisposition = newStatOptionsSetStringFFI("opendal_stat_options_set_override_content_disposition")

var ffiStatOptionsSetIfModifiedSince = newFFI(ffiOpts{
	sym:    "opendal_stat_options_set_if_modified_since",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeSint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalStatOptions, ms int64) {
	return func(opts *opendalStatOptions, ms int64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&ms))
	}
})

var ffiStatOptionsSetIfUnmodifiedSince = newFFI(ffiOpts{
	sym:    "opendal_stat_options_set_if_unmodified_since",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypeSint64},
}, func(_ context.Context, ffiCall ffiCall) func(opts *opendalStatOptions, ms int64) {
	return func(opts *opendalStatOptions, ms int64) {
		ffiCall(nil, unsafe.Pointer(&opts), unsafe.Pointer(&ms))
	}
})
