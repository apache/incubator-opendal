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
	"sync"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

type ffiOpts struct {
	sym    contextKey
	rType  *ffi.Type
	aTypes []*ffi.Type
}

type ffiCall func(rValue unsafe.Pointer, aValues ...unsafe.Pointer)

type contextKey string

func (c contextKey) String() string {
	return string(c)
}

type contextWithFFI func(ctx context.Context, lib uintptr) (context.Context, error)

type FFI[T any] struct {
	opts     ffiOpts
	withFunc func(ctx context.Context, ffiCall ffiCall) T
}

func newFFI[T any](opts ffiOpts, withFunc func(ctx context.Context, ffiCall ffiCall) T) *FFI[T] {
	ffi := &FFI[T]{
		opts:     opts,
		withFunc: withFunc,
	}
	withFFIs = append(withFFIs, ffi.withFFI)
	return ffi
}

func (f *FFI[T]) symbol(ctx context.Context) T {
	return ctx.Value(f.opts.sym).(T)
}

func (f *FFI[T]) withFFI(ctx context.Context, lib uintptr) (context.Context, error) {
	var cif ffi.Cif
	if status := ffi.PrepCif(
		&cif,
		ffi.DefaultAbi,
		uint32(len(f.opts.aTypes)),
		f.opts.rType,
		f.opts.aTypes...,
	); status != ffi.OK {
		return nil, errors.New(status.String())
	}
	fn, err := GetProcAddress(lib, f.opts.sym.String())
	if err != nil {
		return nil, err
	}
	val := f.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		ffi.Call(&cif, fn, rValue, aValues...)
	})
	return context.WithValue(ctx, f.opts.sym, val), nil
}

var withFFIs []contextWithFFI

var (
	libraryMu sync.Mutex
	libraries = map[string]uintptr{}
)

func loadSharedLibrary(path string) (uintptr, error) {
	libraryMu.Lock()
	defer libraryMu.Unlock()

	if lib, ok := libraries[path]; ok {
		return lib, nil
	}

	lib, err := LoadLibrary(path)
	if err != nil {
		return 0, err
	}
	libraries[path] = lib
	return lib, nil
}

func newContext(path string) (ctx context.Context, cancel context.CancelFunc, err error) {
	lib, err := loadSharedLibrary(path)
	if err != nil {
		return
	}
	ctx = context.Background()
	for _, withFFI := range withFFIs {
		ctx, err = withFFI(ctx, lib)
		if err != nil {
			return
		}
	}
	// Keep the library loaded for the process lifetime. The context stores libffi
	// call targets into this library, so unloading it during operator cleanup can
	// leave Go frames returning into unmapped code.
	cancel = func() {}

	return
}
