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
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// Info returns metadata about the Operator.
//
// This method provides access to essential information about the Operator,
// including its storage scheme, root path, name, and capabilities.
//
// Returns:
//   - *OperatorInfo: A pointer to an OperatorInfo struct containing the Operator's metadata.
func (op *Operator) Info() *OperatorInfo {
	inner := ffiOperatorInfoNew.symbol(op.ctx)(op.inner)
	defer ffiOperatorInfoFree.symbol(op.ctx)(inner)

	return &OperatorInfo{
		scheme:     ffiOperatorInfoGetScheme.symbol(op.ctx)(inner),
		root:       ffiOperatorInfoGetRoot.symbol(op.ctx)(inner),
		name:       ffiOperatorInfoGetName.symbol(op.ctx)(inner),
		capability: &Capability{inner: ffiOperatorInfoGetCapability.symbol(op.ctx)(inner)},
	}
}

// OperatorInfo provides metadata about an Operator instance.
//
// This struct contains essential information about the storage backend
// and its capabilities, allowing users to query details about the
// Operator they are working with.
type OperatorInfo struct {
	scheme     string
	root       string
	name       string
	capability *Capability
}

func (i *OperatorInfo) GetCapability() *Capability {
	return i.capability
}

func (i *OperatorInfo) GetScheme() string {
	return i.scheme
}

func (i *OperatorInfo) GetRoot() string {
	return i.root
}

func (i *OperatorInfo) GetName() string {
	return i.name
}

// Capability represents the set of operations and features supported by an Operator.
//
// Each field indicates the support level for a specific capability:
//   - bool fields: false indicates no support, true indicates support.
//   - uint fields: Represent size limits or thresholds for certain operations.
//
// This struct covers a wide range of capabilities including:
//   - Basic operations: stat, read, write, delete, copy, rename, list
//   - Advanced features: multipart uploads, presigned URLs, batch operations
//   - Operation modifiers: cache control, content type, if-match conditions
//
// The capability information helps in understanding the functionalities
// available for a specific storage backend or Operator configuration.
type Capability struct {
	inner *opendalCapability
}

func (c *Capability) Stat() bool {
	return c.inner.stat == 1
}

func (c *Capability) StatWithIfMatch() bool {
	return c.inner.statWithIfMatch == 1
}

func (c *Capability) StatWithIfmatch() bool {
	return c.StatWithIfMatch()
}

func (c *Capability) StatWithIfNoneMatch() bool {
	return c.inner.statWithIfNoneMatch == 1
}

func (c *Capability) StatWithIfModifiedSince() bool {
	return c.inner.statWithIfModifiedSince == 1
}

func (c *Capability) StatWithIfUnmodifiedSince() bool {
	return c.inner.statWithIfUnmodifiedSince == 1
}

func (c *Capability) StatWithOverrideCacheControl() bool {
	return c.inner.statWithOverrideCacheControl == 1
}

func (c *Capability) StatWithOverrideContentDisposition() bool {
	return c.inner.statWithOverrideContentDisposition == 1
}

func (c *Capability) StatWithOverrideContentType() bool {
	return c.inner.statWithOverrideContentType == 1
}

func (c *Capability) StatWithVersion() bool {
	return c.inner.statWithVersion == 1
}

func (c *Capability) Read() bool {
	return c.inner.read == 1
}

func (c *Capability) ReadWithIfMatch() bool {
	return c.inner.readWithIfMatch == 1
}

func (c *Capability) ReadWithIfNoneMatch() bool {
	return c.inner.readWithIfNoneMatch == 1
}

func (c *Capability) ReadWithOverrideCacheControl() bool {
	return c.inner.readWithOverrideCacheControl == 1
}

func (c *Capability) ReadWithOverrideContentDisposition() bool {
	return c.inner.readWithOverrideContentDisposition == 1
}

func (c *Capability) ReadWithOverrideContentType() bool {
	return c.inner.readWithOverrideContentType == 1
}

