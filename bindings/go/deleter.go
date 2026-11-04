/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

package opendal

import (
	"context"
	"errors"
	"runtime"
	"sync"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// ErrDeleterClosed reports that a Deleter is used after it is released by Close
// or Discard.
var ErrDeleterClosed = errors.New("opendal: deleter is closed")

// Deleter removes many paths from storage.
//
// Deleter queues paths and hands them to the service, using batch deletion when
// the service supports it. Delete only queues a path: the deletion may still be
// pending when Delete returns, and it is Flush or Close that hands the queue to
// the service and waits for every queued path to be removed. Treat a path as
// deleted only after Flush or Close returns nil.
//
// Prefer Operator.Delete for a single path; a Deleter pays off when removing
// many paths from a service with batch delete support, such as S3, and it is the
// only way to delete a specific version of each path in one pass.
//
// Operator.WithDeleter drives this lifecycle for the caller and is the preferred
// entry point. Callers that build a Deleter directly must release it with Close
// or Discard; the binding installs no finalizer, so an unreleased Deleter leaks
// its native handle.
//
// A Deleter is safe for concurrent use: its methods serialize against each
// other, so a delete never overlaps a flush or a release.
//
// # Example
//
//	func exampleDeleter(op *opendal.Operator) {
//		err := op.WithDeleter(func(d *opendal.Deleter) error {
//			for _, path := range []string{"a.txt", "b.txt", "c.txt"} {
//				if err := d.Delete(path); err != nil {
//					return err
//				}
//			}
//			return nil
//		})
//		if err != nil {
//			log.Fatal(err)
//		}
//	}
type Deleter struct {
	// mu serializes access to inner. The C deleter hands out a &mut to the
	// underlying Rust deleter, so two goroutines calling in at once would alias
	// it, which is undefined behavior rather than merely a data race.
	mu    sync.Mutex
	inner *opendalDeleter
	ctx   context.Context
}

// Deleter creates a Deleter that removes many paths.
//
// Deleter is a wrapper around the C-binding function `opendal_operator_deleter`.
//
// # Returns
//
//   - *Deleter: A Deleter used to queue deletions, or an error if the operation fails.
//
// # Behavior
//
//   - The returned Deleter must be released, either with Close, which flushes the
//     queued deletions first, or with Discard, which drops them.
//   - Prefer Operator.WithDeleter, which pairs the release with the queueing for
//     the caller.
func (op *Operator) Deleter() (*Deleter, error) {
	inner, err := ffiOperatorDeleter.symbol(op.ctx)(op.inner)
	if err != nil {
		return nil, err
	}
	return &Deleter{inner: inner, ctx: op.ctx}, nil
}

// Delete queues path for deletion.
//
// The path is not necessarily removed when Delete returns: Flush or Close hands
// the queue to the service and waits for the deletions to complete. Delete
// returns ErrDeleterClosed once the Deleter is released.
//
// Services without batch delete support remove each path as Delete queues it, so
// how much of the work Delete has already done when it returns depends on the
// service.
//
// # Parameters
//
//   - path: The path of the file or directory to delete.
//   - opts: Optional functional options, shared with Operator.Delete, to delete a
//     specific version or to delete recursively.
func (d *Deleter) Delete(path string, opts ...WithDeleteFn) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.inner == nil {
		return ErrDeleterClosed
	}
	if len(opts) == 0 {
		return ffiDeleterDelete.symbol(d.ctx)(d.inner, path)
	}

	o := parseDeleteOptions(opts...)
	cOpts, keepAlive, err := newOpendalDeleteOptions(d.ctx, o)
	if err != nil {
		return err
	}
	defer ffiDeleteOptionsFree.symbol(d.ctx)(cOpts)
	err = ffiDeleterDeleteWith.symbol(d.ctx)(d.inner, path, cOpts)
	// The C option setters copy their arguments into Rust-owned memory, so cOpts
	// borrows no Go memory; keepAlive pins the buffers through the setter calls.
	runtime.KeepAlive(keepAlive)
	return err
}

