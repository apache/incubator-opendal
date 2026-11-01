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
	"fmt"
	"net/http"
	"runtime"
	"time"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

type presignFunc func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error)

// PresignRead returns a presigned HTTP request that can be used to read the object at the given path.
func (op *Operator) PresignRead(path string, expire time.Duration, opts ...WithReadFn) (*http.Request, error) {
	if len(opts) == 0 {
		return op.presign(path, expire, ffiOperatorPresignRead.symbol(op.ctx))
	}

	o := parseReadOptions(opts...)
	cOpts, keepAlive, err := newOpendalReadOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiReadOptionsFree.symbol(op.ctx)(cOpts)
	req, err := ffiOperatorPresignReadWith.symbol(op.ctx)(op.inner, path, uint64(expire/time.Second), cOpts)
	runtime.KeepAlive(keepAlive)
	return op.buildPresignedRequest(req, err)
}

// PresignWrite returns a presigned HTTP request that can be used to write the object at the given path.
func (op *Operator) PresignWrite(path string, expire time.Duration, opts ...WithWriteFn) (*http.Request, error) {
	if len(opts) == 0 {
		return op.presign(path, expire, ffiOperatorPresignWrite.symbol(op.ctx))
	}

	o := parseWriteOptions(opts...)
	cOpts, keepAlive, err := newOpendalWriteOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiWriteOptionsFree.symbol(op.ctx)(cOpts)
	req, err := ffiOperatorPresignWriteWith.symbol(op.ctx)(op.inner, path, uint64(expire/time.Second), cOpts)
	runtime.KeepAlive(keepAlive)
	return op.buildPresignedRequest(req, err)
}

// PresignDelete returns a presigned HTTP request that can be used to delete the object at the given path.
func (op *Operator) PresignDelete(path string, expire time.Duration, opts ...WithDeleteFn) (*http.Request, error) {
	if len(opts) == 0 {
		return op.presign(path, expire, ffiOperatorPresignDelete.symbol(op.ctx))
	}

	o := parseDeleteOptions(opts...)
	cOpts, keepAlive, err := newOpendalDeleteOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiDeleteOptionsFree.symbol(op.ctx)(cOpts)
	req, err := ffiOperatorPresignDeleteWith.symbol(op.ctx)(op.inner, path, uint64(expire/time.Second), cOpts)
	runtime.KeepAlive(keepAlive)
	return op.buildPresignedRequest(req, err)
}

// PresignStat returns a presigned HTTP request that can be used to stat the object at the given path.
func (op *Operator) PresignStat(path string, expire time.Duration, opts ...WithStatFn) (*http.Request, error) {
	if len(opts) == 0 {
		return op.presign(path, expire, ffiOperatorPresignStat.symbol(op.ctx))
	}

	o := parseStatOptions(opts...)
	cOpts, keepAlive, err := newOpendalStatOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiStatOptionsFree.symbol(op.ctx)(cOpts)
	req, err := ffiOperatorPresignStatWith.symbol(op.ctx)(op.inner, path, uint64(expire/time.Second), cOpts)
	runtime.KeepAlive(keepAlive)
	return op.buildPresignedRequest(req, err)
}

func (op *Operator) presign(path string, expire time.Duration, call presignFunc) (*http.Request, error) {
	secs := uint64(expire / time.Second)

	req, err := call(op.inner, path, secs)
	return op.buildPresignedRequest(req, err)
}

func (op *Operator) buildPresignedRequest(req *opendalPresignedRequest, err error) (*http.Request, error) {
	if err != nil {
		return nil, err
	}
	if req == nil {
		return nil, fmt.Errorf("presigned request should not be nil")
	}
	defer ffiPresignedRequestFree.symbol(op.ctx)(req)

	return buildHTTPPresignedRequest(op.ctx, req)
}

func buildHTTPPresignedRequest(ctx context.Context, ptr *opendalPresignedRequest) (req *http.Request, err error) {
	mptr := ffiPresignedRequestMethod.symbol(ctx)(ptr)
	uptr := ffiPresignedRequestUri.symbol(ctx)(ptr)

	method := BytePtrToString(mptr)
	uri := BytePtrToString(uptr)

	req, err = http.NewRequest(method, uri, nil)
	if err != nil {
		return
	}

	hptr := ffiPresignedRequestHeaders.symbol(ctx)(ptr)
	hl := ffiPresignedRequestHeadersLen.symbol(ctx)(ptr)
	if hptr == nil || hl == 0 {
		return
	}

	pairs := unsafe.Slice(hptr, int(hl))
	for i := range pairs {
		key := BytePtrToString(pairs[i].key)
		value := BytePtrToString(pairs[i].value)
		if key == "" {
			continue
		}
		req.Header.Add(key, value)
	}

	return
}

var ffiOperatorPresignRead = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_read",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignReadWith = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_read_with",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64, opts *opendalReadOptions) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64, opts *opendalReadOptions) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignWrite = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_write",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignWriteWith = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_write_with",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64, opts *opendalWriteOptions) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64, opts *opendalWriteOptions) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignDelete = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_delete",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignDeleteWith = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_delete_with",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64, opts *opendalDeleteOptions) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64, opts *opendalDeleteOptions) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignStat = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_stat",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiOperatorPresignStatWith = newFFI(ffiOpts{
	sym:    "opendal_operator_presign_stat_with",
	rType:  &typeResultPresign,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypeUint64, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, path string, expire uint64, opts *opendalStatOptions) (*opendalPresignedRequest, error) {
	return func(op *opendalOperator, path string, expire uint64, opts *opendalStatOptions) (*opendalPresignedRequest, error) {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return nil, err
		}
		var result resultPresign
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&expire),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.req, nil
	}
})

var ffiPresignedRequestMethod = newFFI(ffiOpts{
	sym:    "opendal_presigned_request_method",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(req *opendalPresignedRequest) *byte {
	return func(req *opendalPresignedRequest) *byte {
		var result *byte
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&req),
		)
		return result
	}
})

var ffiPresignedRequestUri = newFFI(ffiOpts{
	sym:    "opendal_presigned_request_uri",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(req *opendalPresignedRequest) *byte {
	return func(req *opendalPresignedRequest) *byte {
		var result *byte
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&req),
		)
		return result
	}
})

var ffiPresignedRequestHeaders = newFFI(ffiOpts{
	sym:    "opendal_presigned_request_headers",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(req *opendalPresignedRequest) *opendalHttpHeaderPair {
	return func(req *opendalPresignedRequest) *opendalHttpHeaderPair {
		var result *opendalHttpHeaderPair
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&req),
		)
		return result
	}
})

var ffiPresignedRequestHeadersLen = newFFI(ffiOpts{
	sym:    "opendal_presigned_request_headers_len",
	rType:  &ffi.TypeUint64,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(req *opendalPresignedRequest) uintptr {
	return func(req *opendalPresignedRequest) uintptr {
		var length uint64
		ffiCall(
			unsafe.Pointer(&length),
			unsafe.Pointer(&req),
		)
		return uintptr(length)
	}
})

var ffiPresignedRequestFree = newFFI(ffiOpts{
	sym:    "opendal_presigned_request_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(req *opendalPresignedRequest) {
	return func(req *opendalPresignedRequest) {
		ffiCall(
			nil,
			unsafe.Pointer(&req),
		)
	}
})
