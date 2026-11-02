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
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// Copier drives a long-running copy operation from a source to a destination.
//
// Unlike Copy, which performs the whole copy in a single call, a Copier lets the
// caller advance the copy step by step via Next, which is useful for reporting
// progress on large copies. The copy is complete once Next reports that no more
// progress is available. Always call Close to release the resources held by the
// Copier.
//
// # Example
//
//	func exampleCopier(op *opendal.Operator) {
//		copier, err := op.Copier("path/from/file", "path/to/file")
//		if err != nil {
//			log.Fatal(err)
//		}
//		defer copier.Close()
//		for {
//			n, ok, err := copier.Next()
//			if err != nil {
//				log.Fatal(err)
//			}
//			if !ok {
//				break
//			}
//			log.Printf("copied %d bytes", n)
//		}
//	}
type Copier struct {
	inner *opendalCopier
	ctx   context.Context
}

// Copier creates a Copier to copy a file from src to dest.
//
// Copier is a wrapper around the C-binding function `opendal_operator_copier`.
// When options are provided, it uses `opendal_operator_copier_with`.
//
// # Parameters
//
//   - src: The source file path.
//   - dest: The destination file path.
//   - opts: Optional copy options (shared with Copy).
//
// # Returns
//
//   - *Copier: A Copier used to drive the copy, or an error if the operation fails.
//
// # Behavior
//
//   - Both src and dest must be file paths, not directories.
//   - The returned Copier must be released with Close.
func (op *Operator) Copier(src, dest string, opts ...WithCopyFn) (*Copier, error) {
	if len(opts) == 0 {
		inner, err := ffiOperatorCopier.symbol(op.ctx)(op.inner, src, dest)
		if err != nil {
			return nil, err
		}
		return &Copier{inner: inner, ctx: op.ctx}, nil
	}

	o := parseCopyOptions(opts...)
	cOpts, keepAlive, err := newOpendalCopyOptions(op.ctx, o)
	if err != nil {
		return nil, err
	}
	defer ffiCopyOptionsFree.symbol(op.ctx)(cOpts)
	inner, err := ffiOperatorCopierWith.symbol(op.ctx)(op.inner, src, dest, cOpts)
	// cOpts holds raw pointers into the Go buffers tracked by keepAlive.
	// Keep them reachable until the native call above has returned.
	runtime.KeepAlive(keepAlive)
	if err != nil {
		return nil, err
	}
	return &Copier{inner: inner, ctx: op.ctx}, nil
}

// Next drives the copy forward by one step.
//
// It returns the number of bytes copied in this step. ok is false when the copy
// has completed; once that happens, further calls keep returning (0, false, nil).
// A non-nil error reports a failed copy. Calling Next on a closed Copier also
// returns (0, false, nil).
func (c *Copier) Next() (size uint, ok bool, err error) {
	if c.inner == nil {
		return 0, false, nil
	}
	return ffiCopierNext.symbol(c.ctx)(c.inner)
}

// Abort aborts the pending copy operation.
//
// After Abort, the Copier should still be released with Close. Calling Abort
// on a closed Copier is a no-op.
func (c *Copier) Abort() error {
	if c.inner == nil {
		return nil
	}
	return ffiCopierAbort.symbol(c.ctx)(c.inner)
}

// Close releases the resources held by the Copier. It must be called once the
// Copier is no longer needed. Close is safe to call more than once; subsequent
// calls are no-ops.
func (c *Copier) Close() error {
	if c.inner == nil {
		return nil
	}
	ffiCopierFree.symbol(c.ctx)(c.inner)
	c.inner = nil
	return nil
}

var ffiOperatorCopier = newFFI(ffiOpts{
	sym:    "opendal_operator_copier",
	rType:  &typeResultOperatorCopier,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, src, dest string) (*opendalCopier, error) {
	return func(op *opendalOperator, src, dest string) (*opendalCopier, error) {
		byteSrc, err := BytePtrFromString(src)
		if err != nil {
			return nil, err
		}
		byteDest, err := BytePtrFromString(dest)
		if err != nil {
			return nil, err
		}
		var result resultOperatorCopier
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&byteSrc),
			unsafe.Pointer(&byteDest),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.copier, nil
	}
})

var ffiOperatorCopierWith = newFFI(ffiOpts{
	sym:    "opendal_operator_copier_with",
	rType:  &typeResultOperatorCopier,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator, src, dest string, opts *opendalCopyOptions) (*opendalCopier, error) {
	return func(op *opendalOperator, src, dest string, opts *opendalCopyOptions) (*opendalCopier, error) {
		byteSrc, err := BytePtrFromString(src)
		if err != nil {
			return nil, err
		}
		byteDest, err := BytePtrFromString(dest)
		if err != nil {
			return nil, err
		}
		var result resultOperatorCopier
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
			unsafe.Pointer(&byteSrc),
			unsafe.Pointer(&byteDest),
			unsafe.Pointer(&opts),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.copier, nil
	}
})

var ffiCopierNext = newFFI(ffiOpts{
	sym:    "opendal_copier_next",
	rType:  &typeResultCopierNext,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(c *opendalCopier) (uint, bool, error) {
	return func(c *opendalCopier) (uint, bool, error) {
		var result resultCopierNext
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&c),
		)
		if result.error != nil {
			return 0, false, parseError(ctx, result.error)
		}
		return result.size, result.has_next == 1, nil
	}
})

var ffiCopierAbort = newFFI(ffiOpts{
	sym:    "opendal_copier_abort",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(c *opendalCopier) error {
	return func(c *opendalCopier) error {
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&c),
		)
		return parseError(ctx, e)
	}
})

var ffiCopierFree = newFFI(ffiOpts{
	sym:    "opendal_copier_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(c *opendalCopier) {
	return func(c *opendalCopier) {
		ffiCall(
			nil,
			unsafe.Pointer(&c),
		)
	}
})
