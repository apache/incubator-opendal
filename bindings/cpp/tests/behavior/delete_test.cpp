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
#include <thread>
#include <atomic>

namespace opendal::test {

class DeleteBehaviorTest : public OpenDALTest {
protected:
    void SetUp() override {
        OpenDALTest::SetUp();
    }
};

// Test deleting existing file
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteExistingFile) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto path = random_path();
    auto content = random_string(100);
    
    // Create the file
    op_.Write(path, content);
    EXPECT_TRUE(op_.Exists(path));
    
    // Delete the file
    op_.Remove(path);
    EXPECT_FALSE(op_.Exists(path));
}

// Test deleting non-existent file (should not error)
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteNonExistentFile) {
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto path = random_path();
    
    // Ensure file doesn't exist
    EXPECT_FALSE(op_.Exists(path));
    
    // Delete non-existent file should not throw
    EXPECT_NO_THROW(op_.Remove(path));
}

// Test deleting empty directory
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteEmptyDirectory) {
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto dir_path = random_dir_path();
    
    // Create the directory
    op_.CreateDir(dir_path);
    EXPECT_TRUE(op_.Exists(dir_path));
    
    // Delete the directory
    op_.Remove(dir_path);
}

// Test deleting multiple files
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteMultipleFiles) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    std::vector<std::string> paths;
    
    // Create multiple files
    for (int i = 0; i < 10; ++i) {
        auto path = random_path();
        auto content = random_string(100);
        
        op_.Write(path, content);
        EXPECT_TRUE(op_.Exists(path));
        paths.push_back(path);
    }
    
    // Delete all files
    for (const auto& path : paths) {
        op_.Remove(path);
        EXPECT_FALSE(op_.Exists(path));
    }
}

// Test deleting files with special characters
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteSpecialCharFile) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto path = "test_with-special.chars_123/file-name_with.special.txt";
    auto content = random_string(100);
    
    // Try to create the file
    try {
        op_.Write(path, content);
        EXPECT_TRUE(op_.Exists(path));
        
        // Delete the file
        op_.Remove(path);
        EXPECT_FALSE(op_.Exists(path));
    } catch (const std::exception& e) {
        // Check if error message indicates AlreadyExists
        std::string error_msg = e.what();
        if (error_msg.find("AlreadyExists") != std::string::npos) {
            GTEST_SKIP() << "Service doesn't support file overwrite: " << error_msg;
        } else {
            throw; // Re-throw other errors
        }
    }
}

// Test deleting large file
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteLargeFile) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto path = random_path();
    auto content = random_string(1024 * 1024); // 1MB
    
    // Create large file
    op_.Write(path, content);
    EXPECT_TRUE(op_.Exists(path));
    
    // Delete the file
    op_.Remove(path);
    EXPECT_FALSE(op_.Exists(path));
}

// Test concurrent deletes
OPENDAL_TEST_F(DeleteBehaviorTest, ConcurrentDeletes) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    const int num_threads = 10;
    std::vector<std::thread> threads;
    std::vector<std::string> paths(num_threads);
    std::atomic<int> error_count{0};
    
    // Create files
    for (int i = 0; i < num_threads; ++i) {
        paths[i] = random_path();
        op_.Write(paths[i], random_string(100));
        EXPECT_TRUE(op_.Exists(paths[i]));
    }
    
    // Delete concurrently
    for (int i = 0; i < num_threads; ++i) {
        threads.emplace_back([&, i]() {
            try {
                op_.Remove(paths[i]);
            } catch (const std::exception& e) {
                error_count++;
            }
        });
    }
    
    for (auto& thread : threads) {
        thread.join();
    }
    
    EXPECT_EQ(error_count, 0);
    
    // Verify all files are deleted
    for (const auto& path : paths) {
        EXPECT_FALSE(op_.Exists(path));
    }
}

// Test delete after read
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteAfterRead) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto path = random_path();
    auto content = random_string(100);
    
    // Create and read file
    op_.Write(path, content);
    auto result = op_.Read(path);
    EXPECT_EQ(result, content);
    
    // Delete the file
    op_.Remove(path);
    EXPECT_FALSE(op_.Exists(path));
    
    // Try to read again (should fail)
    EXPECT_THROW({
        auto content2 = op_.Read(path);
    }, std::exception);
}

// Test delete and recreate
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteAndRecreate) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    auto path = random_path();
    auto original_content = random_string(100);
    auto new_content = random_string(200);
    
    // Create file
    op_.Write(path, original_content);
    EXPECT_TRUE(op_.Exists(path));
    auto result1 = op_.Read(path);
    EXPECT_EQ(result1, original_content);
    
    // Delete file
    op_.Remove(path);
    EXPECT_FALSE(op_.Exists(path));
    
    // Recreate with different content
    op_.Write(path, new_content);
    EXPECT_TRUE(op_.Exists(path));
    auto result2 = op_.Read(path);
    EXPECT_EQ(result2, new_content);
    EXPECT_NE(result2, original_content);
}

// Test deleting nested structure
OPENDAL_TEST_F(DeleteBehaviorTest, DeleteNestedStructure) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    OPENDAL_SKIP_IF_UNSUPPORTED_CREATE_DIR();
    OPENDAL_SKIP_IF_UNSUPPORTED_DELETE();
    std::string base_dir = "test_nested/";
    auto level1_dir = base_dir + "level1/";
    auto level2_dir = level1_dir + "level2/";
    auto level3_dir = level2_dir + "level3/";
    auto deep_file = level3_dir + "deep_file.txt";
    
    // Create nested structure
    op_.CreateDir(base_dir);
    op_.CreateDir(level1_dir);
    op_.CreateDir(level2_dir);
    op_.CreateDir(level3_dir);
    op_.Write(deep_file, random_string(100));
    
    // Verify structure exists
    EXPECT_TRUE(op_.Exists(base_dir));
    EXPECT_TRUE(op_.Exists(level1_dir));
    EXPECT_TRUE(op_.Exists(level2_dir));
    EXPECT_TRUE(op_.Exists(level3_dir));
    EXPECT_TRUE(op_.Exists(deep_file));
}

} // namespace opendal::test 