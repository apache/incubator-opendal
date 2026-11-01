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
	"time"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

// EntryMode indicates whether an entry is a file, directory, or unknown.
type EntryMode uint8

const (
	EntryModeUnknown EntryMode = iota
	EntryModeFile
	EntryModeDir
)

// Metadata represents essential information about a file or directory.
//
// This struct contains attributes commonly used in file systems
// and object storage systems.
type Metadata struct {
	cacheControl       *string
	contentDisposition *string
	contentEncoding    *string
	contentLength      uint64
	contentMD5         *string
	contentType        *string
	etag               *string
	isCurrent          *bool
	isDeleted          bool
	isFile             bool
	isDir              bool
	lastModified       time.Time
	mode               EntryMode
	version            *string
	userMetadata       map[string]string
}

func optionalStringValue(v *string) (string, bool) {
	if v == nil {
		return "", false
	}
	return *v, true
}

func boolPtrFromOptionalByte(v uint8) *bool {
	switch v {
	case 0:
		b := false
		return &b
	case 1:
		b := true
		return &b
	default:
		return nil
	}
}

func newMetadata(ctx context.Context, inner *opendalMetadata) *Metadata {

	var lastModified time.Time
	ms := ffiMetaLastModified.symbol(ctx)(inner)
	if ms != -1 {
		lastModified = time.UnixMilli(ms)
	}

	isCurrent := boolPtrFromOptionalByte(ffiMetaIsCurrent.symbol(ctx)(inner))

	defer ffiMetadataFree.symbol(ctx)(inner)

	return &Metadata{
		cacheControl:       ffiMetaCacheControl.symbol(ctx)(inner),
		contentDisposition: ffiMetaContentDisposition.symbol(ctx)(inner),
		contentEncoding:    ffiMetaContentEncoding.symbol(ctx)(inner),
		contentLength:      ffiMetaContentLength.symbol(ctx)(inner),
		contentMD5:         ffiMetaContentMD5.symbol(ctx)(inner),
		contentType:        ffiMetaContentType.symbol(ctx)(inner),
		etag:               ffiMetaEtag.symbol(ctx)(inner),
		isCurrent:          isCurrent,
		isDeleted:          ffiMetaIsDeleted.symbol(ctx)(inner),
		isFile:             ffiMetaIsFile.symbol(ctx)(inner),
		isDir:              ffiMetaIsDir.symbol(ctx)(inner),
		lastModified:       lastModified,
		mode:               ffiMetaMode.symbol(ctx)(inner),
		version:            ffiMetaVersion.symbol(ctx)(inner),
		userMetadata:       ffiMetaUserMetadata.symbol(ctx)(inner),
	}
}

// CacheControl returns the cache control of the entry.
func (m *Metadata) CacheControl() (string, bool) {
	return optionalStringValue(m.cacheControl)
}

// ContentDisposition returns the content disposition of the entry.
func (m *Metadata) ContentDisposition() (string, bool) {
	return optionalStringValue(m.contentDisposition)
}

// ContentEncoding returns the content encoding of the entry.
func (m *Metadata) ContentEncoding() (string, bool) {
	return optionalStringValue(m.contentEncoding)
}

// ContentMD5 returns the content MD5 of the entry.
func (m *Metadata) ContentMD5() (string, bool) {
	return optionalStringValue(m.contentMD5)
}

// ContentType returns the content type of the entry.
func (m *Metadata) ContentType() (string, bool) {
	return optionalStringValue(m.contentType)
}

// ETag returns the ETag of the entry.
func (m *Metadata) ETag() (string, bool) {
	return optionalStringValue(m.etag)
}

// ContentLength returns the size of the file in bytes.
//
// For directories, this value may not be meaningful and could be zero.
func (m *Metadata) ContentLength() uint64 {
	return m.contentLength
}

// IsFile returns true if the metadata represents a file, false otherwise.
func (m *Metadata) IsFile() bool {
	return m.isFile
}

// IsDir returns true if the metadata represents a directory, false otherwise.
func (m *Metadata) IsDir() bool {
	return m.isDir
}

// IsCurrent returns whether this metadata represents the current version.
//
// The second return value is false when the service doesn't provide this information.
func (m *Metadata) IsCurrent() (bool, bool) {
	if m.isCurrent == nil {
		return false, false
	}
	return *m.isCurrent, true
}

// IsDeleted returns whether this metadata represents a deleted object or version.
func (m *Metadata) IsDeleted() bool {
	return m.isDeleted
}

// LastModified returns the time when the file or directory was last modified.
//
// The returned time is in UTC.
func (m *Metadata) LastModified() time.Time {
	return m.lastModified
}

// Mode returns the mode of the entry.
func (m *Metadata) Mode() EntryMode {
	return m.mode
}

// Version returns the version of the entry.
func (m *Metadata) Version() (string, bool) {
	return optionalStringValue(m.version)
}

// UserMetadata returns the user-defined metadata of the entry.
func (m *Metadata) UserMetadata() map[string]string {
	if m.userMetadata == nil {
		return nil
	}

	metadata := make(map[string]string, len(m.userMetadata))
	for key, value := range m.userMetadata {
		metadata[key] = value
	}
	return metadata
}

var ffiMetaMode = newFFI(ffiOpts{
	sym:    "opendal_metadata_mode",
	rType:  &ffi.TypeUint8,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) EntryMode {
	return func(m *opendalMetadata) EntryMode {
		// libffi may write integer return values using a full register-sized slot
		// even when the declared C return type is u8/bool. Use word-sized
		// storage here, then narrow after the call, to avoid overwriting nearby
		// Go stack memory.
		var mode uint64
		ffiCall(
			unsafe.Pointer(&mode),
			unsafe.Pointer(&m),
		)
		return EntryMode(uint8(mode))
	}
})

var ffiMetaContentLength = newFFI(ffiOpts{
	sym:    "opendal_metadata_content_length",
	rType:  &ffi.TypeUint64,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) uint64 {
	return func(m *opendalMetadata) uint64 {
		var length uint64
		ffiCall(
			unsafe.Pointer(&length),
			unsafe.Pointer(&m),
		)
		return length
	}
})

var ffiMetaCacheControl = newFFIMetadataString("opendal_metadata_cache_control")

var ffiMetaContentDisposition = newFFIMetadataString("opendal_metadata_content_disposition")

var ffiMetaContentMD5 = newFFIMetadataString("opendal_metadata_content_md5")

var ffiMetaContentType = newFFIMetadataString("opendal_metadata_content_type")

var ffiMetaContentEncoding = newFFIMetadataString("opendal_metadata_content_encoding")

var ffiMetaEtag = newFFIMetadataString("opendal_metadata_etag")

var ffiMetaVersion = newFFIMetadataString("opendal_metadata_version")

var ffiMetaIsFile = newFFI(ffiOpts{
	sym:    "opendal_metadata_is_file",
	rType:  &ffi.TypeUint8,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) bool {
	return func(m *opendalMetadata) bool {
		// See ffiMetaMode for why this uses word-sized return storage.
		var result uint64
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&m),
		)
		return uint8(result) == 1
	}
})

var ffiMetaIsDir = newFFI(ffiOpts{
	sym:    "opendal_metadata_is_dir",
	rType:  &ffi.TypeUint8,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) bool {
	return func(m *opendalMetadata) bool {
		// See ffiMetaMode for why this uses word-sized return storage.
		var result uint64
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&m),
		)
		return uint8(result) == 1
	}
})

var ffiMetaIsCurrent = newFFI(ffiOpts{
	sym:    "opendal_metadata_is_current",
	rType:  &ffi.TypeUint8,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) uint8 {
	return func(m *opendalMetadata) uint8 {
		// See ffiMetaMode for why this uses word-sized return storage.
		var result uint64
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&m),
		)
		return uint8(result)
	}
})

var ffiMetaIsDeleted = newFFI(ffiOpts{
	sym:    "opendal_metadata_is_deleted",
	rType:  &ffi.TypeUint8,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) bool {
	return func(m *opendalMetadata) bool {
		// See ffiMetaMode for why this uses word-sized return storage.
		var result uint64
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&m),
		)
		return uint8(result) == 1
	}
})

var ffiMetaLastModified = newFFI(ffiOpts{
	sym:    "opendal_metadata_last_modified_ms",
	rType:  &ffi.TypeSint64,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) int64 {
	return func(m *opendalMetadata) int64 {
		var result int64
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&m),
		)
		return result
	}
})

var ffiMetaUserMetadata = newFFI(ffiOpts{
	sym:    "opendal_metadata_get_user_metadata",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) map[string]string {
	return func(m *opendalMetadata) map[string]string {
		var metadata *opendalMetadataUserMetadata
		ffiCall(
			unsafe.Pointer(&metadata),
			unsafe.Pointer(&m),
		)
		if metadata == nil {
			return nil
		}
		defer ffiMetaUserMetadataFree.symbol(ctx)(metadata)

		length := ffiMetaUserMetadataLen.symbol(ctx)(metadata)
		if length == 0 {
			return map[string]string{}
		}

		pairsPtr := ffiMetaUserMetadataPairs.symbol(ctx)(metadata)
		if pairsPtr == nil {
			return map[string]string{}
		}

		pairs := unsafe.Slice(pairsPtr, int(length))
		result := make(map[string]string, len(pairs))
		for _, pair := range pairs {
			key := BytePtrToString(pair.key)
			value := BytePtrToString(pair.value)
			result[key] = value
		}
		return result
	}
})

var ffiMetaUserMetadataPairs = newFFI(ffiOpts{
	sym:    "opendal_metadata_user_metadata_pairs",
	rType:  &ffi.TypePointer,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadataUserMetadata) *opendalMetadataUserMetadataPair {
	return func(m *opendalMetadataUserMetadata) *opendalMetadataUserMetadataPair {
		var result *opendalMetadataUserMetadataPair
		ffiCall(
			unsafe.Pointer(&result),
			unsafe.Pointer(&m),
		)
		return result
	}
})

var ffiMetaUserMetadataLen = newFFI(ffiOpts{
	sym:    "opendal_metadata_user_metadata_len",
	rType:  &ffi.TypeUint64,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadataUserMetadata) uintptr {
	return func(m *opendalMetadataUserMetadata) uintptr {
		var length uint64
		ffiCall(
			unsafe.Pointer(&length),
			unsafe.Pointer(&m),
		)
		return uintptr(length)
	}
})

var ffiMetaUserMetadataFree = newFFI(ffiOpts{
	sym:    "opendal_metadata_user_metadata_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadataUserMetadata) {
	return func(m *opendalMetadataUserMetadata) {
		ffiCall(
			nil,
			unsafe.Pointer(&m),
		)
	}
})

func newFFIMetadataString(sym string) *FFI[func(m *opendalMetadata) *string] {
	_ = ffiStringFree
	return newFFI(ffiOpts{
		sym:    contextKey(sym),
		rType:  &ffi.TypePointer,
		aTypes: []*ffi.Type{&ffi.TypePointer},
	}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) *string {
		return func(m *opendalMetadata) *string {
			var bytePtr *byte
			ffiCall(
				unsafe.Pointer(&bytePtr),
				unsafe.Pointer(&m),
			)
			if bytePtr == nil {
				return nil
			}
			value := copyCStringAndFree(bytePtr, ffiStringFree.symbol(ctx))
			return &value
		}
	})
}

var ffiMetadataFree = newFFI(ffiOpts{
	sym:    "opendal_metadata_free",
	rType:  &ffi.TypeVoid,
	aTypes: []*ffi.Type{&ffi.TypePointer},
}, func(ctx context.Context, ffiCall ffiCall) func(m *opendalMetadata) {
	return func(m *opendalMetadata) {
		ffiCall(
			nil,
			unsafe.Pointer(&m),
		)
	}
})
