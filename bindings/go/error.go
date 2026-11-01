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
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// ErrorCode is all kinds of ErrorCode of opendal
type ErrorCode int32

const (
	// OpenDAL don't know what happened here, and no actions other than just
	// returning it back. For example, s3 returns an internal service error.
	CodeUnexpected ErrorCode = iota
	// Underlying service doesn't support this operation.
	CodeUnsupported
	// The config for backend is invalid.
	CodeConfigInvalid
	// The given path is not found.
	CodeNotFound
	// The given path doesn't have enough permission for this operation
	CodePermissioDenied
	// The given path is a directory.
	CodeIsADirectory
	// The given path is not a directory.
	CodeNotADirectory
	// The given path already exists thus we failed to the specified operation on it.
	CodeAlreadyExists
	// Requests that sent to this path is over the limit, please slow down.
	CodeRateLimited
	// The given file paths are same.
	CodeIsSameFile
	// A condition supplied through the OpenDAL operation evaluated to false.
	CodeConditionNotMatch
	// The range of the content is not satisfied.
	//
	// OpenDAL returns this error to indicate that the range of the read request is not satisfied.
	CodeRangeNotSatisfied
	// The operation conflicts with the current or transitional state of the resource.
	//
	// This error code does not indicate whether retrying the same operation is safe.
	CodeConflict
)

func parseError(ctx context.Context, err *opendalError) error {
	if err == nil {
		return nil
	}
	defer ffiErrorFree.symbol(ctx)(err)
	return &Error{
		code:    ErrorCode(err.code),
		message: string(parseBytes(err.message)),
	}
}

type Error struct {
	code    ErrorCode
	message string
}

func (e *Error) Error() string {
	return fmt.Sprintf("%d %s", e.code, e.message)
}

func (e *Error) Code() ErrorCode {
	return e.code
}

func (e *Error) Message() string {
	return e.message
}

var ffiErrorFree = newFFI(ffiOpts{
	sym:    "opendal_error_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(e *opendalError) {
	return func(e *opendalError) {
		ffiCall(
			nil,
			unsafe.Pointer(&e),
		)
	}
})
