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
	"unsafe"

	"github.com/jupiterrider/ffi"
)

var (
	typeResultOperatorNew = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultRead = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&typeBytes,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeBytes = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultStat = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultWrite = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultList = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultListerNext = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultOperatorReader = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultOperatorWriter = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultWriterWrite = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultOperatorCopier = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultCopierNext = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer, // size (usize)
			&ffi.TypeUint8,   // has_next (bool)
			&ffi.TypePointer, // error
			nil,
		}[0],
	}

	typeResultReaderRead = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultReaderSeek = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultIsExist = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypeUint8,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeResultPresign = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypePointer,
			&ffi.TypePointer,
			nil,
		}[0],
	}

	typeCapability = ffi.Type{
		Type: ffi.Struct,
		Elements: &[]*ffi.Type{
			&ffi.TypeUint8,   // stat
			&ffi.TypeUint8,   // stat_with_if_match
			&ffi.TypeUint8,   // stat_with_if_none_match
			&ffi.TypeUint8,   // stat_with_if_modified_since
			&ffi.TypeUint8,   // stat_with_if_unmodified_since
			&ffi.TypeUint8,   // stat_with_override_cache_control
			&ffi.TypeUint8,   // stat_with_override_content_disposition
			&ffi.TypeUint8,   // stat_with_override_content_type
			&ffi.TypeUint8,   // stat_with_version
			&ffi.TypeUint8,   // read
			&ffi.TypeUint8,   // read_with_if_match
			&ffi.TypeUint8,   // read_with_if_match_none
			&ffi.TypeUint8,   // read_with_override_cache_control
			&ffi.TypeUint8,   // read_with_override_content_disposition
			&ffi.TypeUint8,   // read_with_override_content_type
			&ffi.TypeUint8,   // read_with_if_modified_since
			&ffi.TypeUint8,   // read_with_if_unmodified_since
			&ffi.TypeUint8,   // read_with_version
			&ffi.TypeUint8,   // write
			&ffi.TypeUint8,   // write_can_multi
			&ffi.TypeUint8,   // write_can_empty
			&ffi.TypeUint8,   // write_can_append
			&ffi.TypeUint8,   // write_with_content_type
			&ffi.TypeUint8,   // write_with_content_disposition
			&ffi.TypeUint8,   // write_with_content_encoding
			&ffi.TypeUint8,   // write_with_cache_control
			&ffi.TypeUint8,   // write_with_if_match
			&ffi.TypeUint8,   // write_with_if_none_match
			&ffi.TypeUint8,   // write_with_if_not_exists
			&ffi.TypeUint8,   // write_with_user_metadata
			&ffi.TypePointer, // write_multi_max_size
			&ffi.TypePointer, // write_multi_min_size
			&ffi.TypePointer, // write_total_max_size
			&ffi.TypeUint8,   // create_dir
			&ffi.TypeUint8,   // delete
			&ffi.TypeUint8,   // delete_with_version
			&ffi.TypeUint8,   // delete_with_recursive
			&ffi.TypeUint8,   // copy
			&ffi.TypeUint8,   // copy_with_if_not_exists
			&ffi.TypeUint8,   // copy_with_if_match
			&ffi.TypeUint8,   // copy_with_source_version
			&ffi.TypeUint8,   // copy_can_multi
			&ffi.TypePointer, // copy_multi_max_size
			&ffi.TypePointer, // copy_multi_min_size
			&ffi.TypeUint8,   // rename
			&ffi.TypeUint8,   // list
			&ffi.TypeUint8,   // list_with_limit
			&ffi.TypeUint8,   // list_with_start_after
			&ffi.TypeUint8,   // list_with_recursive
			&ffi.TypeUint8,   // list_with_versions
			&ffi.TypeUint8,   // list_with_deleted
			&ffi.TypeUint8,   // presign
			&ffi.TypeUint8,   // presign_read
			&ffi.TypeUint8,   // presign_stat
			&ffi.TypeUint8,   // presign_write
			&ffi.TypeUint8,   // presign_delete
			&ffi.TypeUint8,   // shared
			nil,
		}[0],
	}
)

type opendalCapability struct {
	stat                               uint8
	statWithIfMatch                    uint8
	statWithIfNoneMatch                uint8
	statWithIfModifiedSince            uint8
	statWithIfUnmodifiedSince          uint8
	statWithOverrideCacheControl       uint8
	statWithOverrideContentDisposition uint8
	statWithOverrideContentType        uint8
	statWithVersion                    uint8
	read                               uint8
	readWithIfMatch                    uint8
	readWithIfNoneMatch                uint8
	readWithOverrideCacheControl       uint8
	readWithOverrideContentDisposition uint8
	readWithOverrideContentType        uint8
	readWithIfModifiedSince            uint8
	readWithIfUnmodifiedSince          uint8
	readWithVersion                    uint8
	write                              uint8
	writeCanMulti                      uint8
	writeCanEmpty                      uint8
	writeCanAppend                     uint8
	writeWithContentType               uint8
	writeWithContentDisposition        uint8
	writeWithContentEncoding           uint8
	writeWithCacheControl              uint8
	writeWithIfMatch                   uint8
	writeWithIfNoneMatch               uint8
	writeWithIfNotExists               uint8
	writeWithUserMetadata              uint8
	writeMultiMaxSize                  uint
	writeMultiMinSize                  uint
	writeTotalMaxSize                  uint
	createDir                          uint8
	delete                             uint8
	deleteWithVersion                  uint8
	deleteWithRecursive                uint8
	copy                               uint8
	copyWithIfNotExists                uint8
	copyWithIfMatch                    uint8
	copyWithSourceVersion              uint8
	copyCanMulti                       uint8
	copyMultiMaxSize                   uint
	copyMultiMinSize                   uint
	rename                             uint8
	list                               uint8
	listWithLimit                      uint8
	listWithStartAfter                 uint8
	listWithRecursive                  uint8
	listWithVersions                   uint8
	listWithDeleted                    uint8
	presign                            uint8
	presignRead                        uint8
	presignStat                        uint8
	presignWrite                       uint8
	presignDelete                      uint8
	shared                             uint8
}

type resultOperatorNew struct {
	op    *opendalOperator
	error *opendalError
}

type opendalOperator struct{}

type resultRead struct {
	data  opendalBytes
	error *opendalError
}

type opendalReader struct{}

type resultOperatorReader struct {
	reader *opendalReader
	error  *opendalError
}

type opendalWriter struct{}

type resultOperatorWriter struct {
	writer *opendalWriter
	error  *opendalError
}

type resultWriterWrite struct {
	size  uint
	error *opendalError
}

type opendalCopier struct{}

type resultOperatorCopier struct {
	copier *opendalCopier
	error  *opendalError
}

type resultCopierNext struct {
	size     uint
	has_next uint8
	error    *opendalError
}

type resultReaderRead struct {
	size  uint
	error *opendalError
}

type resultReaderSeek struct {
	pos   uint64
	error *opendalError
}

type resultIsExist struct {
	is_exist uint8
	error    *opendalError
}

type resultStat struct {
	meta  *opendalMetadata
	error *opendalError
}

type resultWrite struct {
	meta  *opendalMetadata
	error *opendalError
}

type resultPresign struct {
	req   *opendalPresignedRequest
	error *opendalError
}

type opendalPresignedRequest struct{}

type opendalHttpHeaderPair struct {
	key   *byte
	value *byte
}

type opendalMetadata struct{}

type opendalMetadataUserMetadata struct{}

type opendalMetadataUserMetadataPair struct {
	key   *byte
	value *byte
}

type opendalBytes struct {
	data     *byte
	len      uintptr
	capacity uintptr
}

type opendalError struct {
	code    int32
	message opendalBytes
}

type opendalOperatorInfo struct{}

type opendalResultList struct {
	lister *opendalLister
	err    *opendalError
}

type opendalLister struct{}

type opendalListOptions struct{}

type opendalDeleteOptions struct{}

type opendalReadOptions struct{}

type opendalReaderOptions struct{}

type opendalCopyOptions struct{}

type opendalWriteOptions struct{}

type opendalStatOptions struct{}

type opendalWriteUserMetadataPair struct {
	key   *byte
	value *byte
}

type opendalResultListerNext struct {
	entry *opendalEntry
	err   *opendalError
}

type opendalEntry struct{}

func toOpendalBytes(data []byte) *opendalBytes {
	l := len(data)
	if l == 0 {
		return &opendalBytes{}
	}

	return &opendalBytes{
		data:     &data[0],
		len:      uintptr(l),
		capacity: uintptr(cap(data)),
	}
}

func parseBytes(b opendalBytes) (data []byte) {
	if b.len == 0 {
		return nil
	}
	data = make([]byte, b.len)
	copy(data, unsafe.Slice(b.data, b.len))
	return
}