// deleteMany queues every path in one crossing of the FFI boundary.
//
// Queueing paths one at a time costs a boundary crossing and a blocking call per
// path, which dominates on services whose queueing is just a buffer insert. This
// hands the whole slice over at once. Every path is queued with default options,
// so callers that need per-path options must use Delete.
func (d *Deleter) deleteMany(paths []string) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.inner == nil {
		return ErrDeleterClosed
	}
	// An empty slice is handled by the FFI wrapper and the C side; Remove, the
	// only caller, already skips creating a deleter for it.
	return ffiDeleterDeleteMany.symbol(d.ctx)(d.inner, paths)
}

// Flush hands the queued deletions to the service, waits for them to complete,
// and keeps the Deleter usable.
//
// A failed flush keeps the paths it could not delete queued, so the caller can
// retry them by calling Flush again. Flush returns ErrDeleterClosed once the
// Deleter is released.
func (d *Deleter) Flush() error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.inner == nil {
		return ErrDeleterClosed
	}
	return ffiDeleterFlush.symbol(d.ctx)(d.inner)
}

// Close flushes the queued deletions, waits for them to complete, and releases
// the resources held by the Deleter.
//
// Close releases the Deleter whether or not the flush succeeds, so the error it
// returns is the only report that the queued deletions failed: check it rather
// than deferring Close. Close drops whatever it could not delete, so retry with
// Flush, which keeps those paths queued, and call Close once it succeeds. Close
// is safe to call more than once; further calls return nil.
func (d *Deleter) Close() error {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.inner == nil {
		return nil
	}
	defer d.releaseLocked()
	return ffiDeleterFlush.symbol(d.ctx)(d.inner)
}

// Discard releases the Deleter without flushing, dropping every path that is
// still queued.
//
// Use Discard to abandon queued deletions, and to release a Deleter on a path
// where flushing is not wanted. Discarding does not undo the deletions the
// service already performed. Discard is safe to call more than once, and after
// Close; further calls are no-ops.
func (d *Deleter) Discard() {
	d.mu.Lock()
	defer d.mu.Unlock()
	if d.inner == nil {
		return
	}
	d.releaseLocked()
}

// releaseLocked frees the native deleter and blocks further use. The caller must
// hold d.mu and must have checked that d.inner is not nil.
func (d *Deleter) releaseLocked() {
	ffiDeleterFree.symbol(d.ctx)(d.inner)
	d.inner = nil
}

// WithDeleter runs fn with a Deleter and releases it before returning.
//
// WithDeleter flushes the queue once fn returns nil, so fn only has to queue the
// paths it wants removed. When fn returns an error, WithDeleter releases the
// Deleter without flushing and returns that error, so it queues nothing further.
// The Deleter is released even if fn panics. fn must not retain it past its
// return, because it is released by then.
//
// Skipping the flush does not undo the deletions the service already performed.
// A service without batch delete support removes each path as Delete queues it,
// and a batch service flushes on its own once a batch fills, so treat a failed
// WithDeleter as "some of these paths may be gone" rather than as a rollback.
//
// # Parameters
//
//   - fn: The function that queues paths on the Deleter.
//
// # Returns
//
//   - error: The error from fn, from creating the Deleter, or from the flush.
//
// # Example
//
//	func exampleWithDeleter(op *opendal.Operator) {
//		err := op.WithDeleter(func(d *opendal.Deleter) error {
//			return d.Delete("a.txt", opendal.DeleteWithVersion("v1"))
//		})
//		if err != nil {
//			log.Printf("delete failed: %v", err)
//		}
//	}
func (op *Operator) WithDeleter(fn func(d *Deleter) error) error {
	deleter, err := op.Deleter()
	if err != nil {
		return err
	}
	// Discard is a no-op once Close has released the deleter, and unlike the
	// straight-line calls it also runs when fn panics.
	defer deleter.Discard()
	if err := fn(deleter); err != nil {
		return err
	}
	return deleter.Close()
}

