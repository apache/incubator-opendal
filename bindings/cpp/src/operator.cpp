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

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <ctime>

#include "lib.rs.h"
#include "opendal.hpp"
#include "utils/ffi_converter.hpp"
#include "utils/rust_converter.hpp"

namespace opendal {

namespace {

ffi::OptionalString ToFfiOptionalString(
    const std::optional<std::string> &value) {
  if (value.has_value()) {
    return ffi::OptionalString{true, utils::rust_string(*value)};
  }
  return ffi::OptionalString{false, rust::String()};
}

ffi::OptionalU64 ToFfiOptionalU64(const std::optional<std::uint64_t> &value) {
  if (value.has_value()) {
    return ffi::OptionalU64{true, *value};
  }
  return ffi::OptionalU64{false, 0};
}

ffi::OptionalUsize ToFfiOptionalUsize(const std::optional<std::size_t> &value) {
  if (value.has_value()) {
    return ffi::OptionalUsize{true, *value};
  }
  return ffi::OptionalUsize{false, 0};
}

ffi::OptionalTimestamp ToFfiOptionalTimestamp(
    const std::optional<std::chrono::system_clock::time_point> &value) {
  if (!value.has_value()) {
    return ffi::OptionalTimestamp{false, 0, 0};
  }

  auto duration = value->time_since_epoch();
  auto seconds_duration =
      std::chrono::duration_cast<std::chrono::seconds>(duration);
  auto nanoseconds = std::chrono::duration_cast<std::chrono::nanoseconds>(
                         duration - seconds_duration)
                         .count();
  auto seconds = seconds_duration.count();
  if (nanoseconds < 0) {
    --seconds;
    nanoseconds += 1000000000;
  }

  return ffi::OptionalTimestamp{true, static_cast<std::int64_t>(seconds),
                                static_cast<std::uint32_t>(nanoseconds)};
}

ffi::FfiBytesRange ToFfiBytesRange(const ReadRange &range) {
  return ffi::FfiBytesRange{static_cast<std::uint8_t>(range.type), range.start,
                            range.end};
}

rust::Vec<ffi::HashMapValue> ToFfiHashMap(
    const std::unordered_map<std::string, std::string> &map) {
  rust::Vec<ffi::HashMapValue> values;
  values.reserve(map.size());
  for (const auto &[key, value] : map) {
    values.push_back({utils::rust_string(key), utils::rust_string(value)});
  }
  return values;
}

ffi::FfiStatOptions ToFfiOptions(const StatOptions &options) {
  return ffi::FfiStatOptions{
      ToFfiOptionalString(options.version),
      ToFfiOptionalString(options.if_match),
      ToFfiOptionalString(options.if_none_match),
      ToFfiOptionalTimestamp(options.if_modified_since),
      ToFfiOptionalTimestamp(options.if_unmodified_since),
      ToFfiOptionalString(options.override_content_type),
      ToFfiOptionalString(options.override_cache_control),
      ToFfiOptionalString(options.override_content_disposition),
  };
}

ffi::FfiReadOptions ToFfiOptions(const ReadOptions &options) {
  return ffi::FfiReadOptions{
      ToFfiBytesRange(options.range),
      ToFfiOptionalString(options.version),
      ToFfiOptionalString(options.if_match),
      ToFfiOptionalString(options.if_none_match),
      ToFfiOptionalTimestamp(options.if_modified_since),
      ToFfiOptionalTimestamp(options.if_unmodified_since),
      ToFfiOptionalU64(options.content_length_hint),
      options.concurrent,
      ToFfiOptionalUsize(options.chunk),
      ToFfiOptionalUsize(options.gap),
      ToFfiOptionalString(options.override_content_type),
      ToFfiOptionalString(options.override_cache_control),
      ToFfiOptionalString(options.override_content_disposition),
  };
}

ffi::FfiReaderOptions ToFfiOptions(const ReaderOptions &options) {
  return ffi::FfiReaderOptions{
      ToFfiOptionalString(options.version),
      ToFfiOptionalString(options.if_match),
      ToFfiOptionalString(options.if_none_match),
      ToFfiOptionalTimestamp(options.if_modified_since),
      ToFfiOptionalTimestamp(options.if_unmodified_since),
      ToFfiOptionalU64(options.content_length_hint),
      options.concurrent,
      ToFfiOptionalUsize(options.chunk),
      ToFfiOptionalUsize(options.gap),
      options.prefetch,
  };
}

ffi::FfiWriteOptions ToFfiOptions(const WriteOptions &options) {
  auto metadata = options.user_metadata.has_value()
                      ? ToFfiHashMap(*options.user_metadata)
                      : rust::Vec<ffi::HashMapValue>();
  return ffi::FfiWriteOptions{
      options.append,
      ToFfiOptionalString(options.cache_control),
      ToFfiOptionalString(options.content_type),
      ToFfiOptionalString(options.content_disposition),
      ToFfiOptionalString(options.content_encoding),
      options.user_metadata.has_value(),
      std::move(metadata),
      ToFfiOptionalString(options.if_match),
      ToFfiOptionalString(options.if_none_match),
      options.if_not_exists,
      options.concurrent,
      ToFfiOptionalUsize(options.chunk),
  };
}

ffi::FfiCopyOptions ToFfiOptions(const CopyOptions &options) {
  return ffi::FfiCopyOptions{
      options.if_not_exists,
      ToFfiOptionalString(options.if_match),
      ToFfiOptionalString(options.source_version),
      ToFfiOptionalU64(options.source_content_length_hint),
      options.concurrent,
      ToFfiOptionalUsize(options.chunk),
  };
}

ffi::FfiRenameOptions ToFfiOptions(const RenameOptions &options) {
  return ffi::FfiRenameOptions{options.if_not_exists};
}

ffi::FfiDeleteOptions ToFfiOptions(const DeleteOptions &options) {
  return ffi::FfiDeleteOptions{ToFfiOptionalString(options.version),
                               options.recursive};
}

ffi::FfiListOptions ToFfiOptions(const ListOptions &options) {
  return ffi::FfiListOptions{
      ToFfiOptionalUsize(options.limit),
      ToFfiOptionalString(options.start_after),
      options.recursive,
      options.versions,
      options.deleted,
  };
}

}  // namespace

std::optional<std::string> parse_optional_string(ffi::OptionalString &&s) {
  if (s.has_value) {
    return std::string(std::move(s.value));
  } else {
    return std::nullopt;
  }
}

std::optional<bool> parse_optional_bool(ffi::OptionalBool &&b) {
  if (b.has_value) {
    return b.value;
  } else {
    return std::nullopt;
  }
}

Metadata parse_meta_data(ffi::Metadata &&meta) {
  Metadata metadata;

  // Basic information
  metadata.type = static_cast<EntryMode>(meta.mode);
  metadata.content_length = meta.content_length;

  // HTTP-style headers
  metadata.cache_control = parse_optional_string(std::move(meta.cache_control));
  metadata.content_disposition =
      parse_optional_string(std::move(meta.content_disposition));
  metadata.content_md5 = parse_optional_string(std::move(meta.content_md5));
  metadata.content_type = parse_optional_string(std::move(meta.content_type));
  metadata.content_encoding =
      parse_optional_string(std::move(meta.content_encoding));
  metadata.etag = parse_optional_string(std::move(meta.etag));

  // Versioning information
  metadata.version = parse_optional_string(std::move(meta.version));
  metadata.is_current = parse_optional_bool(std::move(meta.is_current));
  metadata.is_deleted = meta.is_deleted;

  // Parse last_modified timestamp
  auto last_modified_str = parse_optional_string(std::move(meta.last_modified));
  if (last_modified_str.has_value()) {
    // Parse ISO 8601 string to time_point using strptime to avoid locale lock
    std::tm tm = {};
    const char *str = last_modified_str.value().c_str();

    // Parse ISO 8601 format: YYYY-MM-DDTHH:MM:SS
    int year, month, day, hour, minute, second;
    if (sscanf(str, "%d-%d-%dT%d:%d:%d", &year, &month, &day, &hour, &minute,
               &second) == 6) {
      tm.tm_year = year - 1900;  // years since 1900
      tm.tm_mon = month - 1;     // months since January (0-11)
      tm.tm_mday = day;
      tm.tm_hour = hour;
      tm.tm_min = minute;
      tm.tm_sec = second;
      tm.tm_isdst = -1;  // let mktime determine DST

      std::time_t time_t_value = std::mktime(&tm);
      if (time_t_value != -1) {
        metadata.last_modified =
            std::chrono::system_clock::from_time_t(time_t_value);
      }
    }
  }

  return metadata;
}

Operator::Operator() noexcept = default;

void Operator::Destroy() noexcept {
  if (operator_) {
    ffi::delete_operator(operator_);
    operator_ = nullptr;
  }
}

Operator::Operator(std::string_view scheme,
                   const std::unordered_map<std::string, std::string> &config,
                   std::vector<std::unique_ptr<OperatorOption>> options) {
  auto rust_map = rust::Vec<ffi::HashMapValue>();
  rust_map.reserve(config.size());

  for (auto &[k, v] : config) {
    rust_map.push_back({utils::rust_string(k), utils::rust_string(v)});
  }

  auto layers = ffi::layer_builder_new();
  FfiLayerBuilderMutator mutator(*layers);
  for (const auto &option : options) {
    if (option != nullptr) {
      option->ApplyTo(mutator);
    }
  }

  operator_ = ffi::new_operator(utils::rust_str(scheme), rust_map, *layers);
}

Operator::~Operator() noexcept { Destroy(); }

Operator::Operator(Operator &&other) noexcept : operator_(other.operator_) {
  other.operator_ = nullptr;
}

Operator &Operator::operator=(Operator &&other) noexcept {
  if (this != &other) {
    Destroy();

    operator_ = other.operator_;
    other.operator_ = nullptr;
  }

  return *this;
}

bool Operator::Available() const { return operator_ != nullptr; }

// We can't avoid copy, because std::vector hides the internal structure.
// std::vector doesn't support init from a pointer without copy.
std::string Operator::Read(std::string_view path) {
  auto rust_vec = operator_->read(utils::rust_str(path));
  return {rust_vec.begin(), rust_vec.end()};
}

std::string Operator::Read(std::string_view path, const ReadOptions &options) {
  auto rust_vec =
      operator_->read_options(utils::rust_str(path), ToFfiOptions(options));
  return {rust_vec.begin(), rust_vec.end()};
}

void Operator::Write(std::string_view path, std::string_view data) {
  operator_->write(utils::rust_str(path),
                   utils::rust_slice<const uint8_t>(data));
}

void Operator::Write(std::string_view path, std::string_view data,
                     const WriteOptions &options) {
  operator_->write_options(utils::rust_str(path),
                           utils::rust_slice<const uint8_t>(data),
                           ToFfiOptions(options));
}

bool Operator::Exists(std::string_view path) {
  return operator_->exists(utils::rust_str(path));
}

bool Operator::IsExist(std::string_view path) { return Exists(path); }

void Operator::CreateDir(std::string_view path) {
  operator_->create_dir(utils::rust_str(path));
}

void Operator::Copy(std::string_view src, std::string_view dst) {
  operator_->copy(utils::rust_str(src), utils::rust_str(dst));
}

void Operator::Copy(std::string_view src, std::string_view dst,
                    const CopyOptions &options) {
  operator_->copy_options(utils::rust_str(src), utils::rust_str(dst),
                          ToFfiOptions(options));
}

void Operator::Rename(std::string_view src, std::string_view dst) {
  operator_->rename(utils::rust_str(src), utils::rust_str(dst));
}

void Operator::Rename(std::string_view src, std::string_view dst,
                      const RenameOptions &options) {
  operator_->rename_options(utils::rust_str(src), utils::rust_str(dst),
                            ToFfiOptions(options));
}

void Operator::Remove(std::string_view path) {
  operator_->remove(utils::rust_str(path));
}

void Operator::Remove(std::string_view path, const DeleteOptions &options) {
  operator_->remove_options(utils::rust_str(path), ToFfiOptions(options));
}

void Operator::RemoveAll(const std::vector<std::string> &paths) {
  rust::Vec<rust::String> rust_paths;
  rust_paths.reserve(paths.size());
  for (const auto &path : paths) {
    rust_paths.push_back(utils::rust_string(path));
  }
  operator_->remove_all(std::move(rust_paths));
}

Metadata Operator::Stat(std::string_view path) {
  return parse_meta_data(operator_->stat(utils::rust_str(path)));
}

Metadata Operator::Stat(std::string_view path, const StatOptions &options) {
  return parse_meta_data(
      operator_->stat_options(utils::rust_str(path), ToFfiOptions(options)));
}

std::vector<Entry> Operator::List(std::string_view path) {
  auto rust_vec = operator_->list(utils::rust_str(path));

  std::vector<Entry> entries;
  entries.reserve(rust_vec.size());
  for (auto &&entry : rust_vec) {
    entries.emplace_back(utils::parse_entry(std::move(entry)));
  }

  return entries;
}

std::vector<Entry> Operator::List(std::string_view path,
                                  const ListOptions &options) {
  auto rust_vec =
      operator_->list_options(utils::rust_str(path), ToFfiOptions(options));

  std::vector<Entry> entries;
  entries.reserve(rust_vec.size());
  for (auto &&entry : rust_vec) {
    entries.emplace_back(utils::parse_entry(std::move(entry)));
  }

  return entries;
}

Lister Operator::GetLister(std::string_view path) {
  return operator_->lister(utils::rust_str(path));
}

Lister Operator::GetLister(std::string_view path, const ListOptions &options) {
  return operator_->lister_options(utils::rust_str(path),
                                   ToFfiOptions(options));
}

Reader Operator::GetReader(std::string_view path) {
  return operator_->reader(utils::rust_str(path));
}

Reader Operator::GetReader(std::string_view path,
                           const ReaderOptions &options) {
  return operator_->reader_options(utils::rust_str(path),
                                   ToFfiOptions(options));
}

Writer Operator::GetWriter(std::string_view path) {
  return operator_->writer(utils::rust_str(path));
}

Writer Operator::GetWriter(std::string_view path, const WriteOptions &options) {
  return operator_->writer_options(utils::rust_str(path),
                                   ToFfiOptions(options));
}

}  // namespace opendal
opendal::Capability opendal::Operator::Info() {
  auto op_info = operator_->info();
  return Capability{
      .stat = op_info.stat,
      .stat_with_if_match = op_info.stat_with_if_match,
      .stat_with_if_none_match = op_info.stat_with_if_none_match,
      .read = op_info.read,
      .read_with_if_match = op_info.read_with_if_match,
      .read_with_if_none_match = op_info.read_with_if_none_match,
      .read_with_override_cache_control = op_info.read_with_override_cache_control,
      .read_with_override_content_disposition =
          op_info.read_with_override_content_disposition,
      .read_with_override_content_type = op_info.read_with_override_content_type,
      .write = op_info.write,
      .write_can_multi = op_info.write_can_multi,
      .write_can_empty = op_info.write_can_empty,
      .write_can_append = op_info.write_can_append,
      .write_with_content_type = op_info.write_with_content_type,
      .write_with_content_disposition = op_info.write_with_content_disposition,
      .write_with_cache_control = op_info.write_with_cache_control,
      .write_multi_max_size = op_info.write_multi_max_size,
      .write_multi_min_size = op_info.write_multi_min_size,
      .write_total_max_size = op_info.write_total_max_size,
      .create_dir = op_info.create_dir,
      .delete_feature = op_info.delete_feature,
      .copy = op_info.copy,
      .rename = op_info.rename,
      .list = op_info.list,
      .list_with_limit = op_info.list_with_limit,
      .list_with_start_after = op_info.list_with_start_after,
      .list_with_recursive = op_info.list_with_recursive,
      .presign = op_info.presign,
      .presign_read = op_info.presign_read,
      .presign_stat = op_info.presign_stat,
      .presign_write = op_info.presign_write,
      .shared = op_info.shared,
  };
}