func (c *Capability) ReadWithIfModifiedSince() bool {
	return c.inner.readWithIfModifiedSince == 1
}

func (c *Capability) ReadWithIfUnmodifiedSince() bool {
	return c.inner.readWithIfUnmodifiedSince == 1
}

func (c *Capability) ReadWithVersion() bool {
	return c.inner.readWithVersion == 1
}

func (c *Capability) Write() bool {
	return c.inner.write == 1
}

func (c *Capability) WriteCanMulti() bool {
	return c.inner.writeCanMulti == 1
}

func (c *Capability) WriteCanEmpty() bool {
	return c.inner.writeCanEmpty == 1
}

func (c *Capability) WriteCanAppend() bool {
	return c.inner.writeCanAppend == 1
}

func (c *Capability) WriteWithContentType() bool {
	return c.inner.writeWithContentType == 1
}

func (c *Capability) WriteWithContentDisposition() bool {
	return c.inner.writeWithContentDisposition == 1
}

func (c *Capability) WriteWithCacheControl() bool {
	return c.inner.writeWithCacheControl == 1
}

func (c *Capability) WriteWithContentEncoding() bool {
	return c.inner.writeWithContentEncoding == 1
}

func (c *Capability) WriteWithIfMatch() bool {
	return c.inner.writeWithIfMatch == 1
}

func (c *Capability) WriteWithIfNoneMatch() bool {
	return c.inner.writeWithIfNoneMatch == 1
}

func (c *Capability) WriteWithIfNotExists() bool {
	return c.inner.writeWithIfNotExists == 1
}

func (c *Capability) WriteWithUserMetadata() bool {
	return c.inner.writeWithUserMetadata == 1
}

func (c *Capability) WriteMultiMaxSize() uint {
	return c.inner.writeMultiMaxSize
}

func (c *Capability) WriteMultiMinSize() uint {
	return c.inner.writeMultiMinSize
}
func (c *Capability) WriteTotalMaxSize() uint {
	return c.inner.writeTotalMaxSize
}

func (c *Capability) CreateDir() bool {
	return c.inner.createDir == 1
}

func (c *Capability) Delete() bool {
	return c.inner.delete == 1
}

func (c *Capability) DeleteWithVersion() bool {
	return c.inner.deleteWithVersion == 1
}

func (c *Capability) DeleteWithRecursive() bool {
	return c.inner.deleteWithRecursive == 1
}

func (c *Capability) Copy() bool {
	return c.inner.copy == 1
}

func (c *Capability) CopyWithIfNotExists() bool {
	return c.inner.copyWithIfNotExists == 1
}

func (c *Capability) CopyWithIfMatch() bool {
	return c.inner.copyWithIfMatch == 1
}

func (c *Capability) CopyWithSourceVersion() bool {
	return c.inner.copyWithSourceVersion == 1
}

func (c *Capability) CopyCanMulti() bool {
	return c.inner.copyCanMulti == 1
}

func (c *Capability) CopyMultiMaxSize() uint {
	return c.inner.copyMultiMaxSize
}

func (c *Capability) CopyMultiMinSize() uint {
	return c.inner.copyMultiMinSize
}

func (c *Capability) Rename() bool {
	return c.inner.rename == 1
}

func (c *Capability) List() bool {
	return c.inner.list == 1
}

func (c *Capability) ListWithLimit() bool {
	return c.inner.listWithLimit == 1
}

func (c *Capability) ListWithStartAfter() bool {
	return c.inner.listWithStartAfter == 1
}

func (c *Capability) ListWithRecursive() bool {
	return c.inner.listWithRecursive == 1
}

func (c *Capability) ListWithVersions() bool {
	return c.inner.listWithVersions == 1
}

func (c *Capability) ListWithDeleted() bool {
	return c.inner.listWithDeleted == 1
}

