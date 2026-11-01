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

#pragma once

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <unordered_map>

namespace opendal {

/**
 * @enum EntryMode
 * @brief The mode of the entry
 */
enum class EntryMode : int {
  FILE = 1,
  DIR = 2,
  UNKNOWN = 0,
};

/**
 * @struct Metadata
 * @brief The metadata of a file or directory
 *
 * Metadata contains all the information related to a specific path.
 * Depending on the context of the requests, the metadata for the same path may
 * vary. For example, two versions of the same path might have different content
 * lengths. Keep in mind that metadata is always tied to the given context and
 * is not a global state.
 */
class Metadata {
 public:
  // Basic file information
  EntryMode type{EntryMode::UNKNOWN};
  std::uint64_t content_length{0};

  // HTTP-style headers
  std::optional<std::string>
      cache_control;  ///< Cache-Control header as defined by RFC 7234
  std::optional<std::string>
      content_disposition;  ///< Content-Disposition header as defined by RFC
                            ///< 6266
  std::optional<std::string>
      content_md5;  ///< Content-MD5 hash (deprecated but widely used)
  std::optional<std::string>
      content_type;  ///< Content-Type header as defined by RFC 9110
  std::optional<std::string>
      content_encoding;  ///< Content-Encoding header as defined by RFC 7231
  std::optional<std::string> etag;  ///< ETag header as defined by RFC 7232
  std::optional<std::chrono::system_clock::time_point>
      last_modified;  ///< Last-Modified timestamp

  // Versioning information
  std::optional<std::string>
      version;                     ///< Version identifier (e.g., S3 version ID)
  std::optional<bool> is_current;  ///< Whether this is the current version
  bool is_deleted{false};  ///< Whether this entry has been marked as deleted

  // Default constructor
  Metadata() = default;

  // Destructor
  ~Metadata() = default;

  // Accessor methods

  /**
   * @brief Get the entry mode (file type)
   * @return EntryMode indicating whether this is a file, directory, or unknown
   */
  EntryMode Mode() const { return type; }

  /**
   * @brief Check if this metadata represents a file
   * @return true if this is a file, false otherwise
   */
  bool IsFile() const { return type == EntryMode::FILE; }

  /**
   * @brief Check if this metadata represents a directory
   * @return true if this is a directory, false otherwise
   */
  bool IsDir() const { return type == EntryMode::DIR; }

  /**
   * @brief Content length in bytes
   * @return Content length (0 if not set)
   */
  std::uint64_t ContentLength() const { return content_length; }

  /**
   * @brief Cache control directive
   * @return Optional cache control string
   */
  const std::optional<std::string>& CacheControl() const {
    return cache_control;
  }

  /**
   * @brief Content disposition
   * @return Optional content disposition string
   */
  const std::optional<std::string>& ContentDisposition() const {
    return content_disposition;
  }

  /**
   * @brief Content MD5 hash
   * @return Optional MD5 hash string
   */
  const std::optional<std::string>& ContentMd5() const {
    return content_md5;
  }

  /**
   * @brief Content type (MIME type)
   * @return Optional content type string
   */
  const std::optional<std::string>& ContentType() const {
    return content_type;
  }

  /**
   * @brief Content encoding
   * @return Optional content encoding string
   */
  const std::optional<std::string>& ContentEncoding() const {
    return content_encoding;
  }

  /**
   * @brief ETag
   * @return Optional ETag string
   */
  const std::optional<std::string>& Etag() const { return etag; }

  /**
   * @brief Last modified timestamp
   * @return Optional timestamp of last modification
   */
  const std::optional<std::chrono::system_clock::time_point>& LastModified()
      const {
    return last_modified;
  }

  /**
   * @brief Version identifier
   * @return Optional version string
   */
  const std::optional<std::string>& Version() const { return version; }

  /**
   * @brief Check if this is the current version
   * @return Optional boolean indicating if this is current (None if versioning
   * not supported)
   */
  const std::optional<bool>& IsCurrent() const { return is_current; }

  /**
   * @brief Check if this entry has been deleted
   * @return true if deleted, false otherwise
   */
  bool IsDeleted() const { return is_deleted; }
};

/**
 * @struct Entry
 * @brief The entry of a file or directory
 */
class Entry {
 public:
  std::string path;
};

/**
 * @struct ReadRange
 * @brief Byte range used by read operations.
 */
struct ReadRange {
  enum class Type : std::uint8_t {
    FULL = 0,
    OFFSET = 1,
    RANGE = 2,
    SUFFIX = 3,
  };

  Type type{Type::FULL};
  std::uint64_t start{0};
  std::uint64_t end{0};

  static ReadRange Full() { return {}; }

  static ReadRange Offset(std::uint64_t offset) {
    return {Type::OFFSET, offset, 0};
  }

  static ReadRange Range(std::uint64_t start, std::uint64_t end) {
    return {Type::RANGE, start, end};
  }

  static ReadRange Suffix(std::uint64_t size) {
    return {Type::SUFFIX, 0, size};
  }
};

/**
 * @struct StatOptions
 * @brief Options for stat operations.
 */
struct StatOptions {
  std::optional<std::string> version;
  std::optional<std::string> if_match;
  std::optional<std::string> if_none_match;
  std::optional<std::chrono::system_clock::time_point> if_modified_since;
  std::optional<std::chrono::system_clock::time_point> if_unmodified_since;
  std::optional<std::string> override_content_type;
  std::optional<std::string> override_cache_control;
  std::optional<std::string> override_content_disposition;
};

/**
 * @struct ReadOptions
 * @brief Options for read operations.
 */
struct ReadOptions {
  ReadRange range;
  std::optional<std::string> version;
  std::optional<std::string> if_match;
  std::optional<std::string> if_none_match;
  std::optional<std::chrono::system_clock::time_point> if_modified_since;
  std::optional<std::chrono::system_clock::time_point> if_unmodified_since;
  std::optional<std::uint64_t> content_length_hint;
  std::size_t concurrent{0};
  std::optional<std::size_t> chunk;
  std::optional<std::size_t> gap;
  std::optional<std::string> override_content_type;
  std::optional<std::string> override_cache_control;
  std::optional<std::string> override_content_disposition;
};

/**
 * @struct ReaderOptions
 * @brief Options for creating readers.
 */
struct ReaderOptions {
  std::optional<std::string> version;
  std::optional<std::string> if_match;
  std::optional<std::string> if_none_match;
  std::optional<std::chrono::system_clock::time_point> if_modified_since;
  std::optional<std::chrono::system_clock::time_point> if_unmodified_since;
  std::optional<std::uint64_t> content_length_hint;
  std::size_t concurrent{0};
  std::optional<std::size_t> chunk;
  std::optional<std::size_t> gap;
  std::size_t prefetch{0};
};

/**
 * @struct WriteOptions
 * @brief Options for write and writer operations.
 */
struct WriteOptions {
  bool append{false};
  std::optional<std::string> cache_control;
  std::optional<std::string> content_type;
  std::optional<std::string> content_disposition;
  std::optional<std::string> content_encoding;
  std::optional<std::unordered_map<std::string, std::string>> user_metadata;
  std::optional<std::string> if_match;
  std::optional<std::string> if_none_match;
  bool if_not_exists{false};
  std::size_t concurrent{0};
  std::optional<std::size_t> chunk;
};

/**
 * @struct CopyOptions
 * @brief Options for copy operations.
 */
struct CopyOptions {
  bool if_not_exists{false};
  std::optional<std::string> if_match;
  std::optional<std::string> source_version;
  std::optional<std::uint64_t> source_content_length_hint;
  std::size_t concurrent{0};
  std::optional<std::size_t> chunk;
};

/**
 * @struct RenameOptions
 * @brief Options for rename operations.
 */
struct RenameOptions {
  bool if_not_exists{false};
};

/**
 * @struct DeleteOptions
 * @brief Options for delete operations.
 */
struct DeleteOptions {
  std::optional<std::string> version;
  bool recursive{false};
};

/**
 * @struct ListOptions
 * @brief Options for list and lister operations.
 */
struct ListOptions {
  std::optional<std::size_t> limit;
  std::optional<std::string> start_after;
  bool recursive{false};
  bool versions{false};
  bool deleted{false};
};

}  // namespace opendal
namespace opendal {
class Capability {
 public:
  bool stat;
  bool stat_with_if_match;
  bool stat_with_if_none_match;
  bool read;
  bool read_with_if_match;
  bool read_with_if_none_match;
  bool read_with_override_cache_control;
  bool read_with_override_content_disposition;
  bool read_with_override_content_type;
  bool write;
  bool write_can_multi;
  bool write_can_empty;
  bool write_can_append;
  bool write_with_content_type;
  bool write_with_content_disposition;
  bool write_with_cache_control;
  size_t write_multi_max_size;
  size_t write_multi_min_size;
  size_t write_total_max_size;
  bool create_dir;
  bool delete_feature;
  bool copy;
  bool rename;
  bool list;
  bool list_with_limit;
  bool list_with_start_after;
  bool list_with_recursive;
  bool presign;
  bool presign_read;
  bool presign_stat;
  bool presign_write;
  bool shared;
};
}
