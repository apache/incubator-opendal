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

#include <ctime>
#include <random>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

#include "framework/test_framework.hpp"

namespace opendal::test {

class OpenDALBasicTest : public ::testing::Test {
 protected:
  opendal::Operator op_;

  std::string scheme;
  std::unordered_map<std::string, std::string> config;

  // random number generator
  std::mt19937 rng;

  void SetUp() override {
    scheme = "memory";
    rng.seed(time(nullptr));

    op_ = opendal::Operator(scheme, config);
    EXPECT_TRUE(op_.Available());
  }
};

// Scenario: OpenDAL Blocking Operations
OPENDAL_TEST_F(OpenDALTest, BasicTest) {
  std::string file_path = "test";
  std::string file_path_copied = "test_copied";
  std::string file_path_renamed = "test_renamed";
  std::string dir_path = "test_dir/";
  std::string_view data = "abc";

  // write
  op_.Write(file_path, data);

  // read
  auto res = op_.Read(file_path);
  EXPECT_EQ(res, data);

  // check existence
  EXPECT_TRUE(op_.Exists(file_path));

  // create directory
  op_.CreateDir(dir_path);
  EXPECT_TRUE(op_.Exists(dir_path));

  // get metadata
  auto metadata = op_.Stat(file_path);
  EXPECT_EQ(metadata.type, opendal::EntryMode::FILE);
  EXPECT_EQ(metadata.content_length, data.size());

  // list entries
  auto list_file_path = dir_path + file_path;
  op_.Write(list_file_path, data);
  auto entries = op_.List(dir_path);
  EXPECT_EQ(entries.size(), 2);
  std::unordered_set<std::string> paths;
  for (const auto &entry : entries) {
    paths.insert(entry.path);
  }
  EXPECT_TRUE(paths.find(dir_path) != paths.end());
  EXPECT_TRUE(paths.find(list_file_path) != paths.end());

  // remove files
  op_.Remove(file_path_renamed);
  op_.Remove(dir_path);
  EXPECT_FALSE(op_.Exists(file_path_renamed));
}

OPENDAL_TEST_F(OpenDALTest, ReaderTest) {
  std::string file_path = "test";
  constexpr int size = 2000;
  std::string data(size, 0);

  for (auto &d : data) {
    d = rng_() % 256;
  }

  // write
  op_.Write(file_path, data);

  // get reader
  auto reader = op_.GetReader(file_path);
  // uint8_t part_data[100];
  std::string part_data(100, 0);
  reader.Seek(200, std::ios::cur);
  reader.Read(part_data.data(), 100);
  EXPECT_EQ(reader.Seek(0, std::ios::cur), 300);
  for (int i = 0; i < 100; ++i) {
    EXPECT_EQ(part_data[i], data[200 + i]);
  }

  std::string positional_data(100, 0);
  auto positional_read_size = reader.ReadAt(positional_data.data(), /*size=*/ 100, /*offset=*/500);
  EXPECT_EQ(positional_read_size, 100);
  EXPECT_EQ(reader.Seek(0, std::ios::cur), 300);
  for (int i = 0; i < 100; ++i) {
    EXPECT_EQ(positional_data[i], data[500 + i]);
  }
  reader.Seek(0, std::ios::beg);

  // reader stream
  opendal::ReaderStream stream(op_.GetReader(file_path));

  auto read_fn = [&](std::size_t to_read, std::streampos expected_tellg) {
    std::vector<char> v(to_read);
    stream.read(v.data(), v.size());
    EXPECT_TRUE(!!stream);
    EXPECT_EQ(stream.tellg(), expected_tellg);
  };

  EXPECT_EQ(stream.tellg(), 0);
  read_fn(10, 10);
  read_fn(15, 25);
  read_fn(15, 40);
  stream.get();
  EXPECT_EQ(stream.tellg(), 41);
  read_fn(1000, 1041);

  stream.seekg(0, std::ios::beg);
  std::string reader_data(std::istreambuf_iterator<char>{stream}, {});
  EXPECT_EQ(reader_data, data);
}

OPENDAL_TEST_F(OpenDALTest, WriterTest) {
  auto writer = op_.GetWriter("writer_test");
  writer.Write("hello ");
  writer.Write("world");
  writer.Flush();
  writer.Close();

  EXPECT_EQ(op_.Read("writer_test"), "hello world");
}

OPENDAL_TEST_F(OpenDALTest, ListerTest) {
  std::string dir_path = "test_dir/";
  op_.CreateDir(dir_path);
  auto test1_path = dir_path + "test1";
  op_.Write(test1_path, "123");
  auto test2_path = dir_path + "test2";
  op_.Write(test2_path, "456");

  auto lister = op_.GetLister("test_dir/");

  std::unordered_set<std::string> paths;
  for (const auto &entry : lister) {
    paths.insert(entry.path);
  }
  EXPECT_EQ(paths.size(), 3);
  EXPECT_TRUE(paths.find(dir_path) != paths.end());
  EXPECT_TRUE(paths.find(test1_path) != paths.end());
  EXPECT_TRUE(paths.find(test2_path) != paths.end());
}

TEST(OpenDALOptionsTest, ReadOptions) {
  opendal::Operator op("memory");
  op.Write("read_options", "0123456789");

  opendal::ReadOptions options;
  options.range = opendal::ReadRange::Range(2, 5);
  EXPECT_EQ(op.Read("read_options", options), "234");

  options.range = opendal::ReadRange::Offset(7);
  EXPECT_EQ(op.Read("read_options", options), "789");

  options.range = opendal::ReadRange::Suffix(2);
  EXPECT_EQ(op.Read("read_options", options), "89");
}

TEST(OpenDALOptionsTest, ReaderOptions) {
  opendal::Operator op("memory");
  op.Write("reader_options", "0123456789");

  opendal::ReaderOptions options;
  options.content_length_hint = 10;
  options.concurrent = 2;
  options.chunk = 4;
  options.prefetch = 1;

  auto reader = op.GetReader("reader_options", options);
  std::string data(4, 0);
  EXPECT_EQ(reader.Read(data.data(), data.size()), 4);
  EXPECT_EQ(data, "0123");
}

TEST(OpenDALOptionsTest, WriteOptions) {
  opendal::Operator op("memory");

  opendal::WriteOptions options;
  options.cache_control = "max-age=60";
  options.content_type = "text/plain";
  options.content_disposition = "inline";
  options.content_encoding = "identity";
  options.user_metadata = std::unordered_map<std::string, std::string>{
      {"owner", "cpp-test"},
  };
  options.if_not_exists = true;
  options.concurrent = 2;
  options.chunk = 1024;

  op.Write("write_options", "hello", options);

  auto metadata = op.Stat("write_options");
  ASSERT_TRUE(metadata.cache_control.has_value());
  EXPECT_EQ(*metadata.cache_control, "max-age=60");
  ASSERT_TRUE(metadata.content_type.has_value());
  EXPECT_EQ(*metadata.content_type, "text/plain");
  ASSERT_TRUE(metadata.content_disposition.has_value());
  EXPECT_EQ(*metadata.content_disposition, "inline");
  ASSERT_TRUE(metadata.content_encoding.has_value());
  EXPECT_EQ(*metadata.content_encoding, "identity");

  EXPECT_THROW(op.Write("write_options", "overwrite", options), std::exception);
}

TEST(OpenDALOptionsTest, WriterOptions) {
  opendal::Operator op("memory");

  opendal::WriteOptions options;
  options.content_type = "application/octet-stream";
  options.chunk = 1024;

  auto writer = op.GetWriter("writer_options", options);
  writer.Write("hello ");
  writer.Write("world");
  writer.Close();

  EXPECT_EQ(op.Read("writer_options"), "hello world");
  auto metadata = op.Stat("writer_options");
  ASSERT_TRUE(metadata.content_type.has_value());
  EXPECT_EQ(*metadata.content_type, "application/octet-stream");
}

TEST(OpenDALOptionsTest, ListOptions) {
  opendal::Operator op("memory");
  op.CreateDir("list_options/");
  op.CreateDir("list_options/nested/");
  op.Write("list_options/nested/file", "hello");

  opendal::ListOptions options;
  options.recursive = true;
  options.limit = 16;

  auto entries = op.List("list_options/", options);
  std::unordered_set<std::string> paths;
  for (const auto &entry : entries) {
    paths.insert(entry.path);
  }
  EXPECT_TRUE(paths.find("list_options/nested/file") != paths.end());

  auto lister = op.GetLister("list_options/", options);
  paths.clear();
  for (const auto &entry : lister) {
    paths.insert(entry.path);
  }
  EXPECT_TRUE(paths.find("list_options/nested/file") != paths.end());
}

TEST(OpenDALOptionsTest, ListOptionsUnsupportedByService) {
  opendal::Operator op("memory");
  op.Write("list_unsupported/file", "hello");

  opendal::ListOptions start_after;
  start_after.start_after = "list_unsupported/file";
  EXPECT_THROW(op.List("list_unsupported/", start_after), std::exception);

  opendal::ListOptions versions;
  versions.versions = true;
  EXPECT_THROW(op.List("list_unsupported/", versions), std::exception);

  opendal::ListOptions deleted;
  deleted.deleted = true;
  EXPECT_THROW(op.List("list_unsupported/", deleted), std::exception);
}

TEST(OpenDALOptionsTest, DeleteOptions) {
  opendal::Operator op("memory");
  op.CreateDir("delete_options/");
  op.Write("delete_options/file", "hello");

  opendal::DeleteOptions options;
  options.recursive = true;
  op.Remove("delete_options/", options);

  EXPECT_FALSE(op.Exists("delete_options/file"));
}

TEST(OpenDALOptionsTest, RemoveAll) {
  opendal::Operator op("memory");
  std::vector<std::string> paths;
  for (int index = 0; index < 8; ++index) {
    auto path = "remove_all/file_" + std::to_string(index);
    op.Write(path, "hello");
    EXPECT_TRUE(op.Exists(path));
    paths.push_back(path);
  }

  op.RemoveAll(paths);

  for (const auto &path : paths) {
    EXPECT_FALSE(op.Exists(path));
  }
}

TEST(OpenDALOptionsTest, StatOptions) {
  opendal::Operator op("memory");
  op.Write("stat_options", "hello");

  opendal::StatOptions stat_options;
  auto metadata = op.Stat("stat_options", stat_options);
  EXPECT_EQ(metadata.content_length, 5);
}

} // namespace opendal::test
