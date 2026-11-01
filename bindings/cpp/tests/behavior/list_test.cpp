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

#include "../framework/test_framework.hpp"
#include <set>
#include <thread>
#include <atomic>

namespace opendal::test {

class ListBehaviorTest : public OpenDALTest {
protected:
    void SetUp() override {
        OpenDALTest::SetUp();
    }
};

// Test listing empty directory
OPENDAL_TEST_F(ListBehaviorTest, ListEmptyDirectory) {
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    
    // Create empty directory
    op_.CreateDir(dir_path);
    
    // List the directory
    auto entries = op_.List(dir_path);
    
    // Directory may or may not be included in listing depending on service behavior
    bool found_dir = false;
    for (const auto& entry : entries) {
        if (entry.path == dir_path) {
            found_dir = true;
            auto metadata = op_.Stat(entry.path);
            EXPECT_EQ(metadata.type, opendal::EntryMode::DIR);
        }
    }
    
    // If directory not found in listing, verify it exists using Stat
    if (!found_dir) {
        auto metadata = op_.Stat(dir_path);
        EXPECT_EQ(metadata.type, opendal::EntryMode::DIR);
    }
}

// Test listing directory with files
OPENDAL_TEST_F(ListBehaviorTest, ListDirectoryWithFiles) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    auto file1_path = dir_path + "file1.txt";
    auto file2_path = dir_path + "file2.txt";
    auto file3_path = dir_path + "file3.txt";
    
    // Create directory and files
    op_.CreateDir(dir_path);
    op_.Write(file1_path, random_string(100));
    op_.Write(file2_path, random_string(200));
    op_.Write(file3_path, random_string(300));
    
    // List the directory
    auto entries = op_.List(dir_path);
    
    // Should contain 3 files, and possibly the directory itself depending on service behavior
    std::set<std::string> expected_paths = {file1_path, file2_path, file3_path};
    std::set<std::string> actual_paths;
    
    for (const auto& entry : entries) {
        actual_paths.insert(entry.path);
        
        auto metadata = op_.Stat(entry.path);
        if (entry.path == dir_path) {
            EXPECT_EQ(metadata.type, opendal::EntryMode::DIR);
        } else {
            EXPECT_EQ(metadata.type, opendal::EntryMode::FILE);
        }
    }
    
    // All files should be present
    for (const auto& expected_path : expected_paths) {
        EXPECT_TRUE(actual_paths.count(expected_path) > 0) << "Missing file: " << expected_path;
    }
    
    // Directory may or may not be included depending on service behavior
    if (actual_paths.count(dir_path) > 0) {
        EXPECT_EQ(entries.size(), 4); // directory + 3 files
    } else {
        EXPECT_EQ(entries.size(), 3); // only 3 files
    }
}

// Test listing nested directories
OPENDAL_TEST_F(ListBehaviorTest, ListNestedDirectories) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto base_dir = random_dir_path();
    auto sub_dir = base_dir + "subdir/";
    auto file1 = base_dir + "file1.txt";
    auto file2 = sub_dir + "file2.txt";
    
    // Create nested structure
    op_.CreateDir(base_dir);
    op_.CreateDir(sub_dir);
    op_.Write(file1, random_string(100));
    op_.Write(file2, random_string(200));
    
    // List base directory
    auto entries = op_.List(base_dir);
    std::set<std::string> expected_paths = {sub_dir, file1};
    std::set<std::string> actual_paths;
    
    for (const auto& entry : entries) {
        actual_paths.insert(entry.path);
    }
    
    // All required paths should be present
    for (const auto& expected_path : expected_paths) {
        EXPECT_TRUE(actual_paths.count(expected_path) > 0) << "Missing path: " << expected_path;
    }
    
    // Directory may or may not be included depending on service behavior
    if (actual_paths.count(base_dir) > 0) {
        EXPECT_EQ(entries.size(), 3); // directory + subdirectory + file
    } else {
        EXPECT_EQ(entries.size(), 2); // only subdirectory + file
    }
    
    // List subdirectory
    auto sub_entries = op_.List(sub_dir);
    std::set<std::string> sub_expected_paths = {file2};
    std::set<std::string> sub_actual_paths;
    
    for (const auto& entry : sub_entries) {
        sub_actual_paths.insert(entry.path);
    }
    
    // File should be present in subdirectory listing
    for (const auto& expected_path : sub_expected_paths) {
        EXPECT_TRUE(sub_actual_paths.count(expected_path) > 0) << "Missing path: " << expected_path;
    }
    
    // Subdirectory may or may not be included depending on service behavior
    if (sub_actual_paths.count(sub_dir) > 0) {
        EXPECT_EQ(sub_entries.size(), 2); // subdirectory + file
    } else {
        EXPECT_EQ(sub_entries.size(), 1); // only file
    }
}

// Test listing non-existent directory
OPENDAL_TEST_F(ListBehaviorTest, ListNonExistentDirectory) {
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    
    // Ensure directory doesn't exist
    EXPECT_FALSE(op_.Exists(dir_path));
    
    // Listing non-existent directory should either throw or return empty results
    // Different backends may handle this differently
    try {
        auto entries = op_.List(dir_path);
        // If no exception, should return empty or minimal results
        EXPECT_LE(entries.size(), 1); // At most the directory itself
    } catch (const std::exception& e) {
        // Exception is also acceptable behavior
        SUCCEED() << "Expected exception thrown: " << e.what();
    }
}

