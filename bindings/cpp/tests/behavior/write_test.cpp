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

class WriteBehaviorTest : public OpenDALTest {
protected:
    void SetUp() override {
        OpenDALTest::SetUp();
    }
};

// Test writing empty content
OPENDAL_TEST_F(WriteBehaviorTest, WriteEmptyContent) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    // Check if the service supports writing empty content
    if (!op_.Info().write_can_empty) {
        GTEST_SKIP() << "Service doesn't support writing empty content";
        return;
    }
    auto path = random_path();
    std::string empty_content = "";
    
    op_.Write(path, empty_content);
    
    // Verify the file exists and is empty
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    EXPECT_EQ(result, empty_content);
}

// Test writing small content
OPENDAL_TEST_F(WriteBehaviorTest, WriteSmallContent) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = random_path();
    auto content = random_string(100);
    
    op_.Write(path, content);
    
    // Verify content
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    EXPECT_EQ(result, content);
}

// Test writing large content
OPENDAL_TEST_F(WriteBehaviorTest, WriteLargeContent) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = random_path();
    auto content = random_string(1024 * 1024); // 1MB
    
    op_.Write(path, content);
    
    // Verify content
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    EXPECT_EQ(result, content);
}

// Test writing binary data
OPENDAL_TEST_F(WriteBehaviorTest, WriteBinaryData) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = random_path();
    auto content = random_bytes(1000);
    
    // Convert to string for writing
    std::string content_str(content.begin(), content.end());
    op_.Write(path, content_str);
    
    // Verify content
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    std::vector<uint8_t> result_bytes(result.begin(), result.end());
    EXPECT_EQ(result_bytes, content);
}

// Test overwriting existing file
OPENDAL_TEST_F(WriteBehaviorTest, OverwriteExistingFile) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = random_path();
    auto original_content = random_string(100);
    auto new_content = random_string(200);
    
    // Write original content
    op_.Write(path, original_content);
    auto result1 = op_.Read(path);
    EXPECT_EQ(result1, original_content);
    
    // Try to overwrite with new content
    try {
        op_.Write(path, new_content);
        auto result2 = op_.Read(path);
        EXPECT_EQ(result2, new_content);
        EXPECT_NE(result2, original_content);
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

// Test writing to nested path (should create intermediate directories)
OPENDAL_TEST_F(WriteBehaviorTest, WriteToNestedPath) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = "nested/deep/directory/file.txt";
    auto content = random_string(100);
    
    op_.Write(path, content);
    
    // Verify content
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    EXPECT_EQ(result, content);
}

// Test writing with special characters in filename
OPENDAL_TEST_F(WriteBehaviorTest, WriteSpecialCharFilename) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = "test_with-special.chars_123/file-name_with.special.txt";
    auto content = random_string(100);
    
    op_.Write(path, content);
    
    // Verify content
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    EXPECT_EQ(result, content);
}

// Test writing multiple files
OPENDAL_TEST_F(WriteBehaviorTest, WriteMultipleFiles) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    std::vector<std::string> paths;
    std::vector<std::string> contents;
    
    for (int i = 0; i < 10; ++i) {
        auto path = random_path();
        auto content = random_string(100 + i * 10);
        
        paths.push_back(path);
        contents.push_back(content);
        
        op_.Write(path, content);
    }
    
    // Verify all files
    for (size_t i = 0; i < paths.size(); ++i) {
        EXPECT_TRUE(op_.Exists(paths[i]));
        auto result = op_.Read(paths[i]);
        EXPECT_EQ(result, contents[i]);
    }
}

// Test writing with unicode content
OPENDAL_TEST_F(WriteBehaviorTest, WriteUnicodeContent) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = random_path();
    std::string unicode_content = "Hello 世界 🌍 Здравствуй мир";
    
    op_.Write(path, unicode_content);
    
    // Verify content
    EXPECT_TRUE(op_.Exists(path));
    auto result = op_.Read(path);
    EXPECT_EQ(result, unicode_content);
}

// Test concurrent writes to different files
OPENDAL_TEST_F(WriteBehaviorTest, ConcurrentWritesDifferentFiles) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    const int num_threads = 10;
    std::vector<std::thread> threads;
    std::vector<std::string> paths(num_threads);
    std::vector<std::string> contents(num_threads);
    std::atomic<int> error_count{0};
    
    for (int i = 0; i < num_threads; ++i) {
        paths[i] = random_path();
        contents[i] = random_string(100 + i);
    }
    
    for (int i = 0; i < num_threads; ++i) {
        threads.emplace_back([&, i]() {
            try {
                op_.Write(paths[i], contents[i]);
            } catch (const std::exception& e) {
                error_count++;
            }
        });
    }
    
    for (auto& thread : threads) {
        thread.join();
    }
    
    EXPECT_EQ(error_count, 0);
    
    // Verify all files were written correctly
    for (int i = 0; i < num_threads; ++i) {
        EXPECT_TRUE(op_.Exists(paths[i]));
        auto result = op_.Read(paths[i]);
        EXPECT_EQ(result, contents[i]);
    }
}

// Test writing with different content sizes
OPENDAL_TEST_F(WriteBehaviorTest, WriteDifferentSizes) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    std::vector<size_t> sizes = {1, 10, 100, 1024, 10240, 102400};
    
    for (auto size : sizes) {
        auto path = random_path();
        auto content = random_string(size);
        
        op_.Write(path, content);
        
        // Verify content
        EXPECT_TRUE(op_.Exists(path));
        auto result = op_.Read(path);
        EXPECT_EQ(result.size(), size);
        EXPECT_EQ(result, content);
    }
}

// Test appending behavior (if supported)
OPENDAL_TEST_F(WriteBehaviorTest, WriteAppendBehavior) {
    OPENDAL_SKIP_IF_UNSUPPORTED_WRITE();
    auto path = random_path();
    auto content1 = random_string(100);
    auto content2 = random_string(100);
    
    // Write first content
    op_.Write(path, content1);
    auto result1 = op_.Read(path);
    EXPECT_EQ(result1, content1);
    
    // Try to write second content (should overwrite, not append for normal write)
    try {
        op_.Write(path, content2);
        auto result2 = op_.Read(path);
        EXPECT_EQ(result2, content2);
        EXPECT_NE(result2, content1 + content2); // Should not be appended
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

} // namespace opendal::test 