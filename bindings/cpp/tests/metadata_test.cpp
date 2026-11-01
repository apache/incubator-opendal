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
#include <string>
#include <unordered_map>

#include "gtest/gtest.h"
#include "opendal.hpp"

class MetadataTest : public ::testing::Test {
 protected:
  opendal::Operator op;
  std::string scheme = "memory";
  std::unordered_map<std::string, std::string> config;

  void SetUp() override {
    op = opendal::Operator(scheme, config);
    EXPECT_TRUE(op.Available());
  }

  // Helper methods for BDD-style testing
  void given_a_file_exists_with_content(const std::string& path,
                                        const std::string& content) {
    op.Write(path, content);
    EXPECT_TRUE(op.Exists(path));
  }

  void given_a_directory_exists(const std::string& path) {
    op.CreateDir(path);
    EXPECT_TRUE(op.Exists(path));
  }

  opendal::Metadata when_i_get_metadata_for(const std::string& path) {
    return op.Stat(path);
  }

  void then_metadata_should_indicate_file_type(
      const opendal::Metadata& metadata) {
    EXPECT_TRUE(metadata.IsFile());
    EXPECT_FALSE(metadata.IsDir());
    EXPECT_EQ(metadata.Mode(), opendal::EntryMode::FILE);
    EXPECT_EQ(metadata.type, opendal::EntryMode::FILE);
  }

  void then_metadata_should_indicate_directory_type(
      const opendal::Metadata& metadata) {
    EXPECT_FALSE(metadata.IsFile());
    EXPECT_TRUE(metadata.IsDir());
    EXPECT_EQ(metadata.Mode(), opendal::EntryMode::DIR);
    EXPECT_EQ(metadata.type, opendal::EntryMode::DIR);
  }

  void then_content_length_should_be(const opendal::Metadata& metadata,
                                     std::uint64_t expected_length) {
    EXPECT_EQ(metadata.content_length, expected_length);
    EXPECT_EQ(metadata.ContentLength(), expected_length);
  }

  void then_metadata_should_not_be_deleted(const opendal::Metadata& metadata) {
    EXPECT_FALSE(metadata.is_deleted);
    EXPECT_FALSE(metadata.IsDeleted());
  }
};

// Feature: File Metadata Behavior
// As a developer using OpenDAL C++ bindings
// I want to retrieve accurate metadata for files
// So that I can understand file properties and make informed decisions

TEST_F(MetadataTest, BasicFileProperties) {
  // Scenario: Getting metadata for a regular file

  // Given a file exists with specific content
  const std::string file_path = "test_file.txt";
  const std::string file_content = "Hello, OpenDAL C++ bindings!";
  given_a_file_exists_with_content(file_path, file_content);

  // When I get metadata for the file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then the metadata should indicate it's a file
  then_metadata_should_indicate_file_type(metadata);

  // And the content length should match the file content
  then_content_length_should_be(metadata, file_content.length());

  // And the file should not be marked as deleted
  then_metadata_should_not_be_deleted(metadata);
}

// Feature: Directory Metadata Behavior
// As a developer using OpenDAL C++ bindings
// I want to retrieve accurate metadata for directories
// So that I can distinguish between files and directories

TEST_F(MetadataTest, BasicDirectoryProperties) {
  // Scenario: Getting metadata for a directory

  // Given a directory exists
  const std::string dir_path = "test_directory/";
  given_a_directory_exists(dir_path);

  // When I get metadata for the directory
  auto metadata = when_i_get_metadata_for(dir_path);

  // Then the metadata should indicate it's a directory
  then_metadata_should_indicate_directory_type(metadata);

  // And the directory should not be marked as deleted
  then_metadata_should_not_be_deleted(metadata);
}

TEST_F(MetadataTest, EmptyFile) {
  // Scenario: Getting metadata for an empty file

  // Given an empty file exists
  const std::string file_path = "empty_file.txt";
  const std::string empty_content = "";
  given_a_file_exists_with_content(file_path, empty_content);

  // When I get metadata for the empty file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then the metadata should indicate it's a file
  then_metadata_should_indicate_file_type(metadata);

  // And the content length should be zero
  then_content_length_should_be(metadata, 0);

  // And the file should not be marked as deleted
  then_metadata_should_not_be_deleted(metadata);
}

TEST_F(MetadataTest, LargeFile) {
  // Scenario: Getting metadata for a large file

  // Given a large file exists
  const std::string file_path = "large_file.bin";
  const std::size_t large_size = 1024 * 1024;  // 1MB
  const std::string large_content(large_size, 'X');
  given_a_file_exists_with_content(file_path, large_content);

  // When I get metadata for the large file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then the metadata should indicate it's a file
  then_metadata_should_indicate_file_type(metadata);

  // And the content length should match the large size
  then_content_length_should_be(metadata, large_size);

  // And the file should not be marked as deleted
  then_metadata_should_not_be_deleted(metadata);
}

TEST_F(MetadataTest, NestedDirectories) {
  // Scenario: Getting metadata for nested directories

  // Given nested directories exist
  const std::string parent_dir = "parent/";
  const std::string nested_dir = "parent/child/";
  given_a_directory_exists(parent_dir);
  given_a_directory_exists(nested_dir);

  // When I get metadata for the parent directory
  auto parent_metadata = when_i_get_metadata_for(parent_dir);

  // And I get metadata for the nested directory
  auto nested_metadata = when_i_get_metadata_for(nested_dir);

  // Then both should indicate they are directories
  then_metadata_should_indicate_directory_type(parent_metadata);
  then_metadata_should_indicate_directory_type(nested_metadata);

  // And neither should be marked as deleted
  then_metadata_should_not_be_deleted(parent_metadata);
  then_metadata_should_not_be_deleted(nested_metadata);
}

// Feature: HTTP Headers in Metadata
// As a developer using OpenDAL C++ bindings
// I want to access HTTP-style headers in metadata
// So that I can understand content properties and caching behavior

TEST_F(MetadataTest, OptionalFieldsAccessibility) {
  // Scenario: Accessing optional HTTP header fields

  // Given a file exists
  const std::string file_path = "test_headers.txt";
  const std::string content = "test content for headers";
  given_a_file_exists_with_content(file_path, content);

  // When I get metadata for the file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then I should be able to access all optional header fields without crashes
  EXPECT_NO_THROW(metadata.CacheControl());
  EXPECT_NO_THROW(metadata.ContentDisposition());
  EXPECT_NO_THROW(metadata.ContentMd5());
  EXPECT_NO_THROW(metadata.ContentType());
  EXPECT_NO_THROW(metadata.ContentEncoding());
  EXPECT_NO_THROW(metadata.Etag());
  EXPECT_NO_THROW(metadata.LastModified());
}

// Feature: Versioning Information in Metadata
// As a developer using OpenDAL C++ bindings
// I want to access versioning information in metadata
// So that I can work with versioned storage systems

TEST_F(MetadataTest, VersioningFields) {
  // Scenario: Accessing versioning information

  // Given a file exists
  const std::string file_path = "versioned_file.txt";
  const std::string content = "versioned content";
  given_a_file_exists_with_content(file_path, content);

  // When I get metadata for the file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then I should be able to access versioning fields
  EXPECT_NO_THROW(metadata.Version());
  EXPECT_NO_THROW(metadata.IsCurrent());
  EXPECT_NO_THROW(metadata.IsDeleted());

  // And for memory storage, file should not be deleted by default
  EXPECT_FALSE(metadata.IsDeleted());
}

// Feature: Metadata Consistency
// As a developer using OpenDAL C++ bindings
// I want consistent metadata behavior
// So that I can rely on the API behavior

TEST_F(MetadataTest, AccessorConsistency) {
  // Scenario: Ensuring consistency between direct field access and accessor
  // methods

  // Given a file exists
  const std::string file_path = "consistency_test.txt";
  const std::string content = "test consistency";
  given_a_file_exists_with_content(file_path, content);

  // When I get metadata for the file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then direct field access should match accessor methods
  EXPECT_EQ(metadata.type, metadata.Mode());
  EXPECT_EQ(metadata.content_length, metadata.ContentLength());
  EXPECT_EQ(metadata.is_deleted, metadata.IsDeleted());

  // And accessor methods should provide consistent boolean results
  if (metadata.type == opendal::EntryMode::FILE) {
    EXPECT_TRUE(metadata.IsFile());
    EXPECT_FALSE(metadata.IsDir());
  } else if (metadata.type == opendal::EntryMode::DIR) {
    EXPECT_FALSE(metadata.IsFile());
    EXPECT_TRUE(metadata.IsDir());
  }
}

// Feature: Metadata Object Lifecycle
// As a developer using OpenDAL C++ bindings
// I want metadata objects to behave correctly throughout their lifecycle
// So that I can use them safely in my applications

TEST_F(MetadataTest, CopyAndMove) {
  // Scenario: Copying and moving metadata objects

  // Given a file exists
  const std::string file_path = "lifecycle_test.txt";
  const std::string content = "lifecycle test content";
  given_a_file_exists_with_content(file_path, content);

  // When I get metadata for the file
  auto original_metadata = when_i_get_metadata_for(file_path);

  // And I copy the metadata
  auto copied_metadata = original_metadata;

  // And I move the metadata
  auto moved_metadata = std::move(original_metadata);

  // Then the copied metadata should be identical to the moved one
  EXPECT_EQ(copied_metadata.type, moved_metadata.type);
  EXPECT_EQ(copied_metadata.content_length, moved_metadata.content_length);
  EXPECT_EQ(copied_metadata.is_deleted, moved_metadata.is_deleted);

  // And both should function correctly
  EXPECT_EQ(copied_metadata.IsFile(), moved_metadata.IsFile());
  EXPECT_EQ(copied_metadata.ContentLength(),
            moved_metadata.ContentLength());
}

TEST_F(MetadataTest, DefaultConstruction) {
  // Scenario: Default construction of metadata objects

  // When I create a default metadata object
  opendal::Metadata default_metadata;

  // Then it should have sensible defaults
  EXPECT_EQ(default_metadata.content_length, 0);
  EXPECT_FALSE(default_metadata.is_deleted);
  EXPECT_EQ(default_metadata.ContentLength(), 0);
  EXPECT_FALSE(default_metadata.IsDeleted());

  // And accessor methods should work without crashing
  EXPECT_NO_THROW(default_metadata.Mode());
  EXPECT_NO_THROW(default_metadata.IsFile());
  EXPECT_NO_THROW(default_metadata.IsDir());
}

// Feature: Multiple Files Metadata Comparison
// As a developer using OpenDAL C++ bindings
// I want to compare metadata from different files
// So that I can make decisions based on file properties

TEST_F(MetadataTest, DifferentFileSizes) {
  // Scenario: Comparing metadata from files of different sizes

  // Given files of different sizes exist
  const std::string small_file = "small.txt";
  const std::string large_file = "large.txt";
  const std::string small_content = "small";
  const std::string large_content(1000, 'L');

  given_a_file_exists_with_content(small_file, small_content);
  given_a_file_exists_with_content(large_file, large_content);

  // When I get metadata for both files
  auto small_metadata = when_i_get_metadata_for(small_file);
  auto large_metadata = when_i_get_metadata_for(large_file);

  // Then both should be files
  then_metadata_should_indicate_file_type(small_metadata);
  then_metadata_should_indicate_file_type(large_metadata);

  // And their sizes should differ appropriately
  EXPECT_LT(small_metadata.ContentLength(),
            large_metadata.ContentLength());
  EXPECT_EQ(small_metadata.ContentLength(), small_content.length());
  EXPECT_EQ(large_metadata.ContentLength(), large_content.length());
}

TEST_F(MetadataTest, FileVsDirectory) {
  // Scenario: Comparing metadata between file and directory

  // Given a file and directory exist
  const std::string file_path = "comparison_file.txt";
  const std::string dir_path = "comparison_dir/";
  const std::string content = "file content";

  given_a_file_exists_with_content(file_path, content);
  given_a_directory_exists(dir_path);

  // When I get metadata for both
  auto file_metadata = when_i_get_metadata_for(file_path);
  auto dir_metadata = when_i_get_metadata_for(dir_path);

  // Then they should have different types
  EXPECT_NE(file_metadata.type, dir_metadata.type);
  EXPECT_TRUE(file_metadata.IsFile());
  EXPECT_TRUE(dir_metadata.IsDir());
  EXPECT_FALSE(file_metadata.IsDir());
  EXPECT_FALSE(dir_metadata.IsFile());

  // And both should not be deleted
  then_metadata_should_not_be_deleted(file_metadata);
  then_metadata_should_not_be_deleted(dir_metadata);
}

// Feature: Timestamp Behavior in Metadata
// As a developer using OpenDAL C++ bindings
// I want to access and understand timestamp information
// So that I can track file modification times

