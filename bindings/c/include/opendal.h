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

#ifndef _OPENDAL_H
#define _OPENDAL_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#define OPENDAL_SEEK_SET 0

#define OPENDAL_SEEK_CUR 1

#define OPENDAL_SEEK_END 2

/**
 * \brief The error code for all opendal APIs in C binding.
 * \todo The error handling is not complete, the error with error message will be
 * added in the future.
 */
typedef enum opendal_code {
  /**
   * returning it back. For example, s3 returns an internal service error.
   */
  OPENDAL_UNEXPECTED,
  /**
   * Underlying service doesn't support this operation.
   */
  OPENDAL_UNSUPPORTED,
  /**
   * The config for backend is invalid.
   */
  OPENDAL_CONFIG_INVALID,
  /**
   * The given path is not found.
   */
  OPENDAL_NOT_FOUND,
  /**
   * The given path doesn't have enough permission for this operation
   */
  OPENDAL_PERMISSION_DENIED,
  /**
   * The given path is a directory.
   */
  OPENDAL_IS_A_DIRECTORY,
  /**
   * The given path is not a directory.
   */
  OPENDAL_NOT_A_DIRECTORY,
  /**
   * The given path already exists thus we failed to the specified operation on it.
   */
  OPENDAL_ALREADY_EXISTS,
  /**
   * Requests that sent to this path is over the limit, please slow down.
   */
  OPENDAL_RATE_LIMITED,
  /**
   * The given file paths are same.
   */
  OPENDAL_IS_SAME_FILE,
  /**
   * A condition supplied through the OpenDAL operation evaluated to false.
   */
  OPENDAL_CONDITION_NOT_MATCH,
  /**
   * The range of the content is not satisfied.
   */
  OPENDAL_RANGE_NOT_SATISFIED,
  /**
   * The operation conflicts with the current or transitional state of the resource.
   */
  OPENDAL_CONFLICT,
} opendal_code;

typedef struct opendal_presigned_request_inner opendal_presigned_request_inner;

/**
 * \brief opendal_bytes carries raw-bytes with its length
 *
 * The opendal_bytes type is a C-compatible substitute for Vec type in Rust.
 * For buffers returned by OpenDAL C APIs, call opendal_bytes_free() to free
 * the heap memory and avoid memory leaks. For caller-owned input buffers
 * passed to OpenDAL C APIs, the caller keeps ownership and must not call
 * opendal_bytes_free() on them.
 *
 * @see opendal_bytes_free
 */
typedef struct opendal_bytes {
  /**
   * Pointing to the byte array on heap
   */
  uint8_t *data;
  /**
   * The length of the byte array
   */
  uintptr_t len;
  /**
   * The capacity of the byte array
   */
  uintptr_t capacity;
} opendal_bytes;

/**
 * \brief The opendal error type for C binding, containing an error code and corresponding error
 * message.
 *
 * The normal operations returns a pointer to the opendal_error, and the **nullptr normally
 * represents no error has taken placed**. If any error has taken place, the caller should check
 * the error code and print the error message.
 *
 * The error code is represented in opendal_code, which is an enum on different type of errors.
 * The error messages is represented in opendal_bytes, which is a non-null terminated byte array.
 *
 * \note 1. The error message is on heap, so the error needs to be freed by the caller, by calling
 *       opendal_error_free. 2. The error message is not null terminated, so the caller should
 *       never use "%s" to print the error message.
 *
 * @see opendal_code
 * @see opendal_bytes
 * @see opendal_error_free
 */
typedef struct opendal_error {
  enum opendal_code code;
  struct opendal_bytes message;
} opendal_error;

/**
 * \brief opendal_list_entry is the entry under a path, which is listed from the opendal_lister
 *
 * For examples, please see the comment section of opendal_operator_list()
 * @see opendal_operator_list()
 * @see opendal_list_entry_path()
 * @see opendal_list_entry_name()
 */