// Test listing with many files
OPENDAL_TEST_F(ListBehaviorTest, ListManyFiles) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    const int num_files = 100;
    std::vector<std::string> file_paths;
    
    // Create directory
    op_.CreateDir(dir_path);
    
    // Create many files
    for (int i = 0; i < num_files; ++i) {
        auto file_path = dir_path + "file_" + std::to_string(i) + ".txt";
        op_.Write(file_path, random_string(50));
        file_paths.push_back(file_path);
    }
    
    // List the directory
    auto entries = op_.List(dir_path);
    std::set<std::string> expected_paths(file_paths.begin(), file_paths.end());
    std::set<std::string> actual_paths;
    
    for (const auto& entry : entries) {
        actual_paths.insert(entry.path);
    }
    
    // All files should be present
    for (const auto& expected_path : expected_paths) {
        EXPECT_TRUE(actual_paths.count(expected_path) > 0) << "Missing file: " << expected_path;
    }
    
    // Directory may or may not be included depending on service behavior
    if (actual_paths.count(dir_path) > 0) {
        EXPECT_EQ(entries.size(), num_files + 1); // directory + files
    } else {
        EXPECT_EQ(entries.size(), num_files); // only files
    }
}

// Test listing with special character names
OPENDAL_TEST_F(ListBehaviorTest, ListSpecialCharNames) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    auto file1 = dir_path + "file-with.special_chars.txt";
    auto file2 = dir_path + "file with spaces.txt";
    auto file3 = dir_path + "file_123.txt";
    
    // Create directory and files
    op_.CreateDir(dir_path);
    op_.Write(file1, random_string(100));
    op_.Write(file2, random_string(100));
    op_.Write(file3, random_string(100));
    
    // List the directory
    auto entries = op_.List(dir_path);
    std::set<std::string> expected_paths = {file1, file2, file3};
    std::set<std::string> actual_paths;
    
    for (const auto& entry : entries) {
        actual_paths.insert(entry.path);
    }
    
    // All files should be present
    for (const auto& expected_path : expected_paths) {
        EXPECT_TRUE(actual_paths.count(expected_path) > 0) << "Missing file: " << expected_path;
    }
    
    // Directory may or may not be included depending on service behavior
    if (actual_paths.count(dir_path) > 0) {
        EXPECT_EQ(entries.size(), 4); // directory + 3 files
    } else {
        EXPECT_EQ(entries.size(), 3); // only 3 files
    }
}

// Test using lister iterator
OPENDAL_TEST_F(ListBehaviorTest, ListerIterator) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    auto file1_path = dir_path + "file1.txt";
    auto file2_path = dir_path + "file2.txt";
    
    // Create directory and files
    op_.CreateDir(dir_path);
    op_.Write(file1_path, random_string(100));
    op_.Write(file2_path, random_string(200));
    
    // Use lister
    auto lister = op_.List(dir_path);
    std::set<std::string> expected_paths = {file1_path, file2_path};
    std::set<std::string> actual_paths;
    
    for (const auto& entry : lister) {
        actual_paths.insert(entry.path);
        
        auto metadata = op_.Stat(entry.path);
        if (entry.path == dir_path) {
            EXPECT_EQ(metadata.type, opendal::EntryMode::DIR);
        } else {
            EXPECT_EQ(metadata.type, opendal::EntryMode::FILE);
        }
    }
    
    // All files should be present
    for (const auto& expected_path : expected_paths) {
        EXPECT_TRUE(actual_paths.count(expected_path) > 0) << "Missing file: " << expected_path;
    }
    
    // Directory may or may not be included depending on service behavior
    if (actual_paths.count(dir_path) > 0) {
        EXPECT_EQ(actual_paths.size(), 3); // directory + 2 files
    } else {
        EXPECT_EQ(actual_paths.size(), 2); // only 2 files
    }
}

// Test listing root directory
OPENDAL_TEST_F(ListBehaviorTest, ListRootDirectory) {
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    // List root directory (empty path or "/")
    auto entries = op_.List("/");
    
    // Should not throw and return some entries
    EXPECT_NO_THROW({
        auto entries = op_.List("/");
    });
}

// Test metadata in list results
OPENDAL_TEST_F(ListBehaviorTest, ListMetadata) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    auto file_path = dir_path + "test_file.txt";
    auto content = random_string(1000);
    
    // Create directory and file
    op_.CreateDir(dir_path);
    op_.Write(file_path, content);
    
    // List the directory
    auto entries = op_.List(dir_path);
    
    for (const auto& entry : entries) {
        auto metadata = op_.Stat(entry.path);
        if (entry.path == file_path) {
            EXPECT_EQ(metadata.type, opendal::EntryMode::FILE);
            EXPECT_EQ(metadata.content_length, content.size());
        } else if (entry.path == dir_path) {
            EXPECT_EQ(metadata.type, opendal::EntryMode::DIR);
        }
    }
}

// Test concurrent listing
OPENDAL_TEST_F(ListBehaviorTest, ConcurrentListing) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_LIST();
    auto dir_path = random_dir_path();
    
    // Create directory with some files
    op_.CreateDir(dir_path);
    for (int i = 0; i < 10; ++i) {
        auto file_path = dir_path + "file_" + std::to_string(i) + ".txt";
        op_.Write(file_path, random_string(100));
    }
    
    const int num_threads = 5;
    std::vector<std::thread> threads;
    std::vector<std::vector<opendal::Entry>> results(num_threads);
    std::atomic<int> error_count{0};
    
    for (int i = 0; i < num_threads; ++i) {
        threads.emplace_back([&, i]() {
            try {
                results[i] = op_.List(dir_path);
            } catch (const std::exception& e) {
                error_count++;
            }
        });
    }
    
    for (auto& thread : threads) {
        thread.join();
    }
    
    EXPECT_EQ(error_count, 0);
    
    // All results should be the same
    for (int i = 1; i < num_threads; ++i) {
        EXPECT_EQ(results[0].size(), results[i].size());
    }
}

} // namespace opendal::test 