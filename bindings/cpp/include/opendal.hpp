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

#include <cstdint>
#include <cstring>
#include <iostream>
#include <memory>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#include "data_structure.hpp"
#include "layer.hpp"

namespace opendal {

namespace ffi {
class Operator;
class Reader;
class Lister;
class Writer;
}  // namespace ffi

class Reader;
class Lister;
class Writer;

/**
 * @class Operator
 * @brief Operator is the entry for all public APIs.
 */
class Operator {
 public:
  Operator() noexcept;

  /**
   * @brief Construct a new Operator object
   *
   * @param scheme The scheme of the operator, same as the name of rust doc
   * @param config The configuration of the operator, same as the service doc
   * @param options Optional layers to apply during construction
   */
  Operator(std::string_view scheme,
           const std::unordered_map<std::string, std::string> &config = {},
           std::vector<std::unique_ptr<OperatorOption>> options = {});

  // Disable copy and assign
  Operator(const Operator &) = delete;
  Operator &operator=(const Operator &) = delete;

  // Enable move
  Operator(Operator &&other) noexcept;
  Operator &operator=(Operator &&other) noexcept;

  ~Operator() noexcept;

  /**
   * @brief Check if the operator is available
   *
   * @return true if the operator is available, false otherwise
   */
  bool Available() const;

  /**
   * @brief Read data from the operator
   * @note The operation will make unnecessary copy. So we recommend to use the
   * `GetReader` method.
   *
   * @param path The path of the data
   * @return The data read from the operator
   */
  std::string Read(std::string_view path);

  /**
   * @brief Read data from the operator with extra options
   *
   * @param path The path of the data
   * @param options The read options
   * @return The data read from the operator
   */
  std::string Read(std::string_view path, const ReadOptions &options);

  /**
   * @brief Write data to the operator
   *
   * @param path The path of the data
   * @param data The data to write
   */
  void Write(std::string_view path, std::string_view data);

  /**
   * @brief Write data to the operator with extra options
   *
   * @param path The path of the data
   * @param data The data to write
   * @param options The write options
   */
  void Write(std::string_view path, std::string_view data,
             const WriteOptions &options);

  /**
   * @brief Read data from the operator
   *
   * @param path The path of the data
   * @return The reader of the data
   */
  Reader GetReader(std::string_view path);
  Reader GetReader(std::string_view path, const ReaderOptions &options);
  Writer GetWriter(std::string_view path);
  Writer GetWriter(std::string_view path, const WriteOptions &options);

  /**
   * @brief Check if the path exists
   *
   * @param path The path to check
   * @return true if the path exists, false otherwise
   */
  [[deprecated("Use Exists() instead.")]]
  bool IsExist(std::string_view path);

  /**
   * @brief Check if the path exists
   *
   * @param path The path to check
   * @return true if the path exists, false otherwise
   */
  bool Exists(std::string_view path);

  /**
   * @brief Create a directory
   *
   * @param path The path of the directory
   */
  void CreateDir(std::string_view path);

  /**
   * @brief Copy a file from src to dst.
   *
   * @param src The source path
   * @param dst The destination path
   */
  void Copy(std::string_view src, std::string_view dst);
  void Copy(std::string_view src, std::string_view dst,
            const CopyOptions &options);

  /**
   * @brief Rename a file from src to dst.
   *
   * @param src The source path
   * @param dst The destination path
   */
  void Rename(std::string_view src, std::string_view dst);
  void Rename(std::string_view src, std::string_view dst,
              const RenameOptions &options);

  /**
   * @brief Remove a file or directory
   *
   * @param path The path of the file or directory
   */
  void Remove(std::string_view path);
  void Remove(std::string_view path, const DeleteOptions &options);
  void RemoveAll(const std::vector<std::string> &paths);

  /**
   * @brief Get the metadata of a file or directory
   *
   * @param path The path of the file or directory
   * @return The metadata of the file or directory
   */
  Metadata Stat(std::string_view path);
  Metadata Stat(std::string_view path, const StatOptions &options);

  /**
   * @brief List the entries of a directory
   * @note The returned entries are sorted by name.
   *
   * @param path The path of the directory
   * @return The entries of the directory
   */
  std::vector<Entry> List(std::string_view path);
  std::vector<Entry> List(std::string_view path, const ListOptions &options);

  Lister GetLister(std::string_view path);
  Lister GetLister(std::string_view path, const ListOptions &options);
  Capability Info();

  private:
   void Destroy() noexcept;

   ffi::Operator *operator_{nullptr};
 };

/**
 * @class Reader
 * @brief Reader is designed to read data from the operator.
 * @details It provides basic read and seek operations with a stream-like
 * interface.
 * @code{.cpp}
 * opendal::ReaderStream stream(operator.reader("path"));
 * @endcode
 */
class Reader {
 public:
  Reader(Reader &&other) noexcept;

  ~Reader() noexcept;

  std::streamsize Read(void *s, std::streamsize n);

  std::streamsize ReadAt(void *s, std::streamsize n, uint64_t offset);

  std::streampos Seek(std::streamoff off, std::ios_base::seekdir way);

 private:
  friend class Operator;

  Reader(ffi::Reader *pointer) noexcept;

  void Destroy() noexcept;

  ffi::Reader *reader_{nullptr};
};

/**
 * @class Writer
 * @brief Writer is designed to write a sequence of byte buffers to an object.
 *
 * Call Close to commit the object. OpenDAL selects the service's appropriate
 * implementation, including multipart uploads when supported.
 */
class Writer {
 public:
  Writer(Writer &&other) noexcept;

  ~Writer() noexcept;

  void Write(std::string_view data);

  void Flush();

  void Close();

 private:
  friend class Operator;

  Writer(ffi::Writer *pointer) noexcept;

  void Destroy() noexcept;

  ffi::Writer *writer_{nullptr};
};

/**
 * @class ReaderStream
 * @brief ReaderStream is a stream wrapper of Reader which provides
 * `iostream` interface. It wraps the Reader to provide standard stream
 * operations.
 */
class ReaderStream : public std::istream {
 public:
  class ReaderStreamBuf : public std::streambuf {
   public:
    ReaderStreamBuf(Reader &&reader)
        : reader_(std::move(reader)), buffer_start_pos_(0) {
      setg(buffer_, buffer_, buffer_);
    }

   protected:
    std::streamsize xsgetn(char *s, std::streamsize count) override {
      std::streamsize total_read = 0;

      while (total_read < count) {
        if (gptr() < egptr()) {
          std::streamsize available_bytes = egptr() - gptr();
          std::streamsize to_copy =
              std::min(available_bytes, count - total_read);
          std::memcpy(s + total_read, gptr(), to_copy);
          gbump(to_copy);
          total_read += to_copy;
          continue;
        }

        if (underflow() == traits_type::eof()) break;
      }

      return total_read;
    }

    int_type underflow() override {
      if (gptr() < egptr()) {
        return traits_type::to_int_type(*gptr());
      }

      // Update the buffer start position to current reader position
      buffer_start_pos_ += static_cast<std::streamoff>(egptr() - eback());

      std::streamsize n = reader_.Read(buffer_, sizeof(buffer_));
      if (n <= 0) {
        return traits_type::eof();
      }

      setg(buffer_, buffer_, buffer_ + n);
      return traits_type::to_int_type(*gptr());
    }

    int_type uflow() override {
      int_type result = underflow();
      if (result != traits_type::eof()) {
        gbump(1);
      }
      return result;
    }

    std::streampos seekoff(std::streamoff off, std::ios_base::seekdir dir,
                           std::ios_base::openmode which) override {
      if (dir == std::ios_base::cur && off == 0) {
        // tellg() case - return current position
        return buffer_start_pos_ + static_cast<std::streamoff>(gptr() - eback());
      }

      // Actual seek operation
      std::streampos new_pos = reader_.Seek(off, dir);
      if (new_pos != std::streampos(-1)) {
        buffer_start_pos_ = new_pos;
        setg(buffer_, buffer_, buffer_);
      }
      return new_pos;
    }

    std::streampos seekpos(std::streampos pos,
                           std::ios_base::openmode which) override {
      return seekoff(pos, std::ios_base::beg, which);
    }

   private:
    Reader reader_;
    char buffer_[8192];
    std::streampos buffer_start_pos_;
  };

  ReaderStream(Reader &&reader)
      : std::istream(&buf_), buf_(std::move(reader)) {}

 private:
  ReaderStreamBuf buf_;
};

/**
 * @class Lister
 * @brief Lister is designed to list the entries of a directory.
 * @details It provides next operation to get the next entry. You can also use
 * it like an iterator.
 * @code{.cpp}
 * auto lister = operator.lister("dir/");
 * for (const auto &entry : lister) {
 *   // Do something with entry
 * }
 * @endcode
 */
class Lister {
 public:
  Lister(Lister &&other) noexcept;

  ~Lister() noexcept;

  /**
   * @class ListerIterator
   * @brief ListerIterator is an iterator of Lister.
   * @note It's an undefined behavior to make multiple iterators from one
   * Lister.
   */
  class Iterator {
   public:
    using iterator_category = std::input_iterator_tag;
    using value_type = Entry;
    using difference_type = std::ptrdiff_t;
    using pointer = Entry *;
    using reference = Entry &;

    Iterator(Lister &lister) : lister_{lister} {
      current_entry_ = lister_.Next();
    }

    Entry operator*() { return current_entry_.value(); }

    Iterator &operator++() {
      if (current_entry_) {
        current_entry_ = lister_.Next();
      }
      return *this;
    }

    bool operator!=(const Iterator &other) const {
      return current_entry_ != std::nullopt ||
             other.current_entry_ != std::nullopt;
    }

   protected:
    // Only used for end iterator
    Iterator(Lister &lister, bool /*end*/) noexcept : lister_(lister) {}

   private:
    friend class Lister;

    Lister &lister_;
    std::optional<Entry> current_entry_;
  };

  /**
   * @brief Get the next entry of the lister
   *
   * @return The next entry of the lister
   */
  std::optional<Entry> Next();

  Iterator begin() { return Iterator(*this); }
  Iterator end() { return Iterator(*this, true); }

 private:
  friend class Operator;

  Lister(ffi::Lister *pointer) noexcept;

  void Destroy() noexcept;

  ffi::Lister *lister_{nullptr};
};

}  // namespace opendal