typedef struct opendal_entry {
  /**
   * The pointer to the opendal::Entry in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_entry;

/**
 * \brief The result type returned by opendal_lister_next().
 * The list entry is the list result of the list operation, the error field is the error code and error message.
 * If the operation succeeds, the error should be NULL.
 *
 * \note Please notice if the lister reaches the end, both the list_entry and error will be NULL.
 */
typedef struct opendal_result_lister_next {
  /**
   * The next object name
   */
  struct opendal_entry *entry;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_lister_next;

/**
 * \brief BlockingLister is designed to list entries at given path in a blocking
 * manner.
 *
 * Users can construct Lister by `blocking_list` or `blocking_scan`(currently not supported in C binding)
 *
 * For examples, please see the comment section of opendal_operator_list()
 * @see opendal_operator_list()
 */
typedef struct opendal_lister {
  /**
   * The pointer to the opendal::BlockingLister in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_lister;

/**
 * \brief The layers to apply when initializing an opendal_operator.
 *
 * \note This is also a heap-allocated struct, please free it after you use it.
 */
typedef struct opendal_operator_layers {
  /**
   * The pointer to the Vec<OperatorLayer> in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_operator_layers;

/**
 * \brief Carries all metadata associated with a **path**.
 *
 * The metadata of the "thing" under a path. Please **only** use the opendal_metadata
 * with our provided API, e.g. opendal_metadata_content_length().
 *
 * \note The metadata is also heap-allocated, please call opendal_metadata_free() on this
 * to free the heap memory.
 *
 * @see opendal_metadata_free
 */
typedef struct opendal_metadata {
  /**
   * The pointer to the opendal::Metadata in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_metadata;

/**
 * \brief User metadata associated with a **path**.
 */
typedef struct opendal_metadata_user_metadata {
  /**
   * The pointer to the user metadata in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_metadata_user_metadata;

/**
 * \brief A user metadata key-value pair.
 */
typedef struct opendal_metadata_user_metadata_pair {
  /**
   * The key of the user metadata.
   */
  const char *key;
  /**
   * The value of the user metadata.
   */
  const char *value;
} opendal_metadata_user_metadata_pair;

/**
 * \brief Used to access almost all OpenDAL APIs. It represents an
 * operator that provides the unified interfaces provided by OpenDAL.
 *
 * @see opendal_operator_new This function construct the operator
 * @see opendal_operator_free This function frees the heap memory of the operator
 *
 * \note The opendal_operator actually owns a pointer to
 * an opendal::blocking::Operator, which is inside the Rust core code.
 *
 * \remark You may use the field `ptr` to check whether this is a NULL
 * operator.
 */
typedef struct opendal_operator {
  /**
   * The pointer to the opendal::blocking::Operator in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_operator;

/**
 * \brief The result type returned by opendal_operator_new() operation.
 *
 * If the init logic is successful, the `op` field will be set to a valid
 * pointer, and the `error` field will be set to null. If the init logic fails, the
 * `op` field will be set to null, and the `error` field will be set to a
 * valid pointer with error code and error message.
 *
 * @see opendal_operator_new()
 * @see opendal_operator
 * @see opendal_error
 */
typedef struct opendal_result_operator_new {
  /**
   * The pointer for operator.
   */
  struct opendal_operator *op;
  /**
   * The error pointer for error.
   */
  struct opendal_error *error;
} opendal_result_operator_new;

/**
 * \brief The configuration for the initialization of opendal_operator.
 *
 * \note This is also a heap-allocated struct, please free it after you use it
 *
 * @see opendal_operator_new has an example of using opendal_operator_options
 * @see opendal_operator_options_new This function construct the operator
 * @see opendal_operator_options_free This function frees the heap memory of the operator
 * @see opendal_operator_options_set This function allow you to set the options
 */
typedef struct opendal_operator_options {
  /**
   * The pointer to the HashMap<String, String> in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_operator_options;

/**
 * \brief A key-value pair for write user metadata.
 */
typedef struct opendal_write_user_metadata_pair {
  /**
   * The metadata key.
   */
  const char *key;
  /**
   * The metadata value.
   */
  const char *value;
} opendal_write_user_metadata_pair;

/**
 * \brief The options for write operations.
 *
 * Use `opendal_write_options_new()` to construct and
 * `opendal_write_options_free()` to free.
 */
typedef struct opendal_write_options {
  /**
   * Append data to the existing file.
   */
  bool append;
  /**
   * Cache-Control header value.
   */
  const char *cache_control;
  /**
   * Content-Type header value.
   */
  const char *content_type;
  /**
   * Content-Disposition header value.
   */
  const char *content_disposition;
  /**
   * Content-Encoding header value.
   */
  const char *content_encoding;
  /**
   * If-Match header value.
   */
  const char *if_match;
  /**
   * If-None-Match header value.
   */
  const char *if_none_match;
  /**
   * Only write if target does not exist.
   */
  bool if_not_exists;
  /**
   * Concurrent write operations. `0` means sequential writes
   */
  uintptr_t concurrent;
  /**
   * Whether `chunk` has been set.
   */
  bool has_chunk;
  /**
   * Chunk size for buffered writes.
   */
  uintptr_t chunk;
  /**
   * User metadata pairs.
   */
  const struct opendal_write_user_metadata_pair *user_metadata;
  /**
   * User metadata pairs length.
   */
  uintptr_t user_metadata_len;
} opendal_write_options;

/**
 * \brief The result type returned by the metadata-returning write operations.
 *
 * Returned by opendal_operator_write_with_metadata() and
 * opendal_writer_close_with_metadata(). On success the `meta` field holds the
 * metadata of the just-written object (e.g. etag, version, last modified) and
 * `error` is null; on failure `meta` is null and `error` is set.
 */
typedef struct opendal_result_write {
  /**
   * The metadata of the written object, or null on error.
   */
  struct opendal_metadata *meta;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_write;

/**
 * \brief The result type returned by opendal's read operation.
 *
 * The result type of read operation in opendal C binding, it contains
 * the data that the read operation returns and an NULL error.
 * If the read operation failed, the `data` fields should be a nullptr
 * and the error is not NULL.
 */
typedef struct opendal_result_read {
  /**
   * The byte array with length returned by read operations
   */
  struct opendal_bytes data;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_read;

/**
 * \brief The options for read operations.
 *
 * Use `opendal_read_options_new()` to construct and
 * `opendal_read_options_free()` to free.
 */
typedef struct opendal_read_options {
  /**
   * The start offset of the range to read; default 0.
   */
  uint64_t offset;
  /**
   * Whether `length` has been set.
   */
  bool has_length;
  /**
   * The number of bytes to read starting from `offset`.
   */
  uint64_t length;
  /**
   * The version of the object to read; NULL means unset.
   */
  const char *version;
  /**
   * If-Match header value; NULL means unset.
   */
  const char *if_match;
  /**
   * If-None-Match header value; NULL means unset.
   */
  const char *if_none_match;
  /**
   * Whether `if_modified_since` has been set.
   */
  bool has_if_modified_since;
  /**
   * If-Modified-Since condition, in Unix milliseconds.
   */
  int64_t if_modified_since;
  /**
   * Whether `if_unmodified_since` has been set.
   */
  bool has_if_unmodified_since;
  /**
   * If-Unmodified-Since condition, in Unix milliseconds.
   */
  int64_t if_unmodified_since;
  /**
   * Concurrent read operations. `0` means sequential reads.
   */
  uintptr_t concurrent;
  /**
   * Whether `chunk` has been set.
   */
  bool has_chunk;
  /**
   * Chunk size for each read request.
   */
  uintptr_t chunk;
  /**
   * Whether `gap` has been set.
   */
  bool has_gap;
  /**
   * Gap size for merging nearby range reads.
   */
  uintptr_t gap;
  /**
   * Whether `content_length_hint` has been set.
   */
  bool has_content_length_hint;
  /**
   * Known content length of the object, used as an execution hint to avoid
   * extra metadata requests while planning reads.
   */
  uint64_t content_length_hint;
  /**
   * Override the response Content-Type header (presign only); NULL means unset.
   */
  const char *override_content_type;
  /**
   * Override the response Cache-Control header (presign only); NULL means unset.
   */
  const char *override_cache_control;
  /**
   * Override the response Content-Disposition header (presign only); NULL means unset.
   */
  const char *override_content_disposition;
} opendal_read_options;

/**
 * \brief The result type returned by opendal's reader operation.
 *
 * \note The opendal_reader actually owns a pointer to
 * a opendal::BlockingReader, which is inside the Rust core code.
 */
typedef struct opendal_reader {
  /**
   * The pointer to the opendal::StdReader in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_reader;

/**
 * \brief The result type returned by opendal_operator_reader().
 * The result type for opendal_operator_reader(), the field `reader` contains the reader
 * of the path, which is an iterator of the objects under the path. the field `code` represents
 * whether the stat operation is successful.
 */
typedef struct opendal_result_operator_reader {
  /**
   * The pointer for opendal_reader
   */
  struct opendal_reader *reader;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_operator_reader;

/**
 * \brief The options for creating a reader via `opendal_operator_reader_with`.
 *
 * Use `opendal_reader_options_new()` to construct and
 * `opendal_reader_options_free()` to free.
 */
typedef struct opendal_reader_options {
  /**
   * The version of the object to read; NULL means unset.
   */
  const char *version;
  /**
   * If-Match header value; NULL means unset.
   */
  const char *if_match;
  /**
   * If-None-Match header value; NULL means unset.
   */
  const char *if_none_match;
  /**
   * Whether `if_modified_since` has been set.
   */
  bool has_if_modified_since;
  /**
   * If-Modified-Since condition, in Unix milliseconds.
   */
  int64_t if_modified_since;
  /**
   * Whether `if_unmodified_since` has been set.
   */
  bool has_if_unmodified_since;
  /**
   * If-Unmodified-Since condition, in Unix milliseconds.
   */
  int64_t if_unmodified_since;
  /**
   * Whether `content_length_hint` has been set.
   */
  bool has_content_length_hint;
  /**
   * Known content length of the object, used as an execution hint to avoid
   * extra metadata requests while planning reads.
   */
  uint64_t content_length_hint;
  /**
   * Concurrent read operations. `0` falls back to sequential reads.
   */
  uintptr_t concurrent;
  /**
   * Whether `chunk` has been set.
   */
  bool has_chunk;
  /**
   * Chunk size for each read request.
   */
  uintptr_t chunk;
  /**
   * Whether `gap` has been set.
   */
  bool has_gap;
  /**
   * Gap size for merging nearby range reads.
   */
  uintptr_t gap;
  /**
   * Number of prefetched byte ranges buffered during concurrent reads.
   */
  uintptr_t prefetch;
} opendal_reader_options;

/**
 * \brief The result type returned by opendal's writer operation.
 * \note The opendal_writer actually owns a pointer to
 * an opendal::blocking::Writer, which is inside the Rust core code.
 */
typedef struct opendal_writer {
  /**
   * The pointer to the opendal::blocking::Writer in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_writer;

/**
 * \brief The result type returned by opendal_operator_writer().
 * The result type for opendal_operator_writer(), the field `writer` contains the writer
 * of the path, which is an iterator of the objects under the path. the field `code` represents
 */
typedef struct opendal_result_operator_writer {
  /**
   * The pointer for opendal_writer
   */
  struct opendal_writer *writer;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_operator_writer;

/**
 * \brief The options for the delete operation.
 *
 * This struct carries the options for the delete operation, including an optional
 * version string and whether to delete recursively.
 * Use `opendal_delete_options_new()` to construct and `opendal_delete_options_free()` to free.
 *
 * @see opendal_operator_delete_with
 * @see opendal_delete_options_new
 * @see opendal_delete_options_free
 * @see opendal_delete_options_set_version
 * @see opendal_delete_options_set_recursive
 */
typedef struct opendal_delete_options {
  /**
   * Optional version string to delete a specific version; NULL means unset.
   */
  char *version;
  /**
   * Whether to delete recursively; default false.
   */
  bool recursive;
} opendal_delete_options;

/**
 * \brief The result type returned by opendal_operator_is_exist().
 *
 * The result type for opendal_operator_is_exist(), the field `is_exist`
 * contains whether the path exists, and the field `error` contains the
 * corresponding error. If successful, the `error` field is null.
 *
 * \note If the opendal_operator_is_exist() fails, the `is_exist` field
 * will be set to false.
 */
typedef struct opendal_result_is_exist {
  /**
   * Whether the path exists
   */
  bool is_exist;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_is_exist;

/**
 * \brief The result type returned by opendal_operator_exists().
 *
 * The result type for opendal_operator_exists(), the field `exists`
 * contains whether the path exists, and the field `error` contains the
 * corresponding error. If successful, the `error` field is null.
 *
 * \note If the opendal_operator_exists() fails, the `exists` field
 * will be set to false.
 */
typedef struct opendal_result_exists {
  /**
   * Whether the path exists
   */
  bool exists;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_exists;

/**
 * \brief The result type returned by opendal_operator_stat().
 *
 * The result type for opendal_operator_stat(), the field `meta` contains the metadata
 * of the path, the field `error` represents whether the stat operation is successful.
 * If successful, the `error` field is null.
 */
typedef struct opendal_result_stat {
  /**
   * The metadata output of the stat
   */
  struct opendal_metadata *meta;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_stat;

/**
 * \brief The options for stat operations.
 *
 * Use `opendal_stat_options_new()` to construct and
 * `opendal_stat_options_free()` to free.
 *
 * @see opendal_operator_stat_with
 */
typedef struct opendal_stat_options {
  /**
   * The version of the object to stat; NULL means unset.
   */
  const char *version;
  /**
   * If-Match header value; NULL means unset.
   */
  const char *if_match;
  /**
   * If-None-Match header value; NULL means unset.
   */
  const char *if_none_match;
  /**
   * Whether `if_modified_since` has been set.
   */
  bool has_if_modified_since;
  /**
   * If-Modified-Since timestamp in milliseconds since the Unix epoch.
   */
  int64_t if_modified_since;
  /**
   * Whether `if_unmodified_since` has been set.
   */
  bool has_if_unmodified_since;
  /**
   * If-Unmodified-Since timestamp in milliseconds since the Unix epoch.
   */
  int64_t if_unmodified_since;
  /**
   * Override the response Content-Type header; NULL means unset.
   */
  const char *override_content_type;
  /**
   * Override the response Cache-Control header; NULL means unset.
   */
  const char *override_cache_control;
  /**
   * Override the response Content-Disposition header; NULL means unset.
   */
  const char *override_content_disposition;
} opendal_stat_options;

/**
 * \brief The result type returned by opendal_operator_list().
 *
 * The result type for opendal_operator_list(), the field `lister` contains the lister
 * of the path, which is an iterator of the objects under the path. the field `error` represents
 * whether the stat operation is successful. If successful, the `error` field is null.
 */
typedef struct opendal_result_list {
  /**
   * The lister, used for further listing operations
   */
  struct opendal_lister *lister;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_list;

/**
 * \brief The options for the list operation.
 *
 * This struct carries the options for the list operation, including whether to
 * list recursively, an optional result limit, and an optional start-after key.
 * Use `opendal_list_options_new()` to construct and `opendal_list_options_free()` to free.
 *
 * @see opendal_operator_list_with
 * @see opendal_list_options_new
 * @see opendal_list_options_free
 * @see opendal_list_options_set_recursive
 * @see opendal_list_options_set_limit
 * @see opendal_list_options_set_start_after
 * @see opendal_list_options_set_versions
 * @see opendal_list_options_set_deleted
 */
typedef struct opendal_list_options {
  /**
   * Whether to list recursively under the prefix; default false.
   */
  bool recursive;
  /**
   * Optional hint for maximum results per request; 0 means unset.
   */
  uintptr_t limit;
  /**
   * Optional key to start listing from; NULL means unset.
   */
  char *start_after;
  /**
   * Include object versions when supported by version-aware backends; default false.
   */
  bool versions;
  /**
   * Include delete markers when supported by version-aware backends; default false.
   */
  bool deleted;
} opendal_list_options;

/**
 * \brief The options for copy operations.
 *
 * Use `opendal_copy_options_new()` to construct and
 * `opendal_copy_options_free()` to free.
 */
typedef struct opendal_copy_options {
  /**
   * Only copy if target does not exist; default false.
   */
  bool if_not_exists;
  /**
   * If-Match condition; NULL means unset.
   */
  const char *if_match;
  /**
   * Source version; NULL means unset.
   */
  const char *source_version;
  /**
   * Whether `source_content_length_hint` has been set.
   */
  bool has_source_content_length_hint;
  /**
   * Known content length of the source object.
   */
  uint64_t source_content_length_hint;
  /**
   * Concurrent copy operations. `0` means sequential copy.
   */
  uintptr_t concurrent;
  /**
   * Whether `chunk` has been set.
   */
  bool has_chunk;
  /**
   * Chunk size for segmented copy operations.
   */
  uintptr_t chunk;
} opendal_copy_options;

/**
 * \brief opendal_copier completes a long-running copy operation to
 * completion in a blocking manner. opendal_copier repeatedly calls a
 * copy operation until completion.
 *
 * Internally, each copy step performs a unit of work and reports progress.
 * When copy completes, `has_next` returns false.
 *
 * A "step" is one backend-defined unit of work. For backends that copy in
 * multiple requests (e.g. multipart copy), one step typically copies one chunk;
 * for backends that only support single-request copy, the entire copy happens
 * in a single step. The reported byte count is best-effort: a step may report
 * 0 bytes when the backend advances its state without a reliable byte delta.
 *
 * Users can construct a copier by `opendal_operator_copier` or
 * `opendal_operator_copier_with`.
 *
 * @see opendal_operator_copier()
 * @see opendal_copier_next()
 */
typedef struct opendal_copier {
  /**
   * The pointer to the opendal::blocking::Copier in the Rust code.
   * Only used to check whether the copier is NULL.
   */
  void *inner;
} opendal_copier;

/**
 * \brief The result type returned by opendal_operator_copier().
 *
 * Fields:
 * * `copier`: the copier used to drive a long-running copy operation by calling
 *   opendal_copier_next() repeatedly; only valid when `error` is null.
 * * `error`: the error of the operation; null when the operation succeeds.
 */
typedef struct opendal_result_operator_copier {
  /**
   * The pointer for opendal_copier
   */
  struct opendal_copier *copier;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_operator_copier;

/**
 * \brief Metadata for **operator**, users can use this metadata to get information
 * of operator.
 */
typedef struct opendal_operator_info {
  /**
   * The pointer to the opendal::OperatorInfo in the Rust code.
   * Only touch this on judging whether it is NULL.
   */
  void *inner;
} opendal_operator_info;

/**
 * \brief Capability is used to describe what operations are supported
 * by current Operator.
 */
typedef struct opendal_capability {
  /**
   * If operator supports stat.
   */
  bool stat;
  /**
   * If operator supports stat with if match.
   */
  bool stat_with_if_match;
  /**
   * If operator supports stat with if none match.
   */
  bool stat_with_if_none_match;
  /**
   * If operator supports stat with if modified since.
   */
  bool stat_with_if_modified_since;
  /**
   * If operator supports stat with if unmodified since.
   */
  bool stat_with_if_unmodified_since;
  /**
   * if operator supports stat with override cache control.
   */
  bool stat_with_override_cache_control;
  /**
   * if operator supports stat with override content disposition.
   */
  bool stat_with_override_content_disposition;
  /**
   * if operator supports stat with override content type.
   */
  bool stat_with_override_content_type;
  /**
   * If operator supports stat with version.
   */
  bool stat_with_version;
  /**
   * If operator supports read.
   */
  bool read;
  /**
   * If operator supports read with if match.
   */
  bool read_with_if_match;
  /**
   * If operator supports read with if none match.
   */
  bool read_with_if_none_match;
  /**
   * if operator supports read with override cache control.
   */
  bool read_with_override_cache_control;
  /**
   * if operator supports read with override content disposition.
   */
  bool read_with_override_content_disposition;
  /**
   * if operator supports read with override content type.
   */
  bool read_with_override_content_type;
  /**
   * If operator supports read with if modified since.
   */
  bool read_with_if_modified_since;
  /**
   * If operator supports read with if unmodified since.
   */
  bool read_with_if_unmodified_since;
  /**
   * If operator supports read with version.
   */
  bool read_with_version;
  /**
   * If operator supports write.
   */
  bool write;
  /**
   * If operator supports write can be called in multi times.
   */
  bool write_can_multi;
  /**
   * If operator supports write with empty content.
   */
  bool write_can_empty;
  /**
   * If operator supports write by append.
   */
  bool write_can_append;
  /**
   * If operator supports write with content type.
   */
  bool write_with_content_type;
  /**
   * If operator supports write with content disposition.
   */
  bool write_with_content_disposition;
  /**
   * If operator supports write with content encoding.
   */
  bool write_with_content_encoding;
  /**
   * If operator supports write with cache control.
   */
  bool write_with_cache_control;
  /**
   * If operator supports write with if match.
   */
  bool write_with_if_match;
  /**
   * If operator supports write with if none match.
   */
  bool write_with_if_none_match;
  /**
   * If operator supports write with if not exists.
   */
  bool write_with_if_not_exists;
  /**
   * If operator supports write with user metadata.
   */
  bool write_with_user_metadata;
  /**
   * write_multi_max_size is the max size that services support in write_multi.
   *
   * For example, AWS S3 supports 5GiB as max in write_multi.
   *
   * If it is not set, this will be zero
   */
  uintptr_t write_multi_max_size;
  /**
   * write_multi_min_size is the min size that services support in write_multi.
   *
   * For example, AWS S3 requires at least 5MiB in write_multi expect the last one.
   *
   * If it is not set, this will be zero
   */
  uintptr_t write_multi_min_size;
  /**
   * write_total_max_size is the max size that services support in write_total.
   *
   * For example, Cloudflare D1 supports 1MB as max in write_total.
   *
   * If it is not set, this will be zero
   */
  uintptr_t write_total_max_size;
  /**
   * If operator supports create dir.
   */
  bool create_dir;
  /**
   * If operator supports delete.
   */
  bool delete_;
  /**
   * If operator supports delete with version.
   */
  bool delete_with_version;
  /**
   * If operator supports delete with recursive.
   */
  bool delete_with_recursive;
  /**
   * If operator supports copy.
   */
  bool copy;
  /**
   * If operator supports copy with if not exists.
   */
  bool copy_with_if_not_exists;
  /**
   * If operator supports copy with if match.
   */
  bool copy_with_if_match;
  /**
   * If operator supports copy with source version.
   */
  bool copy_with_source_version;
  /**
   * If operator supports copy can be split into multiple server-side tasks.
   */
  bool copy_can_multi;
  /**
   * copy_multi_max_size is the max size supported for segmented copy tasks.
   *
   * If it is not set, this will be zero
   */
  uintptr_t copy_multi_max_size;
  /**
   * copy_multi_min_size is the min size required for segmented copy tasks.
   *
   * If it is not set, this will be zero
   */
  uintptr_t copy_multi_min_size;
  /**
   * If operator supports rename.
   */
  bool rename;
  /**
   * If operator supports list.
   */
  bool list;
  /**
   * If backend supports list with limit.
   */
  bool list_with_limit;
  /**
   * If backend supports list with start after.
   */
  bool list_with_start_after;
  /**
   * If backend supports list without delimiter.
   */
  bool list_with_recursive;
  /**
   * If backend supports list with versions.
   */
  bool list_with_versions;
  /**
   * If backend supports list with deleted.
   */
  bool list_with_deleted;
  /**
   * If operator supports presign.
   */
  bool presign;
  /**
   * If operator supports presign read.
   */
  bool presign_read;
  /**
   * If operator supports presign stat.
   */
  bool presign_stat;
  /**
   * If operator supports presign write.
   */
  bool presign_write;
  /**
   * If operator supports presign delete.
   */
  bool presign_delete;
  /**
   * If operator supports shared.
   */
  bool shared;
} opendal_capability;

/**
 * \brief The underlying presigned request, which contains the HTTP method, URI, and headers.
 * This is an opaque struct, please use the accessor functions to get the fields.
 */
typedef struct opendal_presigned_request {
  struct opendal_presigned_request_inner *inner;
} opendal_presigned_request;

/**
 * @brief The result of a presign operation.
 */
typedef struct opendal_result_presign {
  /**
   * The presigned request.
   */
  struct opendal_presigned_request *req;
  /**
   * The error.
   */
  struct opendal_error *error;
} opendal_result_presign;

/**
 * \brief The key-value pair for the headers of the presigned request.
 */
typedef struct opendal_http_header_pair {
  /**
   * The key of the header.
   */
  const char *key;
  /**
   * The value of the header.
   */
  const char *value;
} opendal_http_header_pair;

/**
 * \brief The is the result type returned by opendal_reader_read().
 * The result type contains a size field, which is the size of the data read,
 * which is zero on error. The error field is the error code and error message.
 */
typedef struct opendal_result_reader_read {
  /**
   * The read size if succeed.
   */
  uintptr_t size;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_reader_read;

/**
 * \brief The result type returned by opendal_reader_seek().
 * The result type contains a pos field, which is the new position after seek,
 * which is zero on error. The error field is the error code and error message.
 */
typedef struct opendal_result_reader_seek {
  /**
   * New position after seek
   */
  uint64_t pos;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_reader_seek;

/**
 * \brief The result type returned by opendal_writer_write().
 * The result type contains a size field, which is the size of the data written,
 * which is zero on error. The error field is the error code and error message.
 */
typedef struct opendal_result_writer_write {
  /**
   * The write size if succeed.
   */
  uintptr_t size;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_writer_write;

/**
 * \brief The result type returned by opendal_copier_next().
 *
 * A copy is driven by calling opendal_copier_next() repeatedly: each call
 * performs one step of the copy until the copy reports completion.
 *
 * Fields:
 * * `size`: the number of bytes copied in this step; zero on error or completion.
 * * `has_next`: true when the copy made progress and should be driven again by
 *   another opendal_copier_next() call; when false and `error` is null, the copy
 *   has completed.
 * * `error`: the error code and message; null when the step succeeds.
 */
typedef struct opendal_result_copier_next {
  /**
   * The number of bytes copied in this step.
   */
  uintptr_t size;
  /**
   * Whether the copy operation made progress and should be driven again.
   */
  bool has_next;
  /**
   * The error, if ok, it is null
   */
  struct opendal_error *error;
} opendal_result_copier_next;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * \brief Frees the opendal_error, ok to call on NULL
 */
void opendal_error_free(struct opendal_error *ptr);

/**
 * \brief Return the next object to be listed
 *
 * Lister is an iterator of the objects under its path, this method is the same as
 * calling next() on the iterator
 *
 * For examples, please see the comment section of opendal_operator_list()
 * @see opendal_operator_list()
 */
struct opendal_result_lister_next opendal_lister_next(struct opendal_lister *self);

/**
 * \brief Free the heap-allocated metadata used by opendal_lister
 */
void opendal_lister_free(struct opendal_lister *ptr);

/**
 * \brief Construct a heap-allocated opendal_operator_layers.
 */
struct opendal_operator_layers *opendal_operator_layers_new(void);

/**
 * \brief Add a retry layer.
 */
void opendal_operator_layers_add_retry(struct opendal_operator_layers *self,
                                       bool jitter,
                                       float factor,
                                       uint64_t min_delay_ns,
                                       uint64_t max_delay_ns,
                                       uint64_t max_times);

/**
 * \brief Add a timeout layer.
 */
void opendal_operator_layers_add_timeout(struct opendal_operator_layers *self,
                                         uint64_t timeout_ns,
                                         uint64_t io_timeout_ns);

/**
 * \brief Free the allocated memory used by opendal_operator_layers.
 */
void opendal_operator_layers_free(struct opendal_operator_layers *ptr);

/**
 * \brief Free the heap-allocated metadata used by opendal_metadata
 */
void opendal_metadata_free(struct opendal_metadata *ptr);

/**
 * \brief Return mode of the metadata: 0 for unknown, 1 for file, and 2 for dir.
 */
uint8_t opendal_metadata_mode(const struct opendal_metadata *self);

/**
 * \brief Return the content_length of the metadata
 */
uint64_t opendal_metadata_content_length(const struct opendal_metadata *self);

/**
 * \brief Return whether the path represents a file
 */
bool opendal_metadata_is_file(const struct opendal_metadata *self);

/**
 * \brief Return whether the path represents a directory
 */
bool opendal_metadata_is_dir(const struct opendal_metadata *self);

/**
 * \brief Return whether this metadata is current.
 *
 * Returns 1 for current, 0 for not current, and 2 if unknown.
 */
uint8_t opendal_metadata_is_current(const struct opendal_metadata *self);

/**
 * \brief Return whether this metadata is deleted.
 */
bool opendal_metadata_is_deleted(const struct opendal_metadata *self);

/**
 * \brief Return the cache control of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_cache_control(const struct opendal_metadata *self);

/**
 * \brief Return the content disposition of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_content_disposition(const struct opendal_metadata *self);

/**
 * \brief Return the content md5 of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_content_md5(const struct opendal_metadata *self);

/**
 * \brief Return the content type of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_content_type(const struct opendal_metadata *self);

/**
 * \brief Return the content encoding of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_content_encoding(const struct opendal_metadata *self);

/**
 * \brief Return the etag of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_etag(const struct opendal_metadata *self);

/**
 * \brief Return the version of the metadata.
 *
 * \note: The string is on heap, free it with opendal_string_free().
 */
char *opendal_metadata_version(const struct opendal_metadata *self);

/**
 * \brief Return the user metadata of the metadata.
 *
 * \note: The returned user metadata is on heap, free it with opendal_metadata_user_metadata_free().
 */
struct opendal_metadata_user_metadata *opendal_metadata_get_user_metadata(const struct opendal_metadata *self);

/**
 * \brief Return the last_modified of the metadata, in milliseconds
 *
 * # Example
 * ```C
 * // ... previously you wrote "Hello, World!" to path "/testpath"
 * opendal_result_stat s = opendal_operator_stat(op, "/testpath");
 * assert(s.error == NULL);
 *
 * opendal_metadata *meta = s.meta;
 * assert(opendal_metadata_last_modified_ms(meta) != -1);
 * ```
 */
int64_t opendal_metadata_last_modified_ms(const struct opendal_metadata *self);

/**
 * \brief Return the key-value pairs of the user metadata.
 */
const struct opendal_metadata_user_metadata_pair *opendal_metadata_user_metadata_pairs(const struct opendal_metadata_user_metadata *metadata);

/**
 * \brief Return the number of key-value pairs in the user metadata.
 */
uintptr_t opendal_metadata_user_metadata_len(const struct opendal_metadata_user_metadata *metadata);

/**
 * \brief Free the user metadata returned by opendal_metadata_user_metadata.
 */
void opendal_metadata_user_metadata_free(struct opendal_metadata_user_metadata *metadata);

/**
 * \brief Free the heap-allocated operator pointed by opendal_operator.
 *
 * Please only use this for a pointer pointing at a valid opendal_operator.
 * Calling this function on NULL does nothing, but calling this function on pointers
 * of other type will lead to segfault.
 *
 * # Example
 *
 * ```C
 * opendal_operator *op = opendal_operator_new("fs", NULL);
 * // ... use this op, maybe some reads and writes
 *
 * // free this operator
 * opendal_operator_free(op);
 * ```
 */
void opendal_operator_free(const struct opendal_operator *ptr);

/**
 * \brief Construct an operator based on `scheme` and `options`
 *
 * Uses an array of key-value pairs to initialize the operator based on provided `scheme`
 * and `options`. For each scheme, i.e. Backend, different options could be set, you may
 * reference the [documentation](https://opendal.apache.org/docs/category/services/) for
 * each service, especially for the **Configuration Part**.
 *
 * @param scheme the service scheme you want to specify, e.g. "fs", "s3"
 * @param options the pointer to the options for this operator, it could be NULL, which means no
 * option is set
 * @see opendal_operator_options
 * @return A valid opendal_result_operator_new setup with the `scheme` and `options` is the construction
 * succeeds. On success the operator field is a valid pointer to a newly allocated opendal_operator,
 * and the error field is NULL. Otherwise, the operator field is a NULL pointer and the error field.
 *
 * # Example
 *
 * Following is an example.
 * ```C
 * // Allocate a new options
 * opendal_operator_options *options = opendal_operator_options_new();
 * // Set the options you need
 * opendal_operator_options_set(options, "root", "/myroot");
 *
 * // Construct the operator based on the options and scheme
 * opendal_result_operator_new result = opendal_operator_new("memory", options);
 * opendal_operator* op = result.op;
 *
 * // you could free the options right away since the options is not used afterwards
 * opendal_operator_options_free(options);
 *
 * // ... your operations
 * ```
 *
 * # Safety
 *
 * The only unsafe case is passing an invalid c string pointer to the `scheme` argument.
 */
struct opendal_result_operator_new opendal_operator_new(const char *scheme,
                                                        const struct opendal_operator_options *options);

/**
 * \brief Construct an operator based on scheme, options, and explicit layers.
 *
 * Unlike opendal_operator_new, this function will not add any default layer.
 * Layers will be applied exactly as they were added to opendal_operator_layers.
 *
 * # Safety
 *
 * The only unsafe case is passing an invalid c string pointer to the scheme argument.
 */
struct opendal_result_operator_new opendal_operator_new_with_layers(const char *scheme,
                                                                    const struct opendal_operator_options *options,
                                                                    const struct opendal_operator_layers *layers);

/**
 * \brief Blocking write raw bytes to `path`.
 *
 * Write the `bytes` into the `path` blocking by `op_ptr`.
 * Error is NULL if successful, otherwise it contains the error code and error message.
 *
 * \note It is important to notice that the `bytes` that is passes in will be consumed by this
 *       function. Therefore, you should not use the `bytes` after this function returns.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path you want to write your bytes in
 * @param bytes The opendal_byte typed bytes to be written
 * @see opendal_operator
 * @see opendal_bytes
 * @see opendal_error
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * // prepare your data
 * char* data = "Hello, World!";
 * opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
 *
 * // now you can write!
 * opendal_error *err = opendal_operator_write(op, "/testpath", bytes);
 *
 * // Assert that this succeeds
 * assert(err == NULL);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 * * If `bytes.len` is greater than 0, `bytes.data` must point to at least
 *   `bytes.len` valid bytes. If `bytes.len` is 0, `bytes.data` must be NULL.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_write(const struct opendal_operator *op,
                                             const char *path,
                                             const struct opendal_bytes *bytes);

/**
 * \brief Blocking write raw bytes to `path` with options.
 */
struct opendal_error *opendal_operator_write_with(const struct opendal_operator *op,
                                                  const char *path,
                                                  const struct opendal_bytes *bytes,
                                                  const struct opendal_write_options *opts);

/**
 * \brief Blocking write raw bytes to `path`, returning the written object's metadata.
 *
 * Like `opendal_operator_write_with`, but on success returns the metadata of the
 * just-written object (e.g. etag, version, last modified) instead of discarding it.
 * A NULL `opts` is treated as the default options, behaving like a plain write.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path where the data will be written
 * @param bytes The data to write
 * @param opts The write options, or NULL to use the defaults
 * @see opendal_operator
 * @see opendal_write_options
 * @see opendal_result_write
 * @return Returns opendal_result_write, containing the metadata and an opendal_error.
 * If the operation succeeds, the `meta` field holds the metadata and the `error` field
 * is null. Otherwise, the `meta` will be null and the `error` will be set correspondingly.
 *
 * \note The returned metadata must be freed with opendal_metadata_free().
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 * * The `bytes` provided has valid byte in the `data` field and the `len` field is set
 *   correctly.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_write opendal_operator_write_with_metadata(const struct opendal_operator *op,
                                                                 const char *path,
                                                                 const struct opendal_bytes *bytes,
                                                                 const struct opendal_write_options *opts);

/**
 * \brief Blocking read the data from `path`.
 *
 * Read the data out from `path` blocking by operator.
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to read the data out
 * @see opendal_operator
 * @see opendal_result_read
 * @see opendal_error
 * @return Returns opendal_result_read, the `data` field is a pointer to a newly allocated
 * opendal_bytes, the `error` field contains the error. If the `error` is not NULL, then
 * the operation failed and the `data` field is a nullptr.
 *
 * \note If the read operation succeeds, the returned opendal_bytes is newly allocated on heap.
 * After your usage of that, please call opendal_bytes_free() to free the space.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * // ... you have write "Hello, World!" to path "/testpath"
 *
 * opendal_result_read r = opendal_operator_read(op, "testpath");
 * assert(r.error == NULL);
 *
 * opendal_bytes bytes = r.data;
 * assert(bytes.len == 13);
 * opendal_bytes_free(&bytes);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_read opendal_operator_read(const struct opendal_operator *op,
                                                 const char *path);

/**
 * \brief Blocking read the data from `path` with options.
 *
 * Read the data out from `path` blocking by operator, using the provided
 * `opendal_read_options` to control the behavior, e.g. range, version, or
 * conditional headers.
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to read the data out
 * @param opts The options for the read operation; pass NULL to use defaults
 * @see opendal_operator
 * @see opendal_result_read
 * @see opendal_read_options
 * @see opendal_error
 * @return Returns opendal_result_read, the `data` field is a pointer to a newly allocated
 * opendal_bytes, the `error` field contains the error. If the `error` is not NULL, then
 * the operation failed and the `data` field is a nullptr.
 *
 * \note If the read operation succeeds, the returned opendal_bytes is newly allocated on heap.
 * After your usage of that, please call opendal_bytes_free() to free the space.
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_read opendal_operator_read_with(const struct opendal_operator *op,
                                                      const char *path,
                                                      const struct opendal_read_options *opts);

/**
 * \brief Blocking read the data from `path`.
 *
 * Read the data out from `path` blocking by operator, returns
 * an opendal_result_read with error code.
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to read the data out
 * @see opendal_operator
 * @see opendal_result_read
 * @see opendal_code
 * @return Returns opendal_code
 *
 * \note If the read operation succeeds, the returned opendal_bytes is newly allocated on heap.
 * After your usage of that, please call opendal_bytes_free() to free the space.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * // ... you have created an operator named op
 *
 * opendal_result_operator_reader result = opendal_operator_reader(op, "/testpath");
 * assert(result.error == NULL);
 * // The reader is in result.reader
 * opendal_reader *reader = result.reader;
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_operator_reader opendal_operator_reader(const struct opendal_operator *op,
                                                              const char *path);

/**
 * \brief Blocking create a reader for the specified path with options.
 *
 * This function prepares a reader, applying the conditional, version and
 * concurrency options carried by `opts`. A NULL `opts` is treated as the
 * default options, behaving like `opendal_operator_reader`.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path where the reader will be used
 * @param opts The reader options, or NULL to use the defaults
 * @see opendal_operator
 * @see opendal_reader_options
 * @see opendal_result_operator_reader
 * @return Returns opendal_result_operator_reader, containing a reader and an opendal_error.
 * If the operation succeeds, the `reader` field holds a valid reader and the `error` field
 * is null. Otherwise, the `reader` will be null and the `error` will be set correspondingly.
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_operator_reader opendal_operator_reader_with(const struct opendal_operator *op,
                                                                   const char *path,
                                                                   const struct opendal_reader_options *opts);

/**
 * \brief Blocking create a writer for the specified path.
 *
 * This function prepares a writer that can be used to write data to the specified path
 * using the provided operator. If successful, it returns a valid writer; otherwise, it
 * returns an error.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path where the writer will be used
 * @see opendal_operator
 * @see opendal_result_operator_writer
 * @see opendal_error
 * @return Returns opendal_result_operator_writer, containing a writer and an opendal_error.
 * If the operation succeeds, the `writer` field holds a valid writer and the `error` field
 * is null. Otherwise, the `writer` will be null and the `error` will be set correspondingly.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * opendal_result_operator_writer result = opendal_operator_writer(op, "/testpath");
 * assert(result.error == NULL);
 * opendal_writer *writer = result.writer;
 * // Use the writer to write data...
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_operator_writer opendal_operator_writer(const struct opendal_operator *op,
                                                              const char *path);

/**
 * \brief Blocking create a writer for the specified path with options.
 */
struct opendal_result_operator_writer opendal_operator_writer_with(const struct opendal_operator *op,
                                                                   const char *path,
                                                                   const struct opendal_write_options *opts);

/**
 * \brief Blocking delete the object in `path`.
 *
 * Delete the object in `path` blocking by `op_ptr`.
 * Error is NULL if successful, otherwise it contains the error code and error message.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path you want to delete
 * @see opendal_operator
 * @see opendal_error
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * // prepare your data
 * char* data = "Hello, World!";
 * opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
 * opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
 *
 * assert(error == NULL);
 *
 * // now you can delete!
 * opendal_error *error = opendal_operator_delete(op, "/testpath");
 *
 * // Assert that this succeeds
 * assert(error == NULL);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_delete(const struct opendal_operator *op, const char *path);

/**
 * \brief Blocking delete the object in `path` with options.
 *
 * Delete the object in `path` blocking by `op`, using the provided `opendal_delete_options`.
 * This is similar to `opendal_operator_delete` but allows specifying a version or
 * requesting a recursive delete.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path you want to delete
 * @param opts The options for the delete operation; pass NULL to use defaults
 * @see opendal_delete_options
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Safety
 *
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_delete_with(const struct opendal_operator *op,
                                                   const char *path,
                                                   const struct opendal_delete_options *opts);

/**
 * \brief Check whether the path exists.
 *
 * If the operation succeeds, no matter the path exists or not,
 * the error should be a nullptr. Otherwise, the field `is_exist`
 * is filled with false, and the error is set
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to check existence
 * @see opendal_operator
 * @see opendal_result_is_exist
 * @see opendal_error
 * @return Returns opendal_result_is_exist, the `is_exist` field contains whether the path exists.
 * However, it the operation fails, the `is_exist` will contain false and the error will be set.
 *
 * # Example
 *
 * ```C
 * // .. you previously wrote some data to path "/mytest/obj"
 * opendal_result_is_exist e = opendal_operator_is_exist(op, "/mytest/obj");
 * assert(e.error == NULL);
 * assert(e.is_exist);
 *
 * // but you previously did **not** write any data to path "/yourtest/obj"
 * opendal_result_is_exist e = opendal_operator_is_exist(op, "/yourtest/obj");
 * assert(e.error == NULL);
 * assert(!e.is_exist);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_is_exist opendal_operator_is_exist(const struct opendal_operator *op,
                                                         const char *path);

/**
 * \brief Check whether the path exists.
 *
 * If the operation succeeds, no matter the path exists or not,
 * the error should be a nullptr. Otherwise, the field `exists`
 * is filled with false, and the error is set
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to check existence
 * @see opendal_operator
 * @see opendal_result_exists
 * @see opendal_error
 * @return Returns opendal_result_exists, the `exists` field contains whether the path exists.
 * However, it the operation fails, the `exists` will contain false and the error will be set.
 *
 * # Example
 *
 * ```C
 * // .. you previously wrote some data to path "/mytest/obj"
 * opendal_result_exists e = opendal_operator_exists(op, "/mytest/obj");
 * assert(e.error == NULL);
 * assert(e.exists);
 *
 * // but you previously did **not** write any data to path "/yourtest/obj"
 * opendal_result_exists e = opendal_operator_exists(op, "/yourtest/obj");
 * assert(e.error == NULL);
 * assert(!e.exists);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_exists opendal_operator_exists(const struct opendal_operator *op,
                                                     const char *path);

/**
 * \brief Stat the path, return its metadata.
 *
 * Error is NULL if successful, otherwise it contains the error code and error message.
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to stat
 * @see opendal_operator
 * @see opendal_result_stat
 * @see opendal_metadata
 * @return Returns opendal_result_stat, containing a metadata and an opendal_error.
 * If the operation succeeds, the `meta` field would hold a valid metadata and
 * the `error` field should hold nullptr. Otherwise, the metadata will contain a
 * NULL pointer, i.e. invalid, and the `error` will be set correspondingly.
 *
 * # Example
 *
 * ```C
 * // ... previously you wrote "Hello, World!" to path "/testpath"
 * opendal_result_stat s = opendal_operator_stat(op, "/testpath");
 * assert(s.error == NULL);
 *
 * const opendal_metadata *meta = s.meta;
 *
 * // ... you could now use your metadata, notice that please only access metadata
 * // using the APIs provided by OpenDAL
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_stat opendal_operator_stat(const struct opendal_operator *op,
                                                 const char *path);

/**
 * \brief Blocking stat the object in `path` with options.
 *
 * Stat the object in `path` with the provided `opendal_stat_options`. This is
 * similar to `opendal_operator_stat` but allows passing options such as
 * `version`, `if_match`, `if_none_match`, or response header overrides.
 *
 * @param op The opendal_operator created previously
 * @param path The path you want to stat
 * @param opts The options for the stat operation; pass NULL to use defaults
 * @see opendal_operator
 * @see opendal_result_stat
 * @see opendal_stat_options
 * @return Returns opendal_result_stat, containing a metadata and an opendal_error.
 *
 * # Safety
 *
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_stat opendal_operator_stat_with(const struct opendal_operator *op,
                                                      const char *path,
                                                      const struct opendal_stat_options *opts);

/**
 * \brief Blocking list the objects in `path`.
 *
 * List the object in `path` blocking by `op_ptr`, return a result with an
 * opendal_lister. Users should call opendal_lister_next() on the
 * lister.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path you want to list
 * @see opendal_lister
 * @return Returns opendal_result_list, containing a lister and an opendal_error.
 * If the operation succeeds, the `lister` field would hold a valid lister and
 * the `error` field should hold nullptr. Otherwise, the `lister`` will contain a
 * NULL pointer, i.e. invalid, and the `error` will be set correspondingly.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * // You have written some data into some files path "root/dir1"
 * // Your opendal_operator was called op
 * opendal_result_list l = opendal_operator_list(op, "root/dir1");
 * assert(l.error == ERROR);
 *
 * opendal_lister *lister = l.lister;
 * opendal_list_entry *entry;
 *
 * while ((entry = opendal_lister_next(lister)) != NULL) {
 *     const char* de_path = opendal_list_entry_path(entry);
 *     const char* de_name = opendal_list_entry_name(entry);
 *     // ...... your operations
 *
 *     // remember to free the entry after you are done using it
 *     opendal_list_entry_free(entry);
 * }
 *
 * // and remember to free the lister
 * opendal_lister_free(lister);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_list opendal_operator_list(const struct opendal_operator *op,
                                                 const char *path);

/**
 * \brief Blocking list the objects in `path` with options.
 *
 * List the objects in `path` with the provided `opendal_list_options`. This is
 * similar to `opendal_operator_list` but allows passing options such as
 * `recursive` to control the listing behavior.
 *
 * @param op The opendal_operator created previously
 * @param path The designated path you want to list
 * @param opts The options for the list operation; pass NULL to use defaults
 * @see opendal_lister
 * @see opendal_list_options
 * @return Returns opendal_result_list, containing a lister and an opendal_error.
 *
 * # Safety
 *
 * * The memory pointed to by `path` must contain a valid null terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_result_list opendal_operator_list_with(const struct opendal_operator *op,
                                                      const char *path,
                                                      const struct opendal_list_options *opts);

/**
 * \brief Blocking create the directory in `path`.
 *
 * Create the directory in `path` blocking by `op_ptr`.
 * Error is NULL if successful, otherwise it contains the error code and error message.
 *
 * @param op The opendal_operator created previously
 * @param path The designated directory you want to create
 * @see opendal_operator
 * @see opendal_error
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * // create your directory
 * opendal_error *error = opendal_operator_create_dir(op, "/testdir/");
 *
 * // Assert that this succeeds
 * assert(error == NULL);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `path` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_create_dir(const struct opendal_operator *op,
                                                  const char *path);

/**
 * \brief Blocking rename the object in `path`.
 *
 * Rename the object in `src` to `dest` blocking by `op`.
 * Error is NULL if successful, otherwise it contains the error code and error message.
 *
 * @param op The opendal_operator created previously
 * @param src The designated source path you want to rename
 * @param dest The designated destination path you want to rename
 * @see opendal_operator
 * @see opendal_error
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * // prepare your data
 * char* data = "Hello, World!";
 * opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
 * opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
 *
 * assert(error == NULL);
 *
 * // now you can rename!
 * opendal_error *error = opendal_operator_rename(op, "/testpath", "/testpath2");
 *
 * // Assert that this succeeds
 * assert(error == NULL);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `src` or `dest` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_rename(const struct opendal_operator *op,
                                              const char *src,
                                              const char *dest);

/**
 * \brief Blocking copy the object in `path`.
 *
 * Copy the object in `src` to `dest` blocking by `op`.
 * Error is NULL if successful, otherwise it contains the error code and error message.
 *
 * @param op The opendal_operator created previously
 * @param src The designated source path you want to copy
 * @param dest The designated destination path you want to copy
 * @see opendal_operator
 * @see opendal_error
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * // prepare your data
 * char* data = "Hello, World!";
 * opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
 * opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
 *
 * assert(error == NULL);
 *
 * // now you can rename!
 * opendal_error *error = opendal_operator_copy(op, "/testpath", "/testpath2");
 *
 * // Assert that this succeeds
 * assert(error == NULL);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `path` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `src` or `dest` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_copy(const struct opendal_operator *op,
                                            const char *src,
                                            const char *dest);

/**
 * \brief Blocking copy the object in `path` with options.
 *
 * Copy the object from `src` to `dest` blocking by `op`, using the provided
 * `opendal_copy_options` to control the behavior, e.g. `if_not_exists` or
 * `if_match` conditions.
 *
 * @param op The opendal_operator created previously
 * @param src The designated source path you want to copy
 * @param dest The designated destination path you want to copy
 * @param opts The options for the copy operation; pass NULL to use defaults
 * @see opendal_operator
 * @see opendal_copy_options
 * @see opendal_error
 * @return NULL if succeeds, otherwise it contains the error code and error message.
 *
 * # Example
 *
 * Following is an example
 * ```C
 * //...prepare your opendal_operator, named op for example
 *
 * // prepare your data
 * char* data = "Hello, World!";
 * opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
 * opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
 *
 * assert(error == NULL);
 *
 * // prepare options
 * opendal_copy_options *opts = opendal_copy_options_new();
 * opendal_copy_options_set_if_not_exists(opts, true);
 *
 * // now you can copy with options!
 * opendal_error *error = opendal_operator_copy_with(op, "/testpath", "/testpath2", opts);
 *
 * // Assert that this succeeds
 * assert(error == NULL);
 *
 * // remember to free the options
 * opendal_copy_options_free(opts);
 * ```
 *
 * # Safety
 *
 * It is **safe** under the cases below
 * * The memory pointed to by `src` and `dest` must contain a valid nul terminator at the end of
 *   the string.
 *
 * # Panic
 *
 * * If the `src` or `dest` points to NULL, this function panics, i.e. exits with information
 */
struct opendal_error *opendal_operator_copy_with(const struct opendal_operator *op,
                                                 const char *src,
                                                 const char *dest,
                                                 const struct opendal_copy_options *opts);

struct opendal_error *opendal_operator_check(const struct opendal_operator *op);

/**
 * \brief Blocking create a copier to copy a file from `src` to `dest`.
 *
 * The returned copier is used to complete a long-running copy operation. Call
 * `opendal_copier_next` repeatedly to make progress, and `opendal_copier_free`
 * to release it once finished.
 *
 * @param op The opendal_operator created previously
 * @param src The designated source path you want to copy
 * @param dest The designated destination path you want to copy
 * @see opendal_operator
 * @see opendal_copier
 * @see opendal_result_operator_copier
 * @return opendal_result_operator_copier, containing a copier and an opendal_error.
 * If the operation succeeds, the `copier` field holds a valid copier and the `error`
 * field is null. Otherwise, the `copier` will be null and the `error` will be set
 * correspondingly.
 *
 * # Safety
 *
 * * The memory pointed to by `src` and `dest` must contain a valid nul terminator at the
 *   end of the string.
 *
 * # Panic
 *
 * * If the `src` or `dest` points to NULL, this function panics
 */
struct opendal_result_operator_copier opendal_operator_copier(const struct opendal_operator *op,
                                                              const char *src,
                                                              const char *dest);

/**
 * \brief Blocking create a copier to copy a file from `src` to `dest` with options.
 *
 * This is the same as `opendal_operator_copier` but accepts an `opendal_copy_options`
 * to control the behavior, e.g. `concurrent` or `chunk`. Pass NULL to use defaults.
 *
 * @param op The opendal_operator created previously
 * @param src The designated source path you want to copy
 * @param dest The designated destination path you want to copy
 * @param opts The options for the copy operation; pass NULL to use defaults
 * @see opendal_operator_copier
 * @see opendal_copy_options
 * @return opendal_result_operator_copier, containing a copier and an opendal_error.
 */
struct opendal_result_operator_copier opendal_operator_copier_with(const struct opendal_operator *op,
                                                                   const char *src,
                                                                   const char *dest,
                                                                   const struct opendal_copy_options *opts);

/**
 * \brief Get information of underlying accessor.
 *
 * # Example
 *
 * ```C
 * /// suppose you have a memory-backed opendal_operator* named op
 * char *scheme;
 * opendal_operator_info *info = opendal_operator_info_new(op);
 *
 * scheme = opendal_operator_info_get_scheme(info);
 * assert(!strcmp(scheme, "memory"));
 *
 * /// free the heap memory
 * opendal_string_free(scheme);
 * opendal_operator_info_free(info);
 * ```
 */
struct opendal_operator_info *opendal_operator_info_new(const struct opendal_operator *op);

/**
 * \brief Free the heap-allocated opendal_operator_info
 */
void opendal_operator_info_free(struct opendal_operator_info *ptr);

/**
 * \brief Return the nul-terminated operator's scheme, i.e. service
 *
 * \note: The string is on heap, free it with opendal_string_free()
 */
char *opendal_operator_info_get_scheme(const struct opendal_operator_info *self);

/**
 * \brief Return the nul-terminated operator's working root path
 *
 * \note: The string is on heap, free it with opendal_string_free()
 */
char *opendal_operator_info_get_root(const struct opendal_operator_info *self);

/**
 * \brief Return the nul-terminated operator backend's name, could be empty if underlying backend has no
 * namespace concept.
 *
 * \note: The string is on heap, free it with opendal_string_free()
 */
char *opendal_operator_info_get_name(const struct opendal_operator_info *self);

/**
 * \brief Return the operator's capability
 */
struct opendal_capability opendal_operator_info_get_capability(const struct opendal_operator_info *self);

/**
 * \brief Presign a read operation.
 */
struct opendal_result_presign opendal_operator_presign_read(const struct opendal_operator *op,
                                                            const char *path,
                                                            uint64_t expire_secs);

/**
 * \brief Presign a read operation with options.
 */
struct opendal_result_presign opendal_operator_presign_read_with(const struct opendal_operator *op,
                                                                 const char *path,
                                                                 uint64_t expire_secs,
                                                                 const struct opendal_read_options *opts);

/**
 * \brief Presign a write operation.
 */
struct opendal_result_presign opendal_operator_presign_write(const struct opendal_operator *op,
                                                             const char *path,
                                                             uint64_t expire_secs);

/**
 * \brief Presign a write operation with options.
 */
struct opendal_result_presign opendal_operator_presign_write_with(const struct opendal_operator *op,
                                                                  const char *path,
                                                                  uint64_t expire_secs,
                                                                  const struct opendal_write_options *opts);

/**
 * \brief Presign a delete operation.
 */
struct opendal_result_presign opendal_operator_presign_delete(const struct opendal_operator *op,
                                                              const char *path,
                                                              uint64_t expire_secs);

/**
 * \brief Presign a delete operation with options.
 */
struct opendal_result_presign opendal_operator_presign_delete_with(const struct opendal_operator *op,
                                                                   const char *path,
                                                                   uint64_t expire_secs,
                                                                   const struct opendal_delete_options *opts);

/**
 * \brief Presign a stat operation.
 */
struct opendal_result_presign opendal_operator_presign_stat(const struct opendal_operator *op,
                                                            const char *path,
                                                            uint64_t expire_secs);

/**
 * \brief Presign a stat operation with options.
 */
struct opendal_result_presign opendal_operator_presign_stat_with(const struct opendal_operator *op,
                                                                 const char *path,
                                                                 uint64_t expire_secs,
                                                                 const struct opendal_stat_options *opts);

/**
 * Get the method of the presigned request.
 */
const char *opendal_presigned_request_method(const struct opendal_presigned_request *req);

/**
 * Get the URI of the presigned request.
 */
const char *opendal_presigned_request_uri(const struct opendal_presigned_request *req);

/**
 * Get the headers of the presigned request.
 */
const struct opendal_http_header_pair *opendal_presigned_request_headers(const struct opendal_presigned_request *req);

/**
 * Get the length of the headers of the presigned request.
 */
uintptr_t opendal_presigned_request_headers_len(const struct opendal_presigned_request *req);

/**
 * \brief Free the presigned request.
 */
void opendal_presigned_request_free(struct opendal_presigned_request *req);

/**
 * \brief Frees a heap-allocated string returned by OpenDAL C APIs.
 *
 * \note Only pass pointers returned from OpenDAL APIs that transfer string ownership.
 */
void opendal_string_free(char *ptr);

/**
 * \brief Frees the heap memory used by the opendal_bytes
 */
void opendal_bytes_free(struct opendal_bytes *ptr);

/**
 * \brief Construct a heap-allocated opendal_list_options with default values.
 *
 * @return A new opendal_list_options with all options set to their defaults.
 *
 * @see opendal_list_options_free
 */
struct opendal_list_options *opendal_list_options_new(void);

/**
 * \brief Set the recursive option.
 *
 * @param opts The opendal_list_options to modify.
 * @param recursive Whether to list recursively.
 */
void opendal_list_options_set_recursive(struct opendal_list_options *opts, bool recursive);

/**
 * \brief Set the limit option.
 *
 * @param opts The opendal_list_options to modify.
 * @param limit Maximum number of results per request; 0 means unset.
 */
void opendal_list_options_set_limit(struct opendal_list_options *opts, uintptr_t limit);

/**
 * \brief Set the start_after option.
 *
 * Passes the specified key to the underlying service to start listing from.
 *
 * @param opts The opendal_list_options to modify.
 * @param start_after The key to start listing from; NULL to unset.
 */
void opendal_list_options_set_start_after(struct opendal_list_options *opts,
                                          const char *start_after);

/**
 * \brief Set the versions option.
 *
 * @param opts The opendal_list_options to modify.
 * @param versions Whether to include object versions.
 */
void opendal_list_options_set_versions(struct opendal_list_options *opts, bool versions);

/**
 * \brief Set the deleted option.
 *
 * @param opts The opendal_list_options to modify.
 * @param deleted Whether to include delete markers.
 */
void opendal_list_options_set_deleted(struct opendal_list_options *opts, bool deleted);

/**
 * \brief Free the heap memory used by opendal_list_options.
 *
 * @param opts The opendal_list_options to free.
 */
void opendal_list_options_free(struct opendal_list_options *opts);

/**
 * \brief Construct a heap-allocated opendal_delete_options with default values.
 *
 * @return A new opendal_delete_options with all options set to their defaults.
 *
 * @see opendal_delete_options_free
 */
struct opendal_delete_options *opendal_delete_options_new(void);

/**
 * \brief Set the version option.
 *
 * @param opts The opendal_delete_options to modify.
 * @param version The version string to delete; NULL to unset.
 */
void opendal_delete_options_set_version(struct opendal_delete_options *opts, const char *version);

/**
 * \brief Set the recursive option.
 *
 * @param opts The opendal_delete_options to modify.
 * @param recursive Whether to delete recursively.
 */
void opendal_delete_options_set_recursive(struct opendal_delete_options *opts, bool recursive);

/**
 * \brief Free the heap memory used by opendal_delete_options.
 *
 * @param opts The opendal_delete_options to free.
 */
void opendal_delete_options_free(struct opendal_delete_options *opts);

/**
 * \brief Construct a heap-allocated opendal_write_options with default values.
 */
struct opendal_write_options *opendal_write_options_new(void);

/**
 * \brief Free the heap memory used by opendal_write_options.
 */
void opendal_write_options_free(struct opendal_write_options *opts);

/**
 * \brief Set append mode.
 */
void opendal_write_options_set_append(struct opendal_write_options *opts, bool append);

/**
 * \brief Set Cache-Control.
 */
void opendal_write_options_set_cache_control(struct opendal_write_options *opts,
                                             const char *cache_control);

/**
 * \brief Set Content-Type.
 */
void opendal_write_options_set_content_type(struct opendal_write_options *opts,
                                            const char *content_type);

/**
 * \brief Set Content-Disposition.
 */
void opendal_write_options_set_content_disposition(struct opendal_write_options *opts,
                                                   const char *content_disposition);

/**
 * \brief Set Content-Encoding.
 */
void opendal_write_options_set_content_encoding(struct opendal_write_options *opts,
                                                const char *content_encoding);

/**
 * \brief Set If-Match.
 */
void opendal_write_options_set_if_match(struct opendal_write_options *opts, const char *if_match);

/**
 * \brief Set If-None-Match.
 */
void opendal_write_options_set_if_none_match(struct opendal_write_options *opts,
                                             const char *if_none_match);

/**
 * \brief Set if_not_exists.
 */
void opendal_write_options_set_if_not_exists(struct opendal_write_options *opts,
                                             bool if_not_exists);

/**
 * \brief Set concurrent.
 */
void opendal_write_options_set_concurrent(struct opendal_write_options *opts, uintptr_t concurrent);

/**
 * \brief Set chunk.
 */
void opendal_write_options_set_chunk(struct opendal_write_options *opts, uintptr_t chunk);

/**
 * \brief Set user metadata.
 */
void opendal_write_options_set_user_metadata(struct opendal_write_options *opts,
                                             const struct opendal_write_user_metadata_pair *pairs,
                                             uintptr_t len);

/**
 * \brief Construct a heap-allocated opendal_stat_options with default values.
 */
struct opendal_stat_options *opendal_stat_options_new(void);

/**
 * \brief Free the heap memory used by opendal_stat_options.
 */
void opendal_stat_options_free(struct opendal_stat_options *opts);

/**
 * \brief Set the version.
 */
void opendal_stat_options_set_version(struct opendal_stat_options *opts, const char *version);

/**
 * \brief Set If-Match.
 */
void opendal_stat_options_set_if_match(struct opendal_stat_options *opts, const char *if_match);

/**
 * \brief Set If-None-Match.
 */
void opendal_stat_options_set_if_none_match(struct opendal_stat_options *opts,
                                            const char *if_none_match);

/**
 * \brief Set If-Modified-Since in milliseconds since the Unix epoch.
 */
void opendal_stat_options_set_if_modified_since(struct opendal_stat_options *opts,
                                                int64_t if_modified_since);

/**
 * \brief Set If-Unmodified-Since in milliseconds since the Unix epoch.
 */
void opendal_stat_options_set_if_unmodified_since(struct opendal_stat_options *opts,
                                                  int64_t if_unmodified_since);

/**
 * \brief Set the override Content-Type.
 */
void opendal_stat_options_set_override_content_type(struct opendal_stat_options *opts,
                                                    const char *override_content_type);

/**
 * \brief Set the override Cache-Control.
 */
void opendal_stat_options_set_override_cache_control(struct opendal_stat_options *opts,
                                                     const char *override_cache_control);

/**
 * \brief Set the override Content-Disposition.
 */
void opendal_stat_options_set_override_content_disposition(struct opendal_stat_options *opts,
                                                           const char *override_content_disposition);

/**
 * \brief Construct a heap-allocated opendal_read_options with default values.
 */
struct opendal_read_options *opendal_read_options_new(void);

/**
 * \brief Free the heap memory used by opendal_read_options.
 */
void opendal_read_options_free(struct opendal_read_options *opts);

/**
 * \brief Set the read range offset and length.
 */
void opendal_read_options_set_range(struct opendal_read_options *opts,
                                    uint64_t offset,
                                    uint64_t length);

/**
 * \brief Set the read range to start at `offset` and extend to the end of the file.
 */
void opendal_read_options_set_range_from(struct opendal_read_options *opts, uint64_t offset);

/**
 * \brief Set the version of the object to read.
 */
void opendal_read_options_set_version(struct opendal_read_options *opts, const char *version);

/**
 * \brief Set If-Match.
 */
void opendal_read_options_set_if_match(struct opendal_read_options *opts, const char *if_match);

/**
 * \brief Set If-None-Match.
 */
void opendal_read_options_set_if_none_match(struct opendal_read_options *opts,
                                            const char *if_none_match);

/**
 * \brief Set If-Modified-Since, in Unix milliseconds.
 */
void opendal_read_options_set_if_modified_since(struct opendal_read_options *opts,
                                                int64_t if_modified_since);

/**
 * \brief Set If-Unmodified-Since, in Unix milliseconds.
 */
void opendal_read_options_set_if_unmodified_since(struct opendal_read_options *opts,
                                                  int64_t if_unmodified_since);

/**
 * \brief Set concurrent read operations.
 */
void opendal_read_options_set_concurrent(struct opendal_read_options *opts, uintptr_t concurrent);

/**
 * \brief Set chunk size.
 */
void opendal_read_options_set_chunk(struct opendal_read_options *opts, uintptr_t chunk);

/**
 * \brief Set gap size.
 */
void opendal_read_options_set_gap(struct opendal_read_options *opts, uintptr_t gap);

/**
 * \brief Set the known content length of the object.
 *
 * This is an execution hint that allows OpenDAL to avoid extra metadata
 * requests while planning reads. It must not be used as an object identity
 * or consistency condition.
 */
void opendal_read_options_set_content_length_hint(struct opendal_read_options *opts,
                                                  uint64_t content_length_hint);

/**
 * \brief Set the override Content-Type (presign only).
 */
void opendal_read_options_set_override_content_type(struct opendal_read_options *opts,
                                                    const char *override_content_type);

/**
 * \brief Set the override Cache-Control (presign only).
 */
void opendal_read_options_set_override_cache_control(struct opendal_read_options *opts,
                                                     const char *override_cache_control);

/**
 * \brief Set the override Content-Disposition (presign only).
 */
void opendal_read_options_set_override_content_disposition(struct opendal_read_options *opts,
                                                           const char *override_content_disposition);

/**
 * \brief Construct a heap-allocated opendal_reader_options with default values.
 */
struct opendal_reader_options *opendal_reader_options_new(void);

/**
 * \brief Free the heap memory used by opendal_reader_options.
 */
void opendal_reader_options_free(struct opendal_reader_options *opts);

/**
 * \brief Set the version of the object to read.
 */
void opendal_reader_options_set_version(struct opendal_reader_options *opts, const char *version);

/**
 * \brief Set If-Match.
 */
void opendal_reader_options_set_if_match(struct opendal_reader_options *opts, const char *if_match);

/**
 * \brief Set If-None-Match.
 */
void opendal_reader_options_set_if_none_match(struct opendal_reader_options *opts,
                                              const char *if_none_match);

/**
 * \brief Set If-Modified-Since, in Unix milliseconds.
 */
void opendal_reader_options_set_if_modified_since(struct opendal_reader_options *opts,
                                                  int64_t if_modified_since);

/**
 * \brief Set If-Unmodified-Since, in Unix milliseconds.
 */
void opendal_reader_options_set_if_unmodified_since(struct opendal_reader_options *opts,
                                                    int64_t if_unmodified_since);

/**
 * \brief Set the known content length of the object.
 *
 * This is an execution hint that allows OpenDAL to avoid extra metadata
 * requests while planning reads. It must not be used as an object identity
 * or consistency condition.
 */
void opendal_reader_options_set_content_length_hint(struct opendal_reader_options *opts,
                                                    uint64_t content_length_hint);

/**
 * \brief Set concurrent read operations.
 */
void opendal_reader_options_set_concurrent(struct opendal_reader_options *opts,
                                           uintptr_t concurrent);

/**
 * \brief Set chunk size.
 */
void opendal_reader_options_set_chunk(struct opendal_reader_options *opts, uintptr_t chunk);

/**
 * \brief Set gap size.
 */
void opendal_reader_options_set_gap(struct opendal_reader_options *opts, uintptr_t gap);

/**
 * \brief Set the number of prefetched byte ranges buffered during concurrent reads.
 */
void opendal_reader_options_set_prefetch(struct opendal_reader_options *opts, uintptr_t prefetch);

/**
 * \brief Construct a heap-allocated opendal_copy_options with default values.
 */
struct opendal_copy_options *opendal_copy_options_new(void);

/**
 * \brief Free the heap memory used by opendal_copy_options.
 */
void opendal_copy_options_free(struct opendal_copy_options *opts);

/**
 * \brief Set if_not_exists.
 */
void opendal_copy_options_set_if_not_exists(struct opendal_copy_options *opts, bool if_not_exists);

/**
 * \brief Set If-Match.
 */
void opendal_copy_options_set_if_match(struct opendal_copy_options *opts, const char *if_match);

/**
 * \brief Set source version.
 */
void opendal_copy_options_set_source_version(struct opendal_copy_options *opts,
                                             const char *source_version);

/**
 * \brief Set source_content_length_hint.
 */
void opendal_copy_options_set_source_content_length_hint(struct opendal_copy_options *opts,
                                                         uint64_t source_content_length_hint);

/**
 * \brief Set concurrent.
 */
void opendal_copy_options_set_concurrent(struct opendal_copy_options *opts, uintptr_t concurrent);

/**
 * \brief Set chunk.
 */
void opendal_copy_options_set_chunk(struct opendal_copy_options *opts, uintptr_t chunk);

/**
 * \brief Construct a heap-allocated opendal_operator_options
 *
 * @return An empty opendal_operator_option, which could be set by
 * opendal_operator_option_set().
 *
 * @see opendal_operator_option_set
 */
struct opendal_operator_options *opendal_operator_options_new(void);

/**
 * \brief Set a Key-Value pair inside opendal_operator_options
 *
 * # Safety
 *
 * This function is unsafe because it dereferences and casts the raw pointers
 * Make sure the pointer of `key` and `value` point to a valid string.
 *
 * # Example
 *
 * ```C
 * opendal_operator_options *options = opendal_operator_options_new();
 * opendal_operator_options_set(options, "root", "/myroot");
 *
 * // .. use your opendal_operator_options
 *
 * opendal_operator_options_free(options);
 * ```
 */
void opendal_operator_options_set(struct opendal_operator_options *self,
                                  const char *key,
                                  const char *value);

/**
 * \brief Free the allocated memory used by [`opendal_operator_options`]
 */
void opendal_operator_options_free(struct opendal_operator_options *ptr);

/**
 * \brief Path of entry.
 *
 * Path is relative to operator's root. Only valid in current operator.
 *
 * \note Free the returned string with opendal_string_free()
 */
char *opendal_entry_path(const struct opendal_entry *self);

/**
 * \brief Name of entry.
 *
 * Name is the last segment of path.
 * If this entry is a dir, `Name` MUST endswith `/`
 * Otherwise, `Name` MUST NOT endswith `/`.
 *
 * \note Free the returned string with opendal_string_free()
 */
char *opendal_entry_name(const struct opendal_entry *self);

/**
 * \brief Return the metadata associated with this entry.
 *
 * The returned metadata is heap-allocated and must be freed
 * by the caller via opendal_metadata_free().
 */
struct opendal_metadata *opendal_entry_metadata(const struct opendal_entry *self);

/**
 * \brief Frees the heap memory used by the opendal_list_entry
 */
void opendal_entry_free(struct opendal_entry *ptr);

/**
 * \brief Read data from the reader.
 */
struct opendal_result_reader_read opendal_reader_read(struct opendal_reader *self,
                                                      uint8_t *buf,
                                                      uintptr_t len);

/**
 * \brief Seek to an offset, in bytes, in a stream.
 */
struct opendal_result_reader_seek opendal_reader_seek(struct opendal_reader *self,
                                                      int64_t offset,
                                                      int32_t whence);

/**
 * \brief Frees the heap memory used by the opendal_reader.
 */
void opendal_reader_free(struct opendal_reader *ptr);

/**
 * \brief Write data to the writer.
 */
struct opendal_result_writer_write opendal_writer_write(struct opendal_writer *self,
                                                        const struct opendal_bytes *bytes);

/**
 * \brief Close the writer and make sure all data have been stored.
 */
struct opendal_error *opendal_writer_close(struct opendal_writer *ptr);

/**
 * \brief Close the writer and return the written object's metadata.
 *
 * Like `opendal_writer_close`, but on success returns the metadata of the
 * written object (e.g. etag, version, last modified) instead of discarding it.
 *
 * @param ptr The opendal_writer to close
 * @see opendal_result_write
 * @return Returns opendal_result_write. On success the `meta` field holds the
 * metadata and `error` is null; on failure `meta` is null and `error` is set.
 *
 * \note The returned metadata must be freed with opendal_metadata_free().
 */
struct opendal_result_write opendal_writer_close_with_metadata(struct opendal_writer *ptr);

/**
 * \brief Frees the heap memory used by the opendal_writer.
 */
void opendal_writer_free(struct opendal_writer *ptr);

/**
 * \brief Perform one step of the copy operation.
 *
 * One step performs one backend-defined unit of work: typically one chunk for
 * backends that copy in multiple requests, or the entire copy for backends that
 * only support single-request copy.
 *
 * Returns the number of bytes copied in this step (best-effort; may be 0 when
 * the backend advances without a reliable byte delta). When `has_next` is true
 * the caller should call this function again to continue the copy. When
 * `has_next` is false and `error` is null the copy has completed.
 *
 * @see opendal_operator_copier()
 */
struct opendal_result_copier_next opendal_copier_next(struct opendal_copier *self);

/**
 * \brief Abort the pending copy operation.
 *
 * Returns NULL if the abort succeeds, otherwise it contains the error code and
 * error message.
 */
struct opendal_error *opendal_copier_abort(struct opendal_copier *self);

/**
 * \brief Free the heap memory used by the opendal_copier.
 */
void opendal_copier_free(struct opendal_copier *ptr);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _OPENDAL_H */
