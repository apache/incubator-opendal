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
	"net/http"
	"slices"
	"testing"
	"time"
	"unsafe"

	"github.com/jupiterrider/ffi"
)

func TestEntryFreeUsesVoidReturnType(t *testing.T) {
	if ffiEntryFree.opts.rType != &ffi.TypeVoid {
		t.Fatalf("ffiEntryFree return type = %v, want void", ffiEntryFree.opts.rType)
	}
}

func TestCopyCStringAndFreeNil(t *testing.T) {
	var freed int
	freeCString := func(*byte) {
		freed++
	}

	got := copyCStringAndFree(nil, freeCString)
	if got != "" {
		t.Fatalf("copyCStringAndFree(nil) = %q, want empty string", got)
	}
	if freed != 0 {
		t.Fatalf("copyCStringAndFree(nil) freed %d pointers, want 0", freed)
	}
}

func TestOperatorInfoCopiesAndFreesOwnedStrings(t *testing.T) {
	var freed []*byte
	freeCString := func(ptr *byte) {
		freed = append(freed, ptr)
	}

	schemePtr := mustBytePtrFromString(t, "memory")
	rootPtr := mustBytePtrFromString(t, "/tmp/")
	namePtr := mustBytePtrFromString(t, "namespace")
	infoInner := &opendalOperatorInfo{}
	capability := &opendalCapability{stat: 1}
	infoFreed := 0

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiStringFree.opts.sym, freeCString)
	ctx = context.WithValue(ctx, ffiOperatorInfoNew.opts.sym, func(op *opendalOperator) *opendalOperatorInfo {
		if op == nil {
			t.Fatal("Info() passed nil operator")
		}
		return infoInner
	})
	ctx = context.WithValue(ctx, ffiOperatorInfoFree.opts.sym, func(info *opendalOperatorInfo) {
		if info != infoInner {
			t.Fatalf("Info() freed unexpected operator info: %p", info)
		}
		infoFreed++
	})
	ctx = context.WithValue(ctx, ffiOperatorInfoGetScheme.opts.sym, ffiOperatorInfoGetScheme.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertOperatorInfoPointer(t, infoInner, aValues...)
		*(**byte)(rValue) = schemePtr
	}))
	ctx = context.WithValue(ctx, ffiOperatorInfoGetRoot.opts.sym, ffiOperatorInfoGetRoot.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertOperatorInfoPointer(t, infoInner, aValues...)
		*(**byte)(rValue) = rootPtr
	}))
	ctx = context.WithValue(ctx, ffiOperatorInfoGetName.opts.sym, ffiOperatorInfoGetName.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertOperatorInfoPointer(t, infoInner, aValues...)
		*(**byte)(rValue) = namePtr
	}))
	ctx = context.WithValue(ctx, ffiOperatorInfoGetCapability.opts.sym, func(info *opendalOperatorInfo) *opendalCapability {
		if info != infoInner {
			t.Fatalf("Info() requested full capability for unexpected operator info: %p", info)
		}
		return capability
	})

	op := &Operator{
		ctx:   ctx,
		inner: &opendalOperator{},
	}

	info := op.Info()
	if info.GetScheme() != "memory" {
		t.Fatalf("Info().GetScheme() = %q, want memory", info.GetScheme())
	}
	if info.GetRoot() != "/tmp/" {
		t.Fatalf("Info().GetRoot() = %q, want /tmp/", info.GetRoot())
	}
	if info.GetName() != "namespace" {
		t.Fatalf("Info().GetName() = %q, want namespace", info.GetName())
	}
	if !info.GetCapability().Stat() {
		t.Fatal("Info().GetCapability().Stat() = false, want true")
	}
	if infoFreed != 1 {
		t.Fatalf("Info() freed operator info %d times, want 1", infoFreed)
	}
	assertFreedPointers(t, freed, schemePtr, rootPtr, namePtr)
}

func TestNewEntryCopiesAndFreesOwnedStrings(t *testing.T) {
	var freed []*byte
	freeCString := func(ptr *byte) {
		freed = append(freed, ptr)
	}

	namePtr := mustBytePtrFromString(t, "file.txt")
	pathPtr := mustBytePtrFromString(t, "dir/file.txt")
	entryInner := &opendalEntry{}
	entryFreed := 0

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiStringFree.opts.sym, freeCString)
	ctx = context.WithValue(ctx, ffiEntryFree.opts.sym, func(entry *opendalEntry) {
		if entry != entryInner {
			t.Fatalf("newEntry() freed unexpected entry: %p", entry)
		}
		entryFreed++
	})
	ctx = context.WithValue(ctx, ffiEntryName.opts.sym, ffiEntryName.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertEntryPointer(t, entryInner, aValues...)
		*(**byte)(rValue) = namePtr
	}))
	ctx = context.WithValue(ctx, ffiEntryPath.opts.sym, ffiEntryPath.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertEntryPointer(t, entryInner, aValues...)
		*(**byte)(rValue) = pathPtr
	}))

	// Mock the new metadata path exercised by newEntry()
	metaInner := &opendalMetadata{}
	metaFreed := 0
	ctx = context.WithValue(ctx, ffiEntryMetadata.opts.sym, func(e *opendalEntry) *opendalMetadata {
		if e != entryInner {
			t.Fatalf("ffiEntryMetadata called with unexpected entry")
		}
		return metaInner
	})
	ctx = context.WithValue(ctx, ffiMetadataFree.opts.sym, func(m *opendalMetadata) {
		if m != metaInner {
			t.Fatalf("metadata freed unexpected pointer: %p", m)
		}
		metaFreed++
	})
	ctx = context.WithValue(ctx, ffiMetaCacheControl.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentDisposition.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentEncoding.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentLength.opts.sym, func(m *opendalMetadata) uint64 {
		assertMetadataPointer(t, metaInner, m)
		return 4096
	})
	ctx = context.WithValue(ctx, ffiMetaContentMD5.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})
	contentType := "text/plain"
	ctx = context.WithValue(ctx, ffiMetaContentType.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return &contentType
	})
	ctx = context.WithValue(ctx, ffiMetaEtag.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaIsCurrent.opts.sym, func(m *opendalMetadata) uint8 {
		assertMetadataPointer(t, metaInner, m)
		return 2
	})
	ctx = context.WithValue(ctx, ffiMetaIsDeleted.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metaInner, m)
		return false
	})
	ctx = context.WithValue(ctx, ffiMetaIsFile.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metaInner, m)
		return true
	})
	ctx = context.WithValue(ctx, ffiMetaIsDir.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metaInner, m)
		return false
	})
	ctx = context.WithValue(ctx, ffiMetaLastModified.opts.sym, func(m *opendalMetadata) int64 {
		assertMetadataPointer(t, metaInner, m)
		return 1700000000000
	})
	ctx = context.WithValue(ctx, ffiMetaMode.opts.sym, func(m *opendalMetadata) EntryMode {
		assertMetadataPointer(t, metaInner, m)
		return EntryModeFile
	})
	ctx = context.WithValue(ctx, ffiMetaVersion.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaUserMetadata.opts.sym, func(m *opendalMetadata) map[string]string {
		assertMetadataPointer(t, metaInner, m)
		return nil
	})

	entry := newEntry(ctx, entryInner)
	if entry.Name() != "file.txt" {
		t.Fatalf("newEntry().Name() = %q, want file.txt", entry.Name())
	}
	if entry.Path() != "dir/file.txt" {
		t.Fatalf("newEntry().Path() = %q, want dir/file.txt", entry.Path())
	}
	if entryFreed != 1 {
		t.Fatalf("newEntry() freed entry %d times, want 1", entryFreed)
	}
	assertFreedPointers(t, freed, namePtr, pathPtr)

	// Verify new Metadata() support
	meta := entry.Metadata()
	if meta == nil {
		t.Fatal("newEntry().Metadata() returned nil, expected non-nil")
	}
	if meta.ContentLength() != 4096 {
		t.Fatalf("Metadata().ContentLength() = %d, want 4096", meta.ContentLength())
	}
	if !meta.IsFile() {
		t.Fatal("Metadata().IsFile() = false, want true")
	}
	if meta.IsDir() {
		t.Fatal("Metadata().IsDir() = true, want false")
	}
	if meta.LastModified().UnixMilli() != 1700000000000 {
		t.Fatalf("Metadata().LastModified().UnixMilli() = %d, want 1700000000000", meta.LastModified().UnixMilli())
	}
	if meta.Mode() != EntryModeFile {
		t.Fatalf("Metadata().Mode() = %d, want %d", meta.Mode(), EntryModeFile)
	}
	if contentType, ok := meta.ContentType(); !ok || contentType != "text/plain" {
		t.Fatalf("Metadata().ContentType() = %q, %v, want text/plain, true", contentType, ok)
	}
	if _, ok := meta.CacheControl(); ok {
		t.Fatal("Metadata().CacheControl() ok = true, want false")
	}
	if metaFreed != 1 {
		t.Fatalf("newEntry() freed metadata %d times, want 1", metaFreed)
	}
}

func TestNewMetadataCopiesAllFieldsAndFreesOwnedValues(t *testing.T) {
	var freed []*byte
	freeCString := func(ptr *byte) {
		freed = append(freed, ptr)
	}

	cacheControlPtr := mustBytePtrFromString(t, "max-age=60")
	contentDispositionPtr := mustBytePtrFromString(t, "attachment")
	contentEncodingPtr := mustBytePtrFromString(t, "gzip")
	contentMD5Ptr := mustBytePtrFromString(t, "1B2M2Y8AsgTpgAmY7PhCfg==")
	contentTypePtr := mustBytePtrFromString(t, "application/json")
	etagPtr := mustBytePtrFromString(t, `"etag"`)
	versionPtr := mustBytePtrFromString(t, "v42")

	metadataInner := &opendalMetadata{}
	userMetadataInner := &opendalMetadataUserMetadata{}
	pairs := []opendalMetadataUserMetadataPair{
		{key: mustBytePtrFromString(t, "foo"), value: mustBytePtrFromString(t, "bar")},
		{key: mustBytePtrFromString(t, "empty"), value: mustBytePtrFromString(t, "")},
	}
	metadataFreed := 0
	userMetadataFreed := 0

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiStringFree.opts.sym, freeCString)
	ctx = context.WithValue(ctx, ffiMetadataFree.opts.sym, func(m *opendalMetadata) {
		assertMetadataPointer(t, metadataInner, m)
		metadataFreed++
	})
	ctx = context.WithValue(ctx, ffiMetaCacheControl.opts.sym, ffiMetaCacheControl.withFunc(ctx, metadataStringFunc(t, metadataInner, cacheControlPtr)))
	ctx = context.WithValue(ctx, ffiMetaContentDisposition.opts.sym, ffiMetaContentDisposition.withFunc(ctx, metadataStringFunc(t, metadataInner, contentDispositionPtr)))
	ctx = context.WithValue(ctx, ffiMetaContentEncoding.opts.sym, ffiMetaContentEncoding.withFunc(ctx, metadataStringFunc(t, metadataInner, contentEncodingPtr)))
	ctx = context.WithValue(ctx, ffiMetaContentLength.opts.sym, func(m *opendalMetadata) uint64 {
		assertMetadataPointer(t, metadataInner, m)
		return 4096
	})
	ctx = context.WithValue(ctx, ffiMetaContentMD5.opts.sym, ffiMetaContentMD5.withFunc(ctx, metadataStringFunc(t, metadataInner, contentMD5Ptr)))
	ctx = context.WithValue(ctx, ffiMetaContentType.opts.sym, ffiMetaContentType.withFunc(ctx, metadataStringFunc(t, metadataInner, contentTypePtr)))
	ctx = context.WithValue(ctx, ffiMetaEtag.opts.sym, ffiMetaEtag.withFunc(ctx, metadataStringFunc(t, metadataInner, etagPtr)))
	ctx = context.WithValue(ctx, ffiMetaIsCurrent.opts.sym, func(m *opendalMetadata) uint8 {
		assertMetadataPointer(t, metadataInner, m)
		return 1
	})
	ctx = context.WithValue(ctx, ffiMetaIsDeleted.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metadataInner, m)
		return true
	})
	ctx = context.WithValue(ctx, ffiMetaIsFile.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metadataInner, m)
		return false
	})
	ctx = context.WithValue(ctx, ffiMetaIsDir.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metadataInner, m)
		return true
	})
	ctx = context.WithValue(ctx, ffiMetaLastModified.opts.sym, func(m *opendalMetadata) int64 {
		assertMetadataPointer(t, metadataInner, m)
		return 1700000000000
	})
	ctx = context.WithValue(ctx, ffiMetaMode.opts.sym, func(m *opendalMetadata) EntryMode {
		assertMetadataPointer(t, metadataInner, m)
		return EntryModeDir
	})
	ctx = context.WithValue(ctx, ffiMetaVersion.opts.sym, ffiMetaVersion.withFunc(ctx, metadataStringFunc(t, metadataInner, versionPtr)))
	ctx = context.WithValue(ctx, ffiMetaUserMetadataPairs.opts.sym, ffiMetaUserMetadataPairs.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertUserMetadataPointerFromArgs(t, userMetadataInner, aValues...)
		*(**opendalMetadataUserMetadataPair)(rValue) = &pairs[0]
	}))
	ctx = context.WithValue(ctx, ffiMetaUserMetadataLen.opts.sym, ffiMetaUserMetadataLen.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertUserMetadataPointerFromArgs(t, userMetadataInner, aValues...)
		*(*uint64)(rValue) = uint64(len(pairs))
	}))
	ctx = context.WithValue(ctx, ffiMetaUserMetadataFree.opts.sym, func(m *opendalMetadataUserMetadata) {
		if m != userMetadataInner {
			t.Fatalf("user metadata freed unexpected pointer: %p", m)
		}
		userMetadataFreed++
	})
	ctx = context.WithValue(ctx, ffiMetaUserMetadata.opts.sym, ffiMetaUserMetadata.withFunc(ctx, func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertMetadataPointerFromArgs(t, metadataInner, aValues...)
		*(**opendalMetadataUserMetadata)(rValue) = userMetadataInner
	}))

	meta := newMetadata(ctx, metadataInner)
	assertOptionalString(t, "CacheControl", meta.CacheControl, "max-age=60")
	assertOptionalString(t, "ContentDisposition", meta.ContentDisposition, "attachment")
	assertOptionalString(t, "ContentEncoding", meta.ContentEncoding, "gzip")
	assertOptionalString(t, "ContentMD5", meta.ContentMD5, "1B2M2Y8AsgTpgAmY7PhCfg==")
	assertOptionalString(t, "ContentType", meta.ContentType, "application/json")
	assertOptionalString(t, "ETag", meta.ETag, `"etag"`)
	assertOptionalString(t, "Version", meta.Version, "v42")
	if meta.ContentLength() != 4096 {
		t.Fatalf("ContentLength() = %d, want 4096", meta.ContentLength())
	}
	current, ok := meta.IsCurrent()
	if !ok || !current {
		t.Fatalf("IsCurrent() = %v, %v, want true, true", current, ok)
	}
	if !meta.IsDeleted() {
		t.Fatal("IsDeleted() = false, want true")
	}
	if meta.IsFile() {
		t.Fatal("IsFile() = true, want false")
	}
	if !meta.IsDir() {
		t.Fatal("IsDir() = false, want true")
	}
	if meta.LastModified().UnixMilli() != 1700000000000 {
		t.Fatalf("LastModified().UnixMilli() = %d, want 1700000000000", meta.LastModified().UnixMilli())
	}
	if meta.Mode() != EntryModeDir {
		t.Fatalf("Mode() = %d, want %d", meta.Mode(), EntryModeDir)
	}
	assertStringMap(t, meta.UserMetadata(), map[string]string{"foo": "bar", "empty": ""})

	userMetadata := meta.UserMetadata()
	userMetadata["foo"] = "changed"
	assertStringMap(t, meta.UserMetadata(), map[string]string{"foo": "bar", "empty": ""})

	if metadataFreed != 1 {
		t.Fatalf("metadata freed %d times, want 1", metadataFreed)
	}
	if userMetadataFreed != 1 {
		t.Fatalf("user metadata freed %d times, want 1", userMetadataFreed)
	}
	assertFreedPointers(t, freed, cacheControlPtr, contentDispositionPtr, contentEncodingPtr, contentMD5Ptr, contentTypePtr, etagPtr, versionPtr)
}

func TestNewMetadataOptionalValuesCanBeAbsent(t *testing.T) {
	metadataInner := &opendalMetadata{}
	metadataFreed := 0

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiMetadataFree.opts.sym, func(m *opendalMetadata) {
		assertMetadataPointer(t, metadataInner, m)
		metadataFreed++
	})
	ctx = context.WithValue(ctx, ffiMetaCacheControl.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentDisposition.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentEncoding.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentLength.opts.sym, func(m *opendalMetadata) uint64 {
		assertMetadataPointer(t, metadataInner, m)
		return 0
	})
	ctx = context.WithValue(ctx, ffiMetaContentMD5.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaContentType.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaEtag.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaIsCurrent.opts.sym, func(m *opendalMetadata) uint8 {
		assertMetadataPointer(t, metadataInner, m)
		return 2
	})
	ctx = context.WithValue(ctx, ffiMetaIsDeleted.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metadataInner, m)
		return false
	})
	ctx = context.WithValue(ctx, ffiMetaIsFile.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metadataInner, m)
		return false
	})
	ctx = context.WithValue(ctx, ffiMetaIsDir.opts.sym, func(m *opendalMetadata) bool {
		assertMetadataPointer(t, metadataInner, m)
		return false
	})
	ctx = context.WithValue(ctx, ffiMetaLastModified.opts.sym, func(m *opendalMetadata) int64 {
		assertMetadataPointer(t, metadataInner, m)
		return -1
	})
	ctx = context.WithValue(ctx, ffiMetaMode.opts.sym, func(m *opendalMetadata) EntryMode {
		assertMetadataPointer(t, metadataInner, m)
		return EntryModeUnknown
	})
	ctx = context.WithValue(ctx, ffiMetaVersion.opts.sym, func(m *opendalMetadata) *string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})
	ctx = context.WithValue(ctx, ffiMetaUserMetadata.opts.sym, func(m *opendalMetadata) map[string]string {
		assertMetadataPointer(t, metadataInner, m)
		return nil
	})

	meta := newMetadata(ctx, metadataInner)
	if _, ok := meta.CacheControl(); ok {
		t.Fatal("CacheControl() ok = true, want false")
	}
	if _, ok := meta.ContentDisposition(); ok {
		t.Fatal("ContentDisposition() ok = true, want false")
	}
	if _, ok := meta.ContentEncoding(); ok {
		t.Fatal("ContentEncoding() ok = true, want false")
	}
	if _, ok := meta.ContentMD5(); ok {
		t.Fatal("ContentMD5() ok = true, want false")
	}
	if _, ok := meta.ContentType(); ok {
		t.Fatal("ContentType() ok = true, want false")
	}
	if _, ok := meta.ETag(); ok {
		t.Fatal("ETag() ok = true, want false")
	}
	if _, ok := meta.Version(); ok {
		t.Fatal("Version() ok = true, want false")
	}
	if current, ok := meta.IsCurrent(); ok || current {
		t.Fatalf("IsCurrent() = %v, %v, want false, false", current, ok)
	}
	if !meta.LastModified().IsZero() {
		t.Fatalf("LastModified() = %v, want zero", meta.LastModified())
	}
	if meta.UserMetadata() != nil {
		t.Fatalf("UserMetadata() = %v, want nil", meta.UserMetadata())
	}
	if metadataFreed != 1 {
		t.Fatalf("metadata freed %d times, want 1", metadataFreed)
	}
}

func TestBoolPtrFromOptionalByte(t *testing.T) {
	if got := boolPtrFromOptionalByte(2); got != nil {
		t.Fatalf("boolPtrFromOptionalByte(2) = %v, want nil", *got)
	}
	for _, tc := range []struct {
		value uint8
		want  bool
	}{
		{value: 0, want: false},
		{value: 1, want: true},
	} {
		got := boolPtrFromOptionalByte(tc.value)
		if got == nil || *got != tc.want {
			t.Fatalf("boolPtrFromOptionalByte(%d) = %v, want %v", tc.value, got, tc.want)
		}
	}
}

func mustBytePtrFromString(t *testing.T, value string) *byte {
	t.Helper()

	ptr, err := BytePtrFromString(value)
	if err != nil {
		t.Fatalf("BytePtrFromString(%q) failed: %v", value, err)
	}
	return ptr
}

func metadataStringFunc(t *testing.T, want *opendalMetadata, ptr *byte) func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
	t.Helper()
	return func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertMetadataPointerFromArgs(t, want, aValues...)
		*(**byte)(rValue) = ptr
	}
}

func assertOptionalString(t *testing.T, name string, fn func() (string, bool), want string) {
	t.Helper()

	got, ok := fn()
	if !ok || got != want {
		t.Fatalf("%s() = %q, %v, want %q, true", name, got, ok, want)
	}
}

func assertStringMap(t *testing.T, got, want map[string]string) {
	t.Helper()

	if len(got) != len(want) {
		t.Fatalf("map length = %d, want %d: %v", len(got), len(want), got)
	}
	for key, wantValue := range want {
		if gotValue, ok := got[key]; !ok || gotValue != wantValue {
			t.Fatalf("map[%q] = %q, %v, want %q, true", key, gotValue, ok, wantValue)
		}
	}
}

func assertOperatorInfoPointer(t *testing.T, want *opendalOperatorInfo, aValues ...unsafe.Pointer) {
	t.Helper()

	if len(aValues) != 1 {
		t.Fatalf("operator info getter received %d arguments, want 1", len(aValues))
	}
	got := *(**opendalOperatorInfo)(aValues[0])
	if got != want {
		t.Fatalf("operator info getter received %p, want %p", got, want)
	}
}

func assertEntryPointer(t *testing.T, want *opendalEntry, aValues ...unsafe.Pointer) {
	t.Helper()

	if len(aValues) != 1 {
		t.Fatalf("entry getter received %d arguments, want 1", len(aValues))
	}
	got := *(**opendalEntry)(aValues[0])
	if got != want {
		t.Fatalf("entry getter received %p, want %p", got, want)
	}
}

func assertMetadataPointer(t *testing.T, want *opendalMetadata, got *opendalMetadata) {
	t.Helper()

	if got != want {
		t.Fatalf("metadata getter received %p, want %p", got, want)
	}
}

func assertMetadataPointerFromArgs(t *testing.T, want *opendalMetadata, aValues ...unsafe.Pointer) {
	t.Helper()

	if len(aValues) != 1 {
		t.Fatalf("metadata getter received %d arguments, want 1", len(aValues))
	}
	got := *(**opendalMetadata)(aValues[0])
	assertMetadataPointer(t, want, got)
}

func assertUserMetadataPointerFromArgs(t *testing.T, want *opendalMetadataUserMetadata, aValues ...unsafe.Pointer) {
	t.Helper()

	if len(aValues) != 1 {
		t.Fatalf("user metadata getter received %d arguments, want 1", len(aValues))
	}
	got := *(**opendalMetadataUserMetadata)(aValues[0])
	if got != want {
		t.Fatalf("user metadata getter received %p, want %p", got, want)
	}
}

func assertFreedPointers(t *testing.T, got []*byte, want ...*byte) {
	t.Helper()

	if len(got) != len(want) {
		t.Fatalf("freed %d pointers, want %d", len(got), len(want))
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("freed pointer[%d] = %p, want %p", i, got[i], want[i])
		}
	}
}

func TestWriteWithOptions(t *testing.T) {
	o := &writeOptions{}
	WriteWithAppend(true)(o)
	WriteWithCacheControl("max-age=60")(o)
	WriteWithContentType("text/plain")(o)
	WriteWithContentDisposition("attachment")(o)
	WriteWithContentEncoding("gzip")(o)
	WriteWithUserMetadata(map[string]string{"foo": "bar"})(o)
	WriteWithIfMatch("etag-a")(o)
	WriteWithIfNoneMatch("etag-b")(o)
	WriteWithIfNotExists(true)(o)
	WriteWithConcurrent(4)(o)
	WriteWithChunk(1024)(o)

	if !o.append {
		t.Fatalf("append = false, want true")
	}
	if o.cacheControl != "max-age=60" {
		t.Fatalf("cacheControl = %q, want max-age=60", o.cacheControl)
	}
	if o.contentType != "text/plain" {
		t.Fatalf("contentType = %q, want text/plain", o.contentType)
	}
	if o.contentDisposition != "attachment" {
		t.Fatalf("contentDisposition = %q, want attachment", o.contentDisposition)
	}
	if o.contentEncoding != "gzip" {
		t.Fatalf("contentEncoding = %q, want gzip", o.contentEncoding)
	}
	assertStringMap(t, o.userMetadata, map[string]string{"foo": "bar"})
	if o.ifMatch != "etag-a" {
		t.Fatalf("ifMatch = %q, want etag-a", o.ifMatch)
	}
	if o.ifNoneMatch != "etag-b" {
		t.Fatalf("ifNoneMatch = %q, want etag-b", o.ifNoneMatch)
	}
	if !o.ifNotExists {
		t.Fatalf("ifNotExists = false, want true")
	}
	if o.concurrent != 4 {
		t.Fatalf("concurrent = %d, want 4", o.concurrent)
	}
	if o.chunk != 1024 {
		t.Fatalf("chunk = %d, want 1024", o.chunk)
	}
}

func TestFfiOperatorWriterWithArgTypes(t *testing.T) {
	aTypes := ffiOperatorWriterWith.opts.aTypes
	if len(aTypes) != 3 {
		t.Fatalf("ffiOperatorWriterWith aTypes len = %d, want 3", len(aTypes))
	}
	for i, at := range aTypes {
		if at != &ffi.TypePointer {
			t.Fatalf("ffiOperatorWriterWith aTypes[%d] = %v, want TypePointer", i, at)
		}
	}
}

func TestFfiOperatorWriteWithMetadataSignature(t *testing.T) {
	if ffiOperatorWriteWithMetadata.opts.rType != &typeResultWrite {
		t.Fatalf("ffiOperatorWriteWithMetadata rType = %v, want typeResultWrite", ffiOperatorWriteWithMetadata.opts.rType)
	}
	aTypes := ffiOperatorWriteWithMetadata.opts.aTypes
	if len(aTypes) != 4 {
		t.Fatalf("ffiOperatorWriteWithMetadata aTypes len = %d, want 4", len(aTypes))
	}
	for i, at := range aTypes {
		if at != &ffi.TypePointer {
			t.Fatalf("ffiOperatorWriteWithMetadata aTypes[%d] = %v, want TypePointer", i, at)
		}
	}
}

func TestFfiWriterCloseWithMetadataSignature(t *testing.T) {
	if ffiWriterCloseWithMetadata.opts.rType != &typeResultWrite {
		t.Fatalf("ffiWriterCloseWithMetadata rType = %v, want typeResultWrite", ffiWriterCloseWithMetadata.opts.rType)
	}
	aTypes := ffiWriterCloseWithMetadata.opts.aTypes
	if len(aTypes) != 1 {
		t.Fatalf("ffiWriterCloseWithMetadata aTypes len = %d, want 1", len(aTypes))
	}
	if aTypes[0] != &ffi.TypePointer {
		t.Fatalf("ffiWriterCloseWithMetadata aTypes[0] = %v, want TypePointer", aTypes[0])
	}
}

func TestWriteOptionsSetterArgTypes(t *testing.T) {
	stringSetters := []*FFI[func(*opendalWriteOptions, string) ([]byte, error)]{
		ffiWriteOptionsSetCacheControl,
		ffiWriteOptionsSetContentType,
		ffiWriteOptionsSetContentDisposition,
		ffiWriteOptionsSetContentEncoding,
		ffiWriteOptionsSetIfMatch,
		ffiWriteOptionsSetIfNoneMatch,
	}
	for _, setter := range stringSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		for i, at := range aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", setter.opts.sym, i, at)
			}
		}
	}

	boolSetters := []*FFI[func(*opendalWriteOptions, bool)]{
		ffiWriteOptionsSetAppend,
		ffiWriteOptionsSetIfNotExists,
	}
	for _, setter := range boolSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		if aTypes[0] != &ffi.TypePointer || aTypes[1] != &ffi.TypeUint8 {
			t.Fatalf("%s aTypes = %v, want TypePointer, TypeUint8", setter.opts.sym, aTypes)
		}
	}

	uintSetters := []*FFI[func(*opendalWriteOptions, uint)]{
		ffiWriteOptionsSetConcurrent,
		ffiWriteOptionsSetChunk,
	}
	for _, setter := range uintSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		for i, at := range aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", setter.opts.sym, i, at)
			}
		}
	}
}

func TestWriteOptionsSetUserMetadataArgTypes(t *testing.T) {
	aTypes := ffiWriteOptionsSetUserMetadata.opts.aTypes
	if len(aTypes) != 3 {
		t.Fatalf("ffiWriteOptionsSetUserMetadata aTypes len = %d, want 3", len(aTypes))
	}
	for i, at := range aTypes {
		if at != &ffi.TypePointer {
			t.Fatalf("ffiWriteOptionsSetUserMetadata aTypes[%d] = %v, want TypePointer", i, at)
		}
	}
}

func TestListWithRecursiveDefaultNotRecursive(t *testing.T) {
	o := &listOptions{}
	if o.recursive {
		t.Fatalf("default listOptions.recursive = true, want false")
	}
}

func TestListWithRecursiveTrue(t *testing.T) {
	o := &listOptions{}
	ListWithRecursive(true)(o)
	if !o.recursive {
		t.Fatalf("ListWithRecursive(true): recursive = false, want true")
	}
}

func TestListWithRecursiveFalse(t *testing.T) {
	o := &listOptions{}
	ListWithRecursive(true)(o)
	ListWithRecursive(false)(o)
	if o.recursive {
		t.Fatalf("ListWithRecursive(false): recursive = true, want false")
	}
}

func TestListWithLimitSetsValue(t *testing.T) {
	o := &listOptions{}
	ListWithLimit(100)(o)
	if o.limit != 100 {
		t.Fatalf("ListWithLimit(100): limit = %d, want 100", o.limit)
	}
}

func TestListWithLimitOverwrite(t *testing.T) {
	o := &listOptions{}
	ListWithLimit(50)(o)
	ListWithLimit(200)(o)
	if o.limit != 200 {
		t.Fatalf("ListWithLimit overwrite: limit = %d, want 200", o.limit)
	}
}

func TestListWithStartAfterSetsValue(t *testing.T) {
	o := &listOptions{}
	ListWithStartAfter("some/key")(o)
	if o.startAfter == nil {
		t.Fatal("ListWithStartAfter: startAfter = nil, want non-nil")
	}
	if *o.startAfter != "some/key" {
		t.Fatalf("ListWithStartAfter: startAfter = %q, want some/key", *o.startAfter)
	}
}

func TestListWithStartAfterEmptyString(t *testing.T) {
	o := &listOptions{}
	ListWithStartAfter("")(o)
	if o.startAfter == nil {
		t.Fatal("ListWithStartAfter(empty): startAfter = nil, want non-nil pointer to empty string")
	}
}

func TestListWithVersionsTrue(t *testing.T) {
	o := &listOptions{}
	ListWithVersions(true)(o)
	if !o.versions {
		t.Fatalf("ListWithVersions(true): versions = false, want true")
	}
}

func TestListWithVersionsFalse(t *testing.T) {
	o := &listOptions{}
	ListWithVersions(true)(o)
	ListWithVersions(false)(o)
	if o.versions {
		t.Fatalf("ListWithVersions(false): versions = true, want false")
	}
}

func TestListWithDeletedTrue(t *testing.T) {
	o := &listOptions{}
	ListWithDeleted(true)(o)
	if !o.deleted {
		t.Fatalf("ListWithDeleted(true): deleted = false, want true")
	}
}

func TestListWithDeletedFalse(t *testing.T) {
	o := &listOptions{}
	ListWithDeleted(true)(o)
	ListWithDeleted(false)(o)
	if o.deleted {
		t.Fatalf("ListWithDeleted(false): deleted = true, want false")
	}
}

func TestFfiOperatorListWithReturnType(t *testing.T) {
	if ffiOperatorListWith.opts.rType != &typeResultList {
		t.Fatalf("ffiOperatorListWith rType = %v, want typeResultList", ffiOperatorListWith.opts.rType)
	}
}

func TestFfiOperatorListWithArgTypes(t *testing.T) {
	aTypes := ffiOperatorListWith.opts.aTypes
	if len(aTypes) != 3 {
		t.Fatalf("ffiOperatorListWith aTypes len = %d, want 3", len(aTypes))
	}
	for i, at := range aTypes {
		if at != &ffi.TypePointer {
			t.Fatalf("ffiOperatorListWith aTypes[%d] = %v, want TypePointer", i, at)
		}
	}
}

func TestReadWithOptions(t *testing.T) {
	modified := time.Unix(1700000000, 0)
	unmodified := time.Unix(1700000123, 0)

	o := &readOptions{}
	ReadWithRange(1024, 2048)(o)
	ReadWithVersion("v1")(o)
	ReadWithIfMatch("etag-a")(o)
	ReadWithIfNoneMatch("etag-b")(o)
	ReadWithIfModifiedSince(modified)(o)
	ReadWithIfUnmodifiedSince(unmodified)(o)
	ReadWithConcurrent(4)(o)
	ReadWithChunk(1024)(o)
	ReadWithGap(512)(o)
	ReadWithContentLengthHint(4096)(o)
	ReadWithOverrideContentType("text/plain")(o)
	ReadWithOverrideCacheControl("max-age=60")(o)
	ReadWithOverrideContentDisposition("attachment")(o)

	if !o.hasRange {
		t.Fatalf("hasRange = false, want true")
	}
	if !o.hasRangeLength {
		t.Fatalf("hasRangeLength = false, want true")
	}
	if o.rangeOffset != 1024 {
		t.Fatalf("rangeOffset = %d, want 1024", o.rangeOffset)
	}
	if o.rangeLength != 2048 {
		t.Fatalf("rangeLength = %d, want 2048", o.rangeLength)
	}
	if o.version != "v1" {
		t.Fatalf("version = %q, want v1", o.version)
	}
	if o.ifMatch != "etag-a" {
		t.Fatalf("ifMatch = %q, want etag-a", o.ifMatch)
	}
	if o.ifNoneMatch != "etag-b" {
		t.Fatalf("ifNoneMatch = %q, want etag-b", o.ifNoneMatch)
	}
	if o.ifModifiedSince == nil || *o.ifModifiedSince != modified.UnixMilli() {
		t.Fatalf("ifModifiedSince = %v, want %d", o.ifModifiedSince, modified.UnixMilli())
	}
	if o.ifUnmodifiedSince == nil || *o.ifUnmodifiedSince != unmodified.UnixMilli() {
		t.Fatalf("ifUnmodifiedSince = %v, want %d", o.ifUnmodifiedSince, unmodified.UnixMilli())
	}
	if o.concurrent != 4 {
		t.Fatalf("concurrent = %d, want 4", o.concurrent)
	}
	if o.chunk != 1024 {
		t.Fatalf("chunk = %d, want 1024", o.chunk)
	}
	if o.gap != 512 {
		t.Fatalf("gap = %d, want 512", o.gap)
	}
	if o.contentLengthHint == nil || *o.contentLengthHint != 4096 {
		t.Fatalf("contentLengthHint = %v, want 4096", o.contentLengthHint)
	}
	if o.overrideContentType != "text/plain" {
		t.Fatalf("overrideContentType = %q, want text/plain", o.overrideContentType)
	}
	if o.overrideCacheControl != "max-age=60" {
		t.Fatalf("overrideCacheControl = %q, want max-age=60", o.overrideCacheControl)
	}
	if o.overrideContentDisposition != "attachment" {
		t.Fatalf("overrideContentDisposition = %q, want attachment", o.overrideContentDisposition)
	}
}

func TestReadWithRangeFrom(t *testing.T) {
	o := &readOptions{}
	ReadWithRangeFrom(1024)(o)

	if !o.hasRange {
		t.Fatalf("hasRange = false, want true")
	}
	if o.hasRangeLength {
		t.Fatalf("hasRangeLength = true, want false")
	}
	if o.rangeOffset != 1024 {
		t.Fatalf("rangeOffset = %d, want 1024", o.rangeOffset)
	}

	// ReadWithRangeFrom overrides a previously set bounded range.
	o = &readOptions{}
	ReadWithRange(0, 2048)(o)
	ReadWithRangeFrom(512)(o)
	if o.hasRangeLength || o.rangeOffset != 512 || o.rangeLength != 0 {
		t.Fatalf("after override: hasRangeLength=%v offset=%d length=%d, want false 512 0",
			o.hasRangeLength, o.rangeOffset, o.rangeLength)
	}
}

func TestFfiOperatorReadWithReturnType(t *testing.T) {
	if ffiOperatorReadWith.opts.rType != &typeResultRead {
		t.Fatalf("ffiOperatorReadWith rType = %v, want typeResultRead", ffiOperatorReadWith.opts.rType)
	}
}

func TestFfiOperatorReadWithArgTypes(t *testing.T) {
	aTypes := ffiOperatorReadWith.opts.aTypes
	if len(aTypes) != 3 {
		t.Fatalf("ffiOperatorReadWith aTypes len = %d, want 3", len(aTypes))
	}
	for i, at := range aTypes {
		if at != &ffi.TypePointer {
			t.Fatalf("ffiOperatorReadWith aTypes[%d] = %v, want TypePointer", i, at)
		}
	}
}

func TestReadOptionsSetterArgTypes(t *testing.T) {
	stringSetters := []*FFI[func(*opendalReadOptions, string) ([]byte, error)]{
		ffiReadOptionsSetVersion,
		ffiReadOptionsSetIfMatch,
		ffiReadOptionsSetIfNoneMatch,
		ffiReadOptionsSetOverrideContentType,
		ffiReadOptionsSetOverrideCacheControl,
		ffiReadOptionsSetOverrideContentDisposition,
	}
	for _, setter := range stringSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		for i, at := range aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", setter.opts.sym, i, at)
			}
		}
	}

	int64Setters := []*FFI[func(*opendalReadOptions, int64)]{
		ffiReadOptionsSetIfModifiedSince,
		ffiReadOptionsSetIfUnmodifiedSince,
	}
	for _, setter := range int64Setters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		if aTypes[0] != &ffi.TypePointer || aTypes[1] != &ffi.TypeSint64 {
			t.Fatalf("%s aTypes = %v, want TypePointer, TypeSint64", setter.opts.sym, aTypes)
		}
	}

	uintSetters := []*FFI[func(*opendalReadOptions, uint)]{
		ffiReadOptionsSetConcurrent,
		ffiReadOptionsSetChunk,
		ffiReadOptionsSetGap,
	}
	for _, setter := range uintSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		for i, at := range aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", setter.opts.sym, i, at)
			}
		}
	}

	hintATypes := ffiReadOptionsSetContentLengthHint.opts.aTypes
	if len(hintATypes) != 2 {
		t.Fatalf("ffiReadOptionsSetContentLengthHint aTypes len = %d, want 2", len(hintATypes))
	}
	if hintATypes[0] != &ffi.TypePointer || hintATypes[1] != &ffi.TypeUint64 {
		t.Fatalf("ffiReadOptionsSetContentLengthHint aTypes = %v, want TypePointer, TypeUint64", hintATypes)
	}

	rangeATypes := ffiReadOptionsSetRange.opts.aTypes
	if len(rangeATypes) != 3 {
		t.Fatalf("ffiReadOptionsSetRange aTypes len = %d, want 3", len(rangeATypes))
	}
	if rangeATypes[0] != &ffi.TypePointer || rangeATypes[1] != &ffi.TypeUint64 || rangeATypes[2] != &ffi.TypeUint64 {
		t.Fatalf("ffiReadOptionsSetRange aTypes = %v, want TypePointer, TypeUint64, TypeUint64", rangeATypes)
	}

	rangeFromATypes := ffiReadOptionsSetRangeFrom.opts.aTypes
	if len(rangeFromATypes) != 2 {
		t.Fatalf("ffiReadOptionsSetRangeFrom aTypes len = %d, want 2", len(rangeFromATypes))
	}
	if rangeFromATypes[0] != &ffi.TypePointer || rangeFromATypes[1] != &ffi.TypeUint64 {
		t.Fatalf("ffiReadOptionsSetRangeFrom aTypes = %v, want TypePointer, TypeUint64", rangeFromATypes)
	}
}

func TestReaderWithOptions(t *testing.T) {
	modified := time.Unix(1700000000, 0)
	unmodified := time.Unix(1700000123, 0)

	o := &readerOptions{}
	ReaderWithVersion("v1")(o)
	ReaderWithIfMatch("etag-a")(o)
	ReaderWithIfNoneMatch("etag-b")(o)
	ReaderWithIfModifiedSince(modified)(o)
	ReaderWithIfUnmodifiedSince(unmodified)(o)
	ReaderWithContentLengthHint(4096)(o)
	ReaderWithConcurrent(4)(o)
	ReaderWithChunk(1024)(o)
	ReaderWithGap(512)(o)
	ReaderWithPrefetch(8)(o)

	if o.version != "v1" {
		t.Fatalf("version = %q, want v1", o.version)
	}
	if o.ifMatch != "etag-a" {
		t.Fatalf("ifMatch = %q, want etag-a", o.ifMatch)
	}
	if o.ifNoneMatch != "etag-b" {
		t.Fatalf("ifNoneMatch = %q, want etag-b", o.ifNoneMatch)
	}
	if o.ifModifiedSince == nil || *o.ifModifiedSince != modified.UnixMilli() {
		t.Fatalf("ifModifiedSince = %v, want %d", o.ifModifiedSince, modified.UnixMilli())
	}
	if o.ifUnmodifiedSince == nil || *o.ifUnmodifiedSince != unmodified.UnixMilli() {
		t.Fatalf("ifUnmodifiedSince = %v, want %d", o.ifUnmodifiedSince, unmodified.UnixMilli())
	}
	if o.contentLengthHint == nil || *o.contentLengthHint != 4096 {
		t.Fatalf("contentLengthHint = %v, want 4096", o.contentLengthHint)
	}
	if o.concurrent != 4 {
		t.Fatalf("concurrent = %d, want 4", o.concurrent)
	}
	if o.chunk != 1024 {
		t.Fatalf("chunk = %d, want 1024", o.chunk)
	}
	if o.gap != 512 {
		t.Fatalf("gap = %d, want 512", o.gap)
	}
	if o.prefetch != 8 {
		t.Fatalf("prefetch = %d, want 8", o.prefetch)
	}
}

func TestFfiOperatorReaderWithReturnType(t *testing.T) {
	if ffiOperatorReaderWith.opts.rType != &typeResultOperatorReader {
		t.Fatalf("ffiOperatorReaderWith rType = %v, want typeResultOperatorReader", ffiOperatorReaderWith.opts.rType)
	}
}

func TestFfiOperatorReaderWithArgTypes(t *testing.T) {
	aTypes := ffiOperatorReaderWith.opts.aTypes
	if len(aTypes) != 3 {
		t.Fatalf("ffiOperatorReaderWith aTypes len = %d, want 3", len(aTypes))
	}
	for i, at := range aTypes {
		if at != &ffi.TypePointer {
			t.Fatalf("ffiOperatorReaderWith aTypes[%d] = %v, want TypePointer", i, at)
		}
	}
}

func TestReaderOptionsSetterArgTypes(t *testing.T) {
	stringSetters := []*FFI[func(*opendalReaderOptions, string) ([]byte, error)]{
		ffiReaderOptionsSetVersion,
		ffiReaderOptionsSetIfMatch,
		ffiReaderOptionsSetIfNoneMatch,
	}
	for _, setter := range stringSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		for i, at := range aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", setter.opts.sym, i, at)
			}
		}
	}

	int64Setters := []*FFI[func(*opendalReaderOptions, int64)]{
		ffiReaderOptionsSetIfModifiedSince,
		ffiReaderOptionsSetIfUnmodifiedSince,
	}
	for _, setter := range int64Setters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		if aTypes[0] != &ffi.TypePointer || aTypes[1] != &ffi.TypeSint64 {
			t.Fatalf("%s aTypes = %v, want TypePointer, TypeSint64", setter.opts.sym, aTypes)
		}
	}

	uintSetters := []*FFI[func(*opendalReaderOptions, uint)]{
		ffiReaderOptionsSetConcurrent,
		ffiReaderOptionsSetChunk,
		ffiReaderOptionsSetGap,
		ffiReaderOptionsSetPrefetch,
	}
	for _, setter := range uintSetters {
		aTypes := setter.opts.aTypes
		if len(aTypes) != 2 {
			t.Fatalf("%s aTypes len = %d, want 2", setter.opts.sym, len(aTypes))
		}
		for i, at := range aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", setter.opts.sym, i, at)
			}
		}
	}

	hintATypes := ffiReaderOptionsSetContentLengthHint.opts.aTypes
	if len(hintATypes) != 2 {
		t.Fatalf("ffiReaderOptionsSetContentLengthHint aTypes len = %d, want 2", len(hintATypes))
	}
	if hintATypes[0] != &ffi.TypePointer || hintATypes[1] != &ffi.TypeUint64 {
		t.Fatalf("ffiReaderOptionsSetContentLengthHint aTypes = %v, want TypePointer, TypeUint64", hintATypes)
	}
}

func TestFfiOperatorPresignWithSignatures(t *testing.T) {
	for _, tc := range []struct {
		name string
		opts ffiOpts
	}{
		{
			name: "ffiOperatorPresignReadWith",
			opts: ffiOperatorPresignReadWith.opts,
		},
		{
			name: "ffiOperatorPresignWriteWith",
			opts: ffiOperatorPresignWriteWith.opts,
		},
		{
			name: "ffiOperatorPresignDeleteWith",
			opts: ffiOperatorPresignDeleteWith.opts,
		},
		{
			name: "ffiOperatorPresignStatWith",
			opts: ffiOperatorPresignStatWith.opts,
		},
	} {
		if tc.opts.rType != &typeResultPresign {
			t.Fatalf("%s rType = %v, want typeResultPresign", tc.name, tc.opts.rType)
		}
		if len(tc.opts.aTypes) != 4 {
			t.Fatalf("%s aTypes len = %d, want 4", tc.name, len(tc.opts.aTypes))
		}
		if tc.opts.aTypes[0] != &ffi.TypePointer ||
			tc.opts.aTypes[1] != &ffi.TypePointer ||
			tc.opts.aTypes[2] != &ffi.TypeUint64 ||
			tc.opts.aTypes[3] != &ffi.TypePointer {
			t.Fatalf("%s aTypes = %v, want pointer, pointer, uint64, pointer", tc.name, tc.opts.aTypes)
		}
	}
}

func TestPresignWithoutOptionsUsesSimpleSymbols(t *testing.T) {
	ctx, reqInner, freeCount := presignTestContext(t, "GET", "https://example.com/simple")
	opInner := &opendalOperator{}
	op := &Operator{ctx: ctx, inner: opInner}

	ctx = context.WithValue(ctx, ffiOperatorPresignRead.opts.sym, func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		return reqInner, nil
	})
	ctx = context.WithValue(ctx, ffiOperatorPresignWrite.opts.sym, func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		return reqInner, nil
	})
	ctx = context.WithValue(ctx, ffiOperatorPresignDelete.opts.sym, func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		return reqInner, nil
	})
	ctx = context.WithValue(ctx, ffiOperatorPresignStat.opts.sym, func(op *opendalOperator, path string, expire uint64) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		return reqInner, nil
	})
	op.ctx = ctx

	for name, call := range map[string]func() (*http.Request, error){
		"read":   func() (*http.Request, error) { return op.PresignRead("file.txt", time.Minute) },
		"write":  func() (*http.Request, error) { return op.PresignWrite("file.txt", time.Minute) },
		"delete": func() (*http.Request, error) { return op.PresignDelete("file.txt", time.Minute) },
		"stat":   func() (*http.Request, error) { return op.PresignStat("file.txt", time.Minute) },
	} {
		req, err := call()
		if err != nil {
			t.Fatalf("Presign%s() failed: %v", name, err)
		}
		if req.Method != "GET" || req.URL.String() != "https://example.com/simple" {
			t.Fatalf("Presign%s() request = %s %s", name, req.Method, req.URL.String())
		}
	}
	if *freeCount != 4 {
		t.Fatalf("presigned request freed %d times, want 4", *freeCount)
	}
}

func TestPresignReadWithOptionsKeepsStringsUntilCall(t *testing.T) {
	ctx, reqInner, _ := presignTestContext(t, "GET", "https://example.com/read")
	opInner := &opendalOperator{}
	cOpts := &opendalReadOptions{}
	var versionPtr *byte
	var overrideContentTypePtr *byte
	var rangeOffset, rangeLength uint64

	ctx = context.WithValue(ctx, ffiReadOptionsNew.opts.sym, func() *opendalReadOptions { return cOpts })
	ctx = context.WithValue(ctx, ffiReadOptionsFree.opts.sym, func(opts *opendalReadOptions) {
		assertReadOptionsPointer(t, cOpts, opts)
	})
	ctx = context.WithValue(ctx, ffiReadOptionsSetRange.opts.sym, func(opts *opendalReadOptions, offset, length uint64) {
		assertReadOptionsPointer(t, cOpts, opts)
		rangeOffset = offset
		rangeLength = length
	})
	ctx = context.WithValue(ctx, ffiReadOptionsSetVersion.opts.sym, ffiReadOptionsSetVersion.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertReadOptionsPointerFromArgs(t, cOpts, aValues...)
		versionPtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiReadOptionsSetIfMatch.opts.sym, noopReadOptionsSetString)
	ctx = context.WithValue(ctx, ffiReadOptionsSetIfNoneMatch.opts.sym, noopReadOptionsSetString)
	ctx = context.WithValue(ctx, ffiReadOptionsSetOverrideContentType.opts.sym, ffiReadOptionsSetOverrideContentType.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertReadOptionsPointerFromArgs(t, cOpts, aValues...)
		overrideContentTypePtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiReadOptionsSetOverrideCacheControl.opts.sym, noopReadOptionsSetString)
	ctx = context.WithValue(ctx, ffiReadOptionsSetOverrideContentDisposition.opts.sym, noopReadOptionsSetString)
	ctx = context.WithValue(ctx, ffiOperatorPresignReadWith.opts.sym, func(op *opendalOperator, path string, expire uint64, opts *opendalReadOptions) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		assertReadOptionsPointer(t, cOpts, opts)
		if rangeOffset != 1 || rangeLength != 2 {
			t.Fatalf("read range = %d, %d, want 1, 2", rangeOffset, rangeLength)
		}
		if got := BytePtrToString(versionPtr); got != "v1" {
			t.Fatalf("read version pointer = %q, want v1", got)
		}
		if got := BytePtrToString(overrideContentTypePtr); got != "text/plain" {
			t.Fatalf("read override content type pointer = %q, want text/plain", got)
		}
		return reqInner, nil
	})

	op := &Operator{ctx: ctx, inner: opInner}
	_, err := op.PresignRead("file.txt", time.Minute,
		ReadWithRange(1, 2),
		ReadWithVersion("v1"),
		ReadWithOverrideContentType("text/plain"),
	)
	if err != nil {
		t.Fatalf("PresignRead with options failed: %v", err)
	}
}

func TestPresignWriteWithOptionsKeepsStringsUntilCall(t *testing.T) {
	ctx, reqInner, _ := presignTestContext(t, "PUT", "https://example.com/write")
	opInner := &opendalOperator{}
	cOpts := &opendalWriteOptions{}
	var contentTypePtr *byte
	var userMetadata []opendalWriteUserMetadataPair

	ctx = context.WithValue(ctx, ffiWriteOptionsNew.opts.sym, func() *opendalWriteOptions { return cOpts })
	ctx = context.WithValue(ctx, ffiWriteOptionsFree.opts.sym, func(opts *opendalWriteOptions) {
		assertWriteOptionsPointer(t, cOpts, opts)
	})
	ctx = context.WithValue(ctx, ffiWriteOptionsSetAppend.opts.sym, func(opts *opendalWriteOptions, append bool) {
		assertWriteOptionsPointer(t, cOpts, opts)
	})
	ctx = context.WithValue(ctx, ffiWriteOptionsSetIfNotExists.opts.sym, func(opts *opendalWriteOptions, ifNotExists bool) {
		assertWriteOptionsPointer(t, cOpts, opts)
	})
	ctx = context.WithValue(ctx, ffiWriteOptionsSetContentType.opts.sym, ffiWriteOptionsSetContentType.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertWriteOptionsPointerFromArgs(t, cOpts, aValues...)
		contentTypePtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiWriteOptionsSetCacheControl.opts.sym, noopWriteOptionsSetString)
	ctx = context.WithValue(ctx, ffiWriteOptionsSetContentDisposition.opts.sym, noopWriteOptionsSetString)
	ctx = context.WithValue(ctx, ffiWriteOptionsSetContentEncoding.opts.sym, noopWriteOptionsSetString)
	ctx = context.WithValue(ctx, ffiWriteOptionsSetIfMatch.opts.sym, noopWriteOptionsSetString)
	ctx = context.WithValue(ctx, ffiWriteOptionsSetIfNoneMatch.opts.sym, noopWriteOptionsSetString)
	ctx = context.WithValue(ctx, ffiWriteOptionsSetUserMetadata.opts.sym, func(opts *opendalWriteOptions, pairs []opendalWriteUserMetadataPair) {
		assertWriteOptionsPointer(t, cOpts, opts)
		userMetadata = pairs
	})
	ctx = context.WithValue(ctx, ffiOperatorPresignWriteWith.opts.sym, func(op *opendalOperator, path string, expire uint64, opts *opendalWriteOptions) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		assertWriteOptionsPointer(t, cOpts, opts)
		if got := BytePtrToString(contentTypePtr); got != "text/plain" {
			t.Fatalf("write content type pointer = %q, want text/plain", got)
		}
		if len(userMetadata) != 1 {
			t.Fatalf("write user metadata len = %d, want 1", len(userMetadata))
		}
		if got := BytePtrToString(userMetadata[0].key); got != "foo" {
			t.Fatalf("write user metadata key = %q, want foo", got)
		}
		if got := BytePtrToString(userMetadata[0].value); got != "bar" {
			t.Fatalf("write user metadata value = %q, want bar", got)
		}
		return reqInner, nil
	})

	op := &Operator{ctx: ctx, inner: opInner}
	_, err := op.PresignWrite("file.txt", time.Minute,
		WriteWithContentType("text/plain"),
		WriteWithUserMetadata(map[string]string{"foo": "bar"}),
	)
	if err != nil {
		t.Fatalf("PresignWrite with options failed: %v", err)
	}
}

func TestPresignStatWithOptionsKeepsStringsUntilCall(t *testing.T) {
	ctx, reqInner, _ := presignTestContext(t, "HEAD", "https://example.com/stat")
	opInner := &opendalOperator{}
	cOpts := &opendalStatOptions{}
	var versionPtr *byte
	var overrideContentTypePtr *byte

	ctx = context.WithValue(ctx, ffiStatOptionsNew.opts.sym, func() *opendalStatOptions { return cOpts })
	ctx = context.WithValue(ctx, ffiStatOptionsFree.opts.sym, func(opts *opendalStatOptions) {
		assertStatOptionsPointer(t, cOpts, opts)
	})
	ctx = context.WithValue(ctx, ffiStatOptionsSetVersion.opts.sym, ffiStatOptionsSetVersion.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertStatOptionsPointerFromArgs(t, cOpts, aValues...)
		versionPtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiStatOptionsSetIfMatch.opts.sym, noopStatOptionsSetString)
	ctx = context.WithValue(ctx, ffiStatOptionsSetIfNoneMatch.opts.sym, noopStatOptionsSetString)
	ctx = context.WithValue(ctx, ffiStatOptionsSetOverrideContentType.opts.sym, ffiStatOptionsSetOverrideContentType.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertStatOptionsPointerFromArgs(t, cOpts, aValues...)
		overrideContentTypePtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiStatOptionsSetOverrideCacheControl.opts.sym, noopStatOptionsSetString)
	ctx = context.WithValue(ctx, ffiStatOptionsSetOverrideContentDisposition.opts.sym, noopStatOptionsSetString)
	ctx = context.WithValue(ctx, ffiOperatorPresignStatWith.opts.sym, func(op *opendalOperator, path string, expire uint64, opts *opendalStatOptions) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		assertStatOptionsPointer(t, cOpts, opts)
		if got := BytePtrToString(versionPtr); got != "v1" {
			t.Fatalf("stat version pointer = %q, want v1", got)
		}
		if got := BytePtrToString(overrideContentTypePtr); got != "text/plain" {
			t.Fatalf("stat override content type pointer = %q, want text/plain", got)
		}
		return reqInner, nil
	})

	op := &Operator{ctx: ctx, inner: opInner}
	_, err := op.PresignStat("file.txt", time.Minute,
		StatWithVersion("v1"),
		StatWithOverrideContentType("text/plain"),
	)
	if err != nil {
		t.Fatalf("PresignStat with options failed: %v", err)
	}
}

func TestPresignDeleteWithOptionsKeepsStringsUntilCall(t *testing.T) {
	ctx, reqInner, _ := presignTestContext(t, "DELETE", "https://example.com/delete")
	opInner := &opendalOperator{}
	cOpts := &opendalDeleteOptions{}
	var versionPtr *byte
	var recursive bool

	ctx = context.WithValue(ctx, ffiDeleteOptionsNew.opts.sym, func() *opendalDeleteOptions { return cOpts })
	ctx = context.WithValue(ctx, ffiDeleteOptionsFree.opts.sym, func(opts *opendalDeleteOptions) {
		assertDeleteOptionsPointer(t, cOpts, opts)
	})
	ctx = context.WithValue(ctx, ffiDeleteOptionsSetRecursive.opts.sym, func(opts *opendalDeleteOptions, value bool) {
		assertDeleteOptionsPointer(t, cOpts, opts)
		recursive = value
	})
	ctx = context.WithValue(ctx, ffiDeleteOptionsSetVersion.opts.sym, ffiDeleteOptionsSetVersion.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertDeleteOptionsPointerFromArgs(t, cOpts, aValues...)
		versionPtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiOperatorPresignDeleteWith.opts.sym, func(op *opendalOperator, path string, expire uint64, opts *opendalDeleteOptions) (*opendalPresignedRequest, error) {
		assertPresignCall(t, opInner, op, "file.txt", path, uint64(60), expire)
		assertDeleteOptionsPointer(t, cOpts, opts)
		if !recursive {
			t.Fatal("delete recursive = false, want true")
		}
		if got := BytePtrToString(versionPtr); got != "v1" {
			t.Fatalf("delete version pointer = %q, want v1", got)
		}
		return reqInner, nil
	})

	op := &Operator{ctx: ctx, inner: opInner}
	_, err := op.PresignDelete("file.txt", time.Minute,
		DeleteWithVersion("v1"),
		DeleteWithRecursive(true),
	)
	if err != nil {
		t.Fatalf("PresignDelete with options failed: %v", err)
	}
}

func presignTestContext(t *testing.T, method, uri string) (context.Context, *opendalPresignedRequest, *int) {
	t.Helper()

	methodData, err := byteSliceFromString(method)
	if err != nil {
		t.Fatalf("byteSliceFromString(%q) failed: %v", method, err)
	}
	uriData, err := byteSliceFromString(uri)
	if err != nil {
		t.Fatalf("byteSliceFromString(%q) failed: %v", uri, err)
	}
	reqInner := &opendalPresignedRequest{}
	freeCount := 0

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiPresignedRequestMethod.opts.sym, func(req *opendalPresignedRequest) *byte {
		assertPresignedRequestPointer(t, reqInner, req)
		return &methodData[0]
	})
	ctx = context.WithValue(ctx, ffiPresignedRequestUri.opts.sym, func(req *opendalPresignedRequest) *byte {
		assertPresignedRequestPointer(t, reqInner, req)
		return &uriData[0]
	})
	ctx = context.WithValue(ctx, ffiPresignedRequestHeaders.opts.sym, func(req *opendalPresignedRequest) *opendalHttpHeaderPair {
		assertPresignedRequestPointer(t, reqInner, req)
		return nil
	})
	ctx = context.WithValue(ctx, ffiPresignedRequestHeadersLen.opts.sym, func(req *opendalPresignedRequest) uintptr {
		assertPresignedRequestPointer(t, reqInner, req)
		return 0
	})
	ctx = context.WithValue(ctx, ffiPresignedRequestFree.opts.sym, func(req *opendalPresignedRequest) {
		assertPresignedRequestPointer(t, reqInner, req)
		freeCount++
	})
	return ctx, reqInner, &freeCount
}

func assertPresignCall(t *testing.T, wantOp *opendalOperator, gotOp *opendalOperator, wantPath, gotPath string, wantExpire, gotExpire uint64) {
	t.Helper()
	if gotOp != wantOp {
		t.Fatalf("presign op = %p, want %p", gotOp, wantOp)
	}
	if gotPath != wantPath {
		t.Fatalf("presign path = %q, want %q", gotPath, wantPath)
	}
	if gotExpire != wantExpire {
		t.Fatalf("presign expire = %d, want %d", gotExpire, wantExpire)
	}
}

func assertPresignedRequestPointer(t *testing.T, want *opendalPresignedRequest, got *opendalPresignedRequest) {
	t.Helper()
	if got != want {
		t.Fatalf("presigned request = %p, want %p", got, want)
	}
}

func assertReadOptionsPointer(t *testing.T, want *opendalReadOptions, got *opendalReadOptions) {
	t.Helper()
	if got != want {
		t.Fatalf("read options = %p, want %p", got, want)
	}
}

func assertReadOptionsPointerFromArgs(t *testing.T, want *opendalReadOptions, aValues ...unsafe.Pointer) {
	t.Helper()
	if len(aValues) != 2 {
		t.Fatalf("read option setter received %d arguments, want 2", len(aValues))
	}
	assertReadOptionsPointer(t, want, *(**opendalReadOptions)(aValues[0]))
}

func noopReadOptionsSetString(*opendalReadOptions, string) ([]byte, error) {
	return nil, nil
}

func assertWriteOptionsPointer(t *testing.T, want *opendalWriteOptions, got *opendalWriteOptions) {
	t.Helper()
	if got != want {
		t.Fatalf("write options = %p, want %p", got, want)
	}
}

func assertWriteOptionsPointerFromArgs(t *testing.T, want *opendalWriteOptions, aValues ...unsafe.Pointer) {
	t.Helper()
	if len(aValues) != 2 {
		t.Fatalf("write option setter received %d arguments, want 2", len(aValues))
	}
	assertWriteOptionsPointer(t, want, *(**opendalWriteOptions)(aValues[0]))
}

func noopWriteOptionsSetString(*opendalWriteOptions, string) ([]byte, error) {
	return nil, nil
}

func assertStatOptionsPointer(t *testing.T, want *opendalStatOptions, got *opendalStatOptions) {
	t.Helper()
	if got != want {
		t.Fatalf("stat options = %p, want %p", got, want)
	}
}

func assertStatOptionsPointerFromArgs(t *testing.T, want *opendalStatOptions, aValues ...unsafe.Pointer) {
	t.Helper()
	if len(aValues) != 2 {
		t.Fatalf("stat option setter received %d arguments, want 2", len(aValues))
	}
	assertStatOptionsPointer(t, want, *(**opendalStatOptions)(aValues[0]))
}

func noopStatOptionsSetString(*opendalStatOptions, string) ([]byte, error) {
	return nil, nil
}

func assertDeleteOptionsPointer(t *testing.T, want *opendalDeleteOptions, got *opendalDeleteOptions) {
	t.Helper()
	if got != want {
		t.Fatalf("delete options = %p, want %p", got, want)
	}
}

func assertDeleteOptionsPointerFromArgs(t *testing.T, want *opendalDeleteOptions, aValues ...unsafe.Pointer) {
	t.Helper()
	if len(aValues) != 2 {
		t.Fatalf("delete option setter received %d arguments, want 2", len(aValues))
	}
	assertDeleteOptionsPointer(t, want, *(**opendalDeleteOptions)(aValues[0]))
}

func TestFfiDeleterSignatures(t *testing.T) {
	// A mismatch here corrupts memory at runtime: nothing else checks the Go
	// descriptors against the C signatures.
	descriptors := []struct {
		name      string
		opts      ffiOpts
		rType     *ffi.Type
		aTypesLen int
	}{
		{name: "ffiOperatorDeleter", opts: ffiOperatorDeleter.opts, rType: &typeResultOperatorDeleter, aTypesLen: 1},
		{name: "ffiDeleterDelete", opts: ffiDeleterDelete.opts, rType: &ffi.TypePointer, aTypesLen: 2},
		{name: "ffiDeleterDeleteMany", opts: ffiDeleterDeleteMany.opts, rType: &ffi.TypePointer, aTypesLen: 3},
		{name: "ffiDeleterDeleteWith", opts: ffiDeleterDeleteWith.opts, rType: &ffi.TypePointer, aTypesLen: 3},
		{name: "ffiDeleterFlush", opts: ffiDeleterFlush.opts, rType: &ffi.TypePointer, aTypesLen: 1},
		{name: "ffiDeleterFree", opts: ffiDeleterFree.opts, rType: &ffi.TypeVoid, aTypesLen: 1},
	}
	for _, tc := range descriptors {
		if tc.opts.rType != tc.rType {
			t.Fatalf("%s rType = %v, want %v", tc.name, tc.opts.rType, tc.rType)
		}
		if len(tc.opts.aTypes) != tc.aTypesLen {
			t.Fatalf("%s aTypes len = %d, want %d", tc.name, len(tc.opts.aTypes), tc.aTypesLen)
		}
		for i, at := range tc.opts.aTypes {
			if at != &ffi.TypePointer {
				t.Fatalf("%s aTypes[%d] = %v, want TypePointer", tc.name, i, at)
			}
		}
	}
}

func TestFfiDeleterDeleteManyMarshalsPaths(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	want := []string{"a.txt", "b.txt", "nested/c.txt"}
	calls := 0

	deleteMany := ffiDeleterDeleteMany.withFunc(context.Background(), func(rValue unsafe.Pointer, aValues ...unsafe.Pointer) {
		calls++
		if len(aValues) != 3 {
			t.Fatalf("delete many received %d arguments, want 3", len(aValues))
		}
		if got := *(**opendalDeleter)(aValues[0]); got != deleterInner {
			t.Fatalf("deleter = %p, want %p", got, deleterInner)
		}

		pathsLen := *(*uint)(aValues[2])
		if pathsLen != uint(len(want)) {
			t.Fatalf("paths len = %d, want %d", pathsLen, len(want))
		}
		pathsPtr := *(***byte)(aValues[1])
		paths := unsafe.Slice(pathsPtr, int(pathsLen))
		for i, path := range paths {
			if got := BytePtrToString(path); got != want[i] {
				t.Fatalf("path[%d] = %q, want %q", i, got, want[i])
			}
		}
		*(**opendalError)(rValue) = nil
	})

	if err := deleteMany(deleterInner, want); err != nil {
		t.Fatalf("delete many failed: %v", err)
	}
	// One crossing for the whole slice is the point of this entry point.
	if calls != 1 {
		t.Fatalf("delete many called FFI %d times, want 1", calls)
	}
}

func TestFfiDeleterDeleteManyRejectsNulBeforeCall(t *testing.T) {
	called := false
	deleteMany := ffiDeleterDeleteMany.withFunc(context.Background(), func(unsafe.Pointer, ...unsafe.Pointer) {
		called = true
	})

	err := deleteMany(newTestDeleterHandle(), []string{"a.txt", "bad\x00path"})
	if err == nil {
		t.Fatal("delete many with a nul path succeeded")
	}
	if called {
		t.Fatal("delete many called FFI after path validation failed")
	}
}

func TestDeleterDeleteWithOptionsKeepsStringsUntilCall(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	cOpts := &opendalDeleteOptions{}
	var versionPtr *byte
	var recursive bool
	optionsFreed := 0

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiDeleteOptionsNew.opts.sym, func() *opendalDeleteOptions { return cOpts })
	ctx = context.WithValue(ctx, ffiDeleteOptionsFree.opts.sym, func(opts *opendalDeleteOptions) {
		assertDeleteOptionsPointer(t, cOpts, opts)
		optionsFreed++
	})
	ctx = context.WithValue(ctx, ffiDeleteOptionsSetRecursive.opts.sym, func(opts *opendalDeleteOptions, value bool) {
		assertDeleteOptionsPointer(t, cOpts, opts)
		recursive = value
	})
	ctx = context.WithValue(ctx, ffiDeleteOptionsSetVersion.opts.sym, ffiDeleteOptionsSetVersion.withFunc(ctx, func(_ unsafe.Pointer, aValues ...unsafe.Pointer) {
		assertDeleteOptionsPointerFromArgs(t, cOpts, aValues...)
		versionPtr = *(**byte)(aValues[1])
	}))
	ctx = context.WithValue(ctx, ffiDeleterDeleteWith.opts.sym, func(d *opendalDeleter, path string, opts *opendalDeleteOptions) error {
		if d != deleterInner {
			t.Fatalf("deleter = %p, want %p", d, deleterInner)
		}
		if path != "file.txt" {
			t.Fatalf("path = %q, want file.txt", path)
		}
		assertDeleteOptionsPointer(t, cOpts, opts)
		if !recursive {
			t.Fatal("delete recursive = false, want true")
		}
		if got := BytePtrToString(versionPtr); got != "v1" {
			t.Fatalf("delete version pointer = %q, want v1", got)
		}
		return nil
	})

	deleter := &Deleter{inner: deleterInner, ctx: ctx}
	if err := deleter.Delete("file.txt", DeleteWithVersion("v1"), DeleteWithRecursive(true)); err != nil {
		t.Fatalf("Delete with options failed: %v", err)
	}
	if optionsFreed != 1 {
		t.Fatalf("delete options freed %d times, want 1", optionsFreed)
	}
}

func TestDeleterCloseFlushesFreesAndBlocksReuse(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, nil)

	deleter := &Deleter{inner: deleterInner, ctx: ctx}
	if err := deleter.Close(); err != nil {
		t.Fatalf("Close failed: %v", err)
	}
	if err := deleter.Close(); err != nil {
		t.Fatalf("second Close failed: %v", err)
	}
	if calls.flushes != 1 {
		t.Fatalf("deleter flushed %d times, want 1", calls.flushes)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times, want 1", calls.freed)
	}
	if err := deleter.Delete("file.txt"); !errors.Is(err, ErrDeleterClosed) {
		t.Fatalf("Delete after Close = %v, want ErrDeleterClosed", err)
	}
}

func TestDeleterCloseReleasesAndReportsFailedFlush(t *testing.T) {
	// Close releases unconditionally, like every other Close in this package, so
	// a deferred Close cannot leak the handle. Retrying is Flush's job.
	deleterInner := newTestDeleterHandle()
	flushErr := errors.New("flush failed")
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, func() error { return flushErr })

	deleter := &Deleter{inner: deleterInner, ctx: ctx}
	if err := deleter.Close(); !errors.Is(err, flushErr) {
		t.Fatalf("Close = %v, want flush error", err)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times after failed flush, want 1", calls.freed)
	}
	if err := deleter.Close(); err != nil {
		t.Fatalf("second Close = %v, want nil", err)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times after the second Close, want 1", calls.freed)
	}
}

func TestDeleterFlushRetriesThenCloseReleases(t *testing.T) {
	// Flush keeps the deleter alive so a failed flush can be retried; only the
	// Close that follows releases it.
	deleterInner := newTestDeleterHandle()
	flushErr := errors.New("flush failed")
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, func() error {
		if calls.flushes == 1 {
			return flushErr
		}
		return nil
	})

	deleter := &Deleter{inner: deleterInner, ctx: ctx}
	if err := deleter.Flush(); !errors.Is(err, flushErr) {
		t.Fatalf("Flush = %v, want flush error", err)
	}
	if calls.freed != 0 {
		t.Fatalf("Flush freed the deleter %d times, want 0", calls.freed)
	}
	if err := deleter.Flush(); err != nil {
		t.Fatalf("Flush retry failed: %v", err)
	}
	if err := deleter.Close(); err != nil {
		t.Fatalf("Close failed: %v", err)
	}
	if calls.flushes != 3 {
		t.Fatalf("flushed %d times, want 3", calls.flushes)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times, want 1", calls.freed)
	}
}

func TestDeleterDiscardReleasesWithoutFlushing(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, nil)

	deleter := &Deleter{inner: deleterInner, ctx: ctx}
	deleter.Discard()
	deleter.Discard()
	if calls.flushes != 0 {
		t.Fatalf("Discard flushed %d times, want 0", calls.flushes)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times, want 1", calls.freed)
	}
	if err := deleter.Delete("file.txt"); !errors.Is(err, ErrDeleterClosed) {
		t.Fatalf("Delete after Discard = %v, want ErrDeleterClosed", err)
	}
}

func TestDeleterFlushKeepsDeleterUsable(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, nil)

	deleter := &Deleter{inner: deleterInner, ctx: ctx}
	if err := deleter.Flush(); err != nil {
		t.Fatalf("Flush failed: %v", err)
	}
	if calls.freed != 0 {
		t.Fatalf("Flush freed the deleter %d times, want 0", calls.freed)
	}
	if err := deleter.Delete("file.txt"); err != nil {
		t.Fatalf("Delete after Flush failed: %v", err)
	}
	if err := deleter.Close(); err != nil {
		t.Fatalf("Close failed: %v", err)
	}
	if calls.flushes != 2 {
		t.Fatalf("flushed %d times, want 2", calls.flushes)
	}
	if err := deleter.Flush(); !errors.Is(err, ErrDeleterClosed) {
		t.Fatalf("Flush after Close = %v, want ErrDeleterClosed", err)
	}
}

func TestRemoveQueuesEveryPathInOneCrossingThenCloses(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, nil)

	op := &Operator{ctx: ctx, inner: &opendalOperator{}}
	if err := op.Remove([]string{"a.txt", "b.txt", "c.txt"}); err != nil {
		t.Fatalf("Remove failed: %v", err)
	}
	assertPaths(t, calls.deleted, []string{"a.txt", "b.txt", "c.txt"})
	// Remove exists to hand the whole slice over at once. Queueing per path would
	// still pass the assertion above, so pin the crossing count down too.
	if calls.bulk != 1 {
		t.Fatalf("Remove made %d bulk calls, want 1", calls.bulk)
	}
	if calls.perPath != 0 {
		t.Fatalf("Remove queued %d paths one at a time, want 0", calls.perPath)
	}
	if calls.flushes != 1 || calls.freed != 1 {
		t.Fatalf("deleter flushed %d times and freed %d times, want 1 and 1", calls.flushes, calls.freed)
	}
}

func TestRemoveDiscardsQueueWhenQueueingFails(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	var calls deleterCalls
	deleteErr := errors.New("delete failed")

	ctx := newDeleterTestContext(t, deleterInner, &calls, func([]string) error { return deleteErr }, nil)

	op := &Operator{ctx: ctx, inner: &opendalOperator{}}
	if err := op.Remove([]string{"a.txt", "b.txt", "c.txt"}); !errors.Is(err, deleteErr) {
		t.Fatalf("Remove = %v, want delete error", err)
	}
	// Flushing here would delete whatever was queued before the failure while
	// reporting that Remove failed.
	if calls.flushes != 0 {
		t.Fatalf("Remove flushed %d times on its error path, want 0", calls.flushes)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times, want 1", calls.freed)
	}
}

func assertPaths(t *testing.T, got, want []string) {
	t.Helper()
	if !slices.Equal(got, want) {
		t.Fatalf("queued %v, want %v", got, want)
	}
}

func TestWithDeleterReportsAndReleasesOnFailedFlush(t *testing.T) {
	deleterInner := newTestDeleterHandle()
	flushErr := errors.New("flush failed")
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, func() error { return flushErr })

	op := &Operator{ctx: ctx, inner: &opendalOperator{}}
	err := op.WithDeleter(func(d *Deleter) error { return d.Delete("a.txt") })
	if !errors.Is(err, flushErr) {
		t.Fatalf("WithDeleter = %v, want flush error", err)
	}
	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times, want 1", calls.freed)
	}
}

func TestWithDeleterReleasesWhenFnPanics(t *testing.T) {
	// ffiDeleterDelete.symbol is an unchecked type assertion, so a panic out of fn
	// is reachable from the binding itself, not only from caller bugs.
	deleterInner := newTestDeleterHandle()
	var calls deleterCalls

	ctx := newDeleterTestContext(t, deleterInner, &calls, nil, nil)

	op := &Operator{ctx: ctx, inner: &opendalOperator{}}
	func() {
		defer func() {
			if recover() == nil {
				t.Fatal("WithDeleter swallowed the panic")
			}
		}()
		_ = op.WithDeleter(func(*Deleter) error { panic("boom") })
	}()

	if calls.freed != 1 {
		t.Fatalf("deleter freed %d times after a panic, want 1", calls.freed)
	}
	if calls.flushes != 0 {
		t.Fatalf("flushed %d times after a panic, want 0", calls.flushes)
	}
}

func TestRemoveWithoutPathsSkipsDeleter(t *testing.T) {
	ctx := context.WithValue(context.Background(), ffiOperatorDeleter.opts.sym, func(*opendalOperator) (*opendalDeleter, error) {
		t.Fatal("Remove(nil) created a deleter")
		return nil, nil
	})

	op := &Operator{ctx: ctx, inner: &opendalOperator{}}
	if err := op.Remove(nil); err != nil {
		t.Fatalf("Remove(nil) = %v, want nil", err)
	}
}

// newTestDeleterHandle returns a distinct stand-in for a C deleter handle.
//
// opendalDeleter is zero-sized, so every &opendalDeleter{} shares runtime.zerobase
// and the "is this the handle I was given?" assertions below would hold no matter
// which handle the code passed. Backing each handle with a byte gives it its own
// address and makes those assertions real.
func newTestDeleterHandle() *opendalDeleter {
	return (*opendalDeleter)(unsafe.Pointer(new(byte)))
}

// deleterCalls records what the stubbed deleter FFI saw. Separating bulk from
// perPath is what lets a test tell "queued the slice at once" apart from "queued
// the same paths one crossing at a time".
type deleterCalls struct {
	deleted []string
	bulk    int
	perPath int
	flushes int
	freed   int
}

func newDeleterTestContext(t *testing.T, inner *opendalDeleter, calls *deleterCalls, deleteFn func([]string) error, flushFn func() error) context.Context {
	t.Helper()

	ctx := context.Background()
	ctx = context.WithValue(ctx, ffiOperatorDeleter.opts.sym, func(op *opendalOperator) (*opendalDeleter, error) {
		if op == nil {
			t.Fatal("Deleter() passed nil operator")
		}
		return inner, nil
	})
	ctx = context.WithValue(ctx, ffiDeleterDelete.opts.sym, func(d *opendalDeleter, path string) error {
		if d != inner {
			t.Fatalf("deleter = %p, want %p", d, inner)
		}
		calls.perPath++
		calls.deleted = append(calls.deleted, path)
		if deleteFn != nil {
			return deleteFn([]string{path})
		}
		return nil
	})
	ctx = context.WithValue(ctx, ffiDeleterDeleteMany.opts.sym, func(d *opendalDeleter, paths []string) error {
		if d != inner {
			t.Fatalf("deleter = %p, want %p", d, inner)
		}
		calls.bulk++
		calls.deleted = append(calls.deleted, paths...)
		if deleteFn != nil {
			return deleteFn(paths)
		}
		return nil
	})
	ctx = context.WithValue(ctx, ffiDeleterFlush.opts.sym, func(d *opendalDeleter) error {
		if d != inner {
			t.Fatalf("flushed deleter = %p, want %p", d, inner)
		}
		// calls.flushes is incremented before flushFn runs, so a flushFn can key
		// its behavior off the attempt number.
		calls.flushes++
		if flushFn != nil {
			return flushFn()
		}
		return nil
	})
	ctx = context.WithValue(ctx, ffiDeleterFree.opts.sym, func(d *opendalDeleter) {
		if d != inner {
			t.Fatalf("freed deleter = %p, want %p", d, inner)
		}
		calls.freed++
	})
	return ctx
}
