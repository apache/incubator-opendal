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
	"strings"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// Rename changes the name or location of a file from the source path to the destination path.
//
// This function moves a file from 'from' to 'to', effectively renaming or relocating it.
//
// # Parameters
//
//   - from: The current file path.
//   - to: The new file path.
//
// # Returns
//
//   - error: An error if the rename operation fails, or nil if successful.
//
// # Behavior
//
//   - Both 'from' and 'to' must be file paths, not directories.
//   - If 'to' already exists, it will be overwritten.
//   - If 'from' and 'to' are identical, an 'IsSameFile' error will be returned.
//
// # Example
//
//	func exampleRename(op *opendal.Operator) {
//		err = op.Rename("path/from/file", "path/to/file")
//		if err != nil {
//			log.Printf("Rename operation failed: %v", err)
//		} else {
//			log.Println("File renamed successfully")
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Rename(src, dest string) error {
	return ffiOperatorRename.symbol(op.ctx)(op.inner, src, dest)
}

// Check verifies if the operator is functioning correctly.
//
// Check is a wrapper around the C-binding function `opendal_operator_check`.
// It performs a health check against the underlying backend, returning any
// error encountered while reaching it.
//
// # Returns
//
//   - error: An error if the check fails, or nil if the operator is working correctly.
//
// # Example
//
//	func exampleCheck(op *opendal.Operator) {
//		err = op.Check()
//		if err != nil {
//			log.Printf("Operator check failed: %v", err)
//		} else {
//			log.Println("Operator is functioning correctly")
//		}
//	}
//
// Note: This example assumes proper error handling and import statements.
func (op *Operator) Check() error {
	return ffiOperatorCheck.symbol(op.ctx)(op.inner)
}

func normalizeScheme(scheme Scheme) (*byte, error) {
	return BytePtrFromString(strings.ReplaceAll(scheme.Name(), "_", "-"))
}

var ffiOperatorNewWithLayers = newFFI(ffiOpts{
	sym:    "opendal_operator_new_with_layers",
	rType:  &typeResultOperatorNew,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(scheme Scheme, opts *operatorOptions, layers *operatorLayers) (op *opendalOperator, err error) {
	return func(scheme Scheme, opts *operatorOptions, layers *operatorLayers) (op *opendalOperator, err error) {
		var byteName *byte
		byteName, err = normalizeScheme(scheme)
		if err != nil {
			return nil, err
		}
		var result resultOperatorNew
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&byteName),
			unsafe.Pointer(&opts),
			unsafe.Pointer(&layers),
		)
		if result.error != nil {
			err = parseError(ctx, result.error)
			return
		}
		op = result.op
		return
	}
})

var ffiOperatorFree = newFFI(ffiOpts{
	sym:    "opendal_operator_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(op *opendalOperator) {
	return func(op *opendalOperator) {
		ffiCall(
			nil,
			unsafe.Pointer(&op),
		)
	}
})

type operatorOptions struct{}

var ffiOperatorOptionsNew = newFFI(ffiOpts{
	sym:   "opendal_operator_options_new",
	rType: &ffi.TypePointer,
}, func(_ context.Context, ffiCall ffiCall) func() (opts *operatorOptions) {
	return func() (opts *operatorOptions) {
		ffiCall(unsafe.Pointer(&opts))
		return
	}
})

var ffiOperatorOptionsSet = newFFI(ffiOpts{
	sym:    "opendal_operator_options_set",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *operatorOptions, key, value string) (err error) {
	return func(opts *operatorOptions, key, value string) (err error) {
		var (
			byteKey   *byte
			byteValue *byte
		)
		byteKey, err = BytePtrFromString(key)
		if err != nil {
			return err
		}
		byteValue, err = BytePtrFromString(value)
		if err != nil {
			return err
		}
		ffiCall(
			nil,
			unsafe.Pointer(&opts),
			unsafe.Pointer(&byteKey),
			unsafe.Pointer(&byteValue),
		)
		return nil
	}
})

var ffiOperatorOptionsFree = newFFI(ffiOpts{
	sym:    "opendal_operator_options_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(opts *operatorOptions) {
	return func(opts *operatorOptions) {
		ffiCall(
			nil,
			unsafe.Pointer(&opts),
		)
	}
})

var ffiOperatorCopy = newFFI(ffiOpts{
	sym:    "opendal_operator_copy",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, src, dest string) (err error) {
	return func(op *opendalOperator, src, dest string) (err error) {
		var (
			byteSrc  *byte
			byteDest *byte
		)
		byteSrc, err = BytePtrFromString(src)
		if err != nil {
			return err
		}
		byteDest, err = BytePtrFromString(dest)
		if err != nil {
			return err
		}
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&op),
			unsafe.Pointer(&byteSrc),
			unsafe.Pointer(&byteDest),
		)
		return parseError(ctx, e)
	}
})

var ffiOperatorRename = newFFI(ffiOpts{
	sym:    "opendal_operator_rename",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, src, dest string) (err error) {
	return func(op *opendalOperator, src, dest string) (err error) {
		var (
			byteSrc  *byte
			byteDest *byte
		)
		byteSrc, err = BytePtrFromString(src)
		if err != nil {
			return err
		}
		byteDest, err = BytePtrFromString(dest)
		if err != nil {
			return err
		}
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&op),
			unsafe.Pointer(&byteSrc),
			unsafe.Pointer(&byteDest),
		)
		return parseError(ctx, e)
	}
})

var ffiOperatorCheck = newFFI(ffiOpts{
	sym:    "opendal_operator_check",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator) error {
	return func(op *opendalOperator) error {
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&op),
		)
		return parseError(ctx, e)
	}
})

var ffiBytesFree = newFFI(ffiOpts{
	sym:    "opendal_bytes_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(b *opendalBytes) {
	return func(b *opendalBytes) {
		ffiCall(
			nil,
			unsafe.Pointer(&b),
		)
	}
})