func (c *Capability) Presign() bool {
	return c.inner.presign == 1
}

func (c *Capability) PresignRead() bool {
	return c.inner.presignRead == 1
}

func (c *Capability) PresignStat() bool {
	return c.inner.presignStat == 1
}

func (c *Capability) PresignWrite() bool {
	return c.inner.presignWrite == 1
}

func (c *Capability) PresignDelete() bool {
	return c.inner.presignDelete == 1
}

func (c *Capability) Shared() bool {
	return c.inner.shared == 1
}

func (c *Capability) Blocking() bool {
	return true
}

var ffiOperatorInfoNew = newFFI(ffiOpts{
	sym:    "opendal_operator_info_new",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(op *opendalOperator) *opendalOperatorInfo {
	return func(op *opendalOperator) *opendalOperatorInfo {
		var result *opendalOperatorInfo
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&op),
		)
		return result
	}
})

var ffiOperatorInfoFree = newFFI(ffiOpts{
	sym:    "opendal_operator_info_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(_ context.Context, ffiCall ffiCall) func(info *opendalOperatorInfo) {
	return func(info *opendalOperatorInfo) {
		ffiCall(
			nil,
			unsafe.Pointer(&info),
		)
	}
})

var ffiOperatorInfoGetCapability = newFFI(ffiOpts{
	sym:    "opendal_operator_info_get_capability",
	rType:  &typeCapability,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(info *opendalOperatorInfo) *opendalCapability {
	return func(info *opendalOperatorInfo) *opendalCapability {
		var cap opendalCapability
		ffiCall(
			unsafe.Pointer(&cap),
			unsafe.Pointer(&info),
		)
		return &cap
	}
})

var ffiOperatorInfoGetScheme = func() *FFI[func(info *opendalOperatorInfo) string] {
	_ = ffiStringFree
	return newFFI(ffiOpts{
		sym:    "opendal_operator_info_get_scheme",
		rType:  &ffi.TypePointer,
		aTypes: []*ffi.Type{&ffi.TypePointer},
	}, func(ctx context.Context, ffiCall ffiCall) func(info *opendalOperatorInfo) string {
		return func(info *opendalOperatorInfo) string {
			var bytePtr *byte
			ffiCall(
				unsafe.Pointer(&bytePtr),
				unsafe.Pointer(&info),
			)
			return copyCStringAndFree(bytePtr, ffiStringFree.symbol(ctx))
		}
	})
}()

var ffiOperatorInfoGetRoot = func() *FFI[func(info *opendalOperatorInfo) string] {
	_ = ffiStringFree
	return newFFI(ffiOpts{
		sym:    "opendal_operator_info_get_root",
		rType:  &ffi.TypePointer,
		aTypes: []*ffi.Type{&ffi.TypePointer},
	}, func(ctx context.Context, ffiCall ffiCall) func(info *opendalOperatorInfo) string {
		return func(info *opendalOperatorInfo) string {
			var bytePtr *byte
			ffiCall(
				unsafe.Pointer(&bytePtr),
				unsafe.Pointer(&info),
			)
			return copyCStringAndFree(bytePtr, ffiStringFree.symbol(ctx))
		}
	})
}()

var ffiOperatorInfoGetName = func() *FFI[func(info *opendalOperatorInfo) string] {
	_ = ffiStringFree
	return newFFI(ffiOpts{
		sym:    "opendal_operator_info_get_name",
		rType:  &ffi.TypePointer,
		aTypes: []*ffi.Type{&ffi.TypePointer},
	}, func(ctx context.Context, ffiCall ffiCall) func(info *opendalOperatorInfo) string {
		return func(info *opendalOperatorInfo) string {
			var bytePtr *byte
			ffiCall(
				unsafe.Pointer(&bytePtr),
				unsafe.Pointer(&info),
			)
			return copyCStringAndFree(bytePtr, ffiStringFree.symbol(ctx))
		}
	})
}()