// Remove deletes the given paths.
//
// Remove drives a Deleter for the caller: it queues every path, then flushes and
// waits for the deletions. Services that support batch deletion remove the paths
// in batches.
//
// Remove queues the whole slice in one crossing of the FFI boundary rather than
// one per path.
//
// Remove is not atomic. Queueing stops at the first path the service rejects, but
// the service may already have deleted an earlier batch, so a failed Remove can
// still leave some paths gone. Every path is deleted with default options; use
// WithDeleter to delete a specific version of each path.
//
// # Parameters
//
//   - paths: The paths to delete.
//
// # Returns
//
//   - error: An error if any deletion fails, or nil if all paths are removed.
//
// # Example
//
//	func exampleRemove(op *opendal.Operator) {
//		err := op.Remove([]string{"a.txt", "b.txt"})
//		if err != nil {
//			log.Printf("Remove operation failed: %v", err)
//		}
//	}
func (op *Operator) Remove(paths []string) error {
	if len(paths) == 0 {
		return nil
	}
	return op.WithDeleter(func(d *Deleter) error {
		return d.deleteMany(paths)
	})
}

var ffiOperatorDeleter = newFFI(ffiOpts{
	sym:    "opendal_operator_deleter",
	rType:  &typeResultOperatorDeleter,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator) (*opendalDeleter, error) {
	return func(op *opendalOperator) (*opendalDeleter, error) {
		var result resultOperatorDeleter
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
		)
		if result.error != nil {
			return nil, parseError(ctx, result.error)
		}
		return result.deleter, nil
	}
})

var ffiDeleterDelete = newFFI(ffiOpts{
	sym:    "opendal_deleter_delete",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(d *opendalDeleter, path string) error {
	return func(d *opendalDeleter, path string) error {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return err
		}
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&d),
			unsafe.Pointer(&bytePath),
		)
		return parseError(ctx, e)
	}
})

var ffiDeleterDeleteMany = newFFI(ffiOpts{
	sym:    "opendal_deleter_delete_many",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(d *opendalDeleter, paths []string) error {
	return func(d *opendalDeleter, paths []string) error {
		pathData := make([][]byte, len(paths))
		pathPointers := make([]*byte, len(paths))
		for i, path := range paths {
			data, err := byteSliceFromString(path)
			if err != nil {
				return err
			}
			pathData[i] = data
			pathPointers[i] = &data[0]
		}

		var pathsPtr **byte
		if len(pathPointers) > 0 {
			pathsPtr = &pathPointers[0]
		}
		var e *opendalError
		pathsLen := uint(len(paths))
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&d),
			unsafe.Pointer(&pathsPtr),
			unsafe.Pointer(&pathsLen),
		)
		// The C side reads through pathsPtr into the Go buffers behind pathData.
		// Keep both reachable until the native call above has returned.
		runtime.KeepAlive(pathData)
		runtime.KeepAlive(pathPointers)
		return parseError(ctx, e)
	}
})

var ffiDeleterDeleteWith = newFFI(ffiOpts{
	sym:    "opendal_deleter_delete_with",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer, &ffi.TypePointer, &ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(d *opendalDeleter, path string, opts *opendalDeleteOptions) error {
	return func(d *opendalDeleter, path string, opts *opendalDeleteOptions) error {
		bytePath, err := BytePtrFromString(path)
		if err != nil {
			return err
		}
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&d),
			unsafe.Pointer(&bytePath),
			unsafe.Pointer(&opts),
		)
		return parseError(ctx, e)
	}
})

var ffiDeleterFlush = newFFI(ffiOpts{
	sym:    "opendal_deleter_flush",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(d *opendalDeleter) error {
	return func(d *opendalDeleter) error {
		var e *opendalError
		ffiCall(
			unsafe.Pointer(&e),
			unsafe.Pointer(&d),
		)
		return parseError(ctx, e)
	}
})

var ffiDeleterFree = newFFI(ffiOpts{
	sym:    "opendal_deleter_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(d *opendalDeleter) {
	return func(d *opendalDeleter) {
		ffiCall(
			nil,
			unsafe.Pointer(&d),
		)
	}
})