TEST_F(MetadataTest, LastModifiedAccess) {
  // Scenario: Accessing last modified timestamp

  // Given a file exists
  const std::string file_path = "timestamp_test.txt";
  const std::string content = "timestamp test content";
  given_a_file_exists_with_content(file_path, content);

  // When I get metadata for the file
  auto metadata = when_i_get_metadata_for(file_path);

  // Then I should be able to access the last modified timestamp
  EXPECT_NO_THROW(metadata.LastModified());

  // Note: For memory storage, last_modified might not be set
  // This is expected behavior and not an error
}

TEST_F(MetadataTest, TimestampConsistency) {
  // Scenario: Timestamp consistency across multiple file operations

  // Given multiple files are created
  const std::string file1 = "timestamp1.txt";
  const std::string file2 = "timestamp2.txt";
  const std::string content1 = "first file";
  const std::string content2 = "second file";

  given_a_file_exists_with_content(file1, content1);
  given_a_file_exists_with_content(file2, content2);

  // When I get metadata for both files
  auto metadata1 = when_i_get_metadata_for(file1);
  auto metadata2 = when_i_get_metadata_for(file2);

  // Then both should handle timestamps consistently
  EXPECT_NO_THROW(metadata1.LastModified());
  EXPECT_NO_THROW(metadata2.LastModified());

  // And the timestamp handling should be the same type
  auto ts1 = metadata1.LastModified();
  auto ts2 = metadata2.LastModified();

  // Both should have the same availability (either both have timestamps or both
  // don't)
  EXPECT_EQ(ts1.has_value(), ts2.has_value());
}

// Feature: Error Handling and Edge Cases
// As a developer using OpenDAL C++ bindings
// I want the metadata API to handle edge cases gracefully
// So that my application doesn't crash on unexpected inputs

TEST_F(MetadataTest, UnknownEntryMode) {
  // Scenario: Handling unknown entry modes gracefully

  // Given a metadata object with unknown mode (using default constructor)
  opendal::Metadata unknown_metadata;
  unknown_metadata.type = opendal::EntryMode::UNKNOWN;

  // When I check the type using accessor methods
  auto mode = unknown_metadata.Mode();
  auto is_file = unknown_metadata.IsFile();
  auto is_dir = unknown_metadata.IsDir();

  // Then it should handle unknown type without crashing
  EXPECT_EQ(mode, opendal::EntryMode::UNKNOWN);
  EXPECT_FALSE(is_file);  // Unknown is not a file
  EXPECT_FALSE(is_dir);   // Unknown is not a directory
}

TEST_F(MetadataTest, EmptyOptionalFields) {
  // Scenario: Accessing optional fields when they're empty

  // Given a default metadata object (which should have empty optional fields)
  opendal::Metadata empty_metadata;

  // When I access all optional fields
  auto cache_control = empty_metadata.CacheControl();
  auto content_disposition = empty_metadata.ContentDisposition();
  auto content_md5 = empty_metadata.ContentMd5();
  auto content_type = empty_metadata.ContentType();
  auto content_encoding = empty_metadata.ContentEncoding();
  auto etag = empty_metadata.Etag();
  auto version = empty_metadata.Version();
  auto is_current = empty_metadata.IsCurrent();
  auto last_modified = empty_metadata.LastModified();

  // Then all optional fields should be empty (no value)
  EXPECT_FALSE(cache_control.has_value());
  EXPECT_FALSE(content_disposition.has_value());
  EXPECT_FALSE(content_md5.has_value());
  EXPECT_FALSE(content_type.has_value());
  EXPECT_FALSE(content_encoding.has_value());
  EXPECT_FALSE(etag.has_value());
  EXPECT_FALSE(version.has_value());
  EXPECT_FALSE(is_current.has_value());
  EXPECT_FALSE(last_modified.has_value());

  // And accessing them should not crash
  EXPECT_NO_THROW(empty_metadata.CacheControl());
  EXPECT_NO_THROW(empty_metadata.ContentDisposition());
  EXPECT_NO_THROW(empty_metadata.ContentMd5());
  EXPECT_NO_THROW(empty_metadata.ContentType());
  EXPECT_NO_THROW(empty_metadata.ContentEncoding());
  EXPECT_NO_THROW(empty_metadata.Etag());
  EXPECT_NO_THROW(empty_metadata.Version());
  EXPECT_NO_THROW(empty_metadata.IsCurrent());
  EXPECT_NO_THROW(empty_metadata.LastModified());
}

TEST_F(MetadataTest, LongFilenames) {
  // Scenario: Handling metadata for files with very long names

  // Given a file with a very long name exists
  const std::string long_filename = std::string(200, 'a') + ".txt";
  const std::string content = "content for long filename";
  given_a_file_exists_with_content(long_filename, content);

  // When I get metadata for the file with long name
  auto metadata = when_i_get_metadata_for(long_filename);

  // Then the metadata should be retrieved successfully
  then_metadata_should_indicate_file_type(metadata);
  then_content_length_should_be(metadata, content.length());
  then_metadata_should_not_be_deleted(metadata);
}

TEST_F(MetadataTest, SpecialCharactersInFilenames) {
  // Scenario: Handling metadata for files with special characters

  // Given files with special characters exist
  const std::string special_file1 = "file-with-dashes.txt";
  const std::string special_file2 = "file_with_underscores.txt";
  const std::string special_file3 = "file.with.dots.txt";
  const std::string content = "special chars content";

  given_a_file_exists_with_content(special_file1, content);
  given_a_file_exists_with_content(special_file2, content);
  given_a_file_exists_with_content(special_file3, content);

  // When I get metadata for files with special characters
  auto metadata1 = when_i_get_metadata_for(special_file1);
  auto metadata2 = when_i_get_metadata_for(special_file2);
  auto metadata3 = when_i_get_metadata_for(special_file3);

  // Then all should be recognized as files with correct properties
  then_metadata_should_indicate_file_type(metadata1);
  then_metadata_should_indicate_file_type(metadata2);
  then_metadata_should_indicate_file_type(metadata3);

  then_content_length_should_be(metadata1, content.length());
  then_content_length_should_be(metadata2, content.length());
  then_content_length_should_be(metadata3, content.length());
}

// Feature: Metadata API Robustness
// As a developer using OpenDAL C++ bindings
// I want the metadata API to be robust and predictable
// So that I can build reliable applications

TEST_F(MetadataTest, RepeatedAccess) {
  // Scenario: Retrieving metadata multiple times for the same file

  // Given a file exists
  const std::string file_path = "repeated_access.txt";
  const std::string content = "repeated access content";
  given_a_file_exists_with_content(file_path, content);

  // When I get metadata multiple times
  auto metadata1 = when_i_get_metadata_for(file_path);
  auto metadata2 = when_i_get_metadata_for(file_path);
  auto metadata3 = when_i_get_metadata_for(file_path);

  // Then all metadata objects should be consistent
  EXPECT_EQ(metadata1.type, metadata2.type);
  EXPECT_EQ(metadata2.type, metadata3.type);
  EXPECT_EQ(metadata1.content_length, metadata2.content_length);
  EXPECT_EQ(metadata2.content_length, metadata3.content_length);
  EXPECT_EQ(metadata1.is_deleted, metadata2.is_deleted);
  EXPECT_EQ(metadata2.is_deleted, metadata3.is_deleted);

  // And all should behave identically
  EXPECT_EQ(metadata1.IsFile(), metadata2.IsFile());
  EXPECT_EQ(metadata2.IsFile(), metadata3.IsFile());
  EXPECT_EQ(metadata1.ContentLength(), metadata2.ContentLength());
  EXPECT_EQ(metadata2.ContentLength(), metadata3.ContentLength());
}

TEST_F(MetadataTest, AfterFileModification) {
  // Scenario: Getting metadata after modifying a file

  // Given a file exists with initial content
  const std::string file_path = "modifiable_file.txt";
  const std::string initial_content = "initial content";
  const std::string modified_content = "modified content with more text";
  given_a_file_exists_with_content(file_path, initial_content);

  // When I get initial metadata
  auto initial_metadata = when_i_get_metadata_for(file_path);

  // And I modify the file content
  op.Write(file_path, modified_content);

  // And I get metadata again
  auto modified_metadata = when_i_get_metadata_for(file_path);

  // Then both should indicate file type
  then_metadata_should_indicate_file_type(initial_metadata);
  then_metadata_should_indicate_file_type(modified_metadata);

  // And the content length should reflect the change
  then_content_length_should_be(initial_metadata, initial_content.length());
  then_content_length_should_be(modified_metadata, modified_content.length());

  // And the new content length should be different from the old one
  EXPECT_NE(initial_metadata.ContentLength(),
            modified_metadata.ContentLength());
  EXPECT_GT(modified_metadata.ContentLength(),
            initial_metadata.ContentLength());
}