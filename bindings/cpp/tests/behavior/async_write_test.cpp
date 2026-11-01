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

#ifdef OPENDAL_ENABLE_ASYNC

#include "../framework/test_framework.hpp"

namespace opendal::test {

class AsyncWriteBehaviorTest : public AsyncOpenDALTest {
protected:
    void SetUp() override {
        AsyncOpenDALTest::SetUp();
    }
};

// Test async writing empty content
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncWriteEmptyContent) {
    auto path = random_path();
    std::vector<uint8_t> empty_content;
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        co_await op_->write(path, empty_content);
        
        // Verify the file exists and is empty
        auto exists = co_await op_->exists(path);
        EXPECT_TRUE(exists);
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, empty_content);
        
        co_return;
    }());
}

// Test async writing small content
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncWriteSmallContent) {
    auto path = random_path();
    auto content = random_bytes(100);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        co_await op_->write(path, content);
        
        // Verify content
        auto exists = co_await op_->exists(path);
        EXPECT_TRUE(exists);
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        
        co_return;
    }());
}

// Test async writing large content
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncWriteLargeContent) {
    auto path = random_path();
    auto content = random_bytes(1024 * 1024); // 1MB
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        co_await op_->write(path, content);
        
        // Verify content
        auto exists = co_await op_->exists(path);
        EXPECT_TRUE(exists);
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        
        co_return;
    }());
}

// Test async concurrent writes to different files
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncConcurrentWritesDifferentFiles) {
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        std::vector<std::string> paths;
        std::vector<std::vector<uint8_t>> contents;
        
        // Prepare test data
        for (int i = 0; i < 10; ++i) {
            paths.push_back(random_path());
            contents.push_back(random_bytes(100 + i));
        }
        
        // Write all files concurrently
        std::vector<cppcoro::task<void>> write_tasks;
        for (size_t i = 0; i < paths.size(); ++i) {
            write_tasks.push_back(op_->write(paths[i], contents[i]));
        }
        
        // Wait for all writes to complete
        for (auto& task : write_tasks) {
            co_await task;
        }
        
        // Verify all files were written correctly
        for (size_t i = 0; i < paths.size(); ++i) {
            auto exists = co_await op_->exists(paths[i]);
            EXPECT_TRUE(exists);
            auto result = co_await op_->read(paths[i]);
            EXPECT_EQ(result, contents[i]);
        }
        
        co_return;
    }());
}

// Test async overwriting existing file
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncOverwriteExistingFile) {
    auto path = random_path();
    auto original_content = random_bytes(100);
    auto new_content = random_bytes(200);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write original content
        co_await op_->write(path, original_content);
        auto result1 = co_await op_->read(path);
        EXPECT_EQ(result1, original_content);
        
        // Overwrite with new content
        co_await op_->write(path, new_content);
        auto result2 = co_await op_->read(path);
        EXPECT_EQ(result2, new_content);
        EXPECT_NE(result2, original_content);
        
        co_return;
    }());
}

// Test async writing to nested path
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncWriteToNestedPath) {
    auto path = "nested/deep/directory/file.txt";
    auto content = random_bytes(100);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        co_await op_->write(path, content);
        
        // Verify content
        auto exists = co_await op_->exists(path);
        EXPECT_TRUE(exists);
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        
        co_return;
    }());
}

// Test async writing performance
OPENDAL_TEST_F(AsyncWriteBehaviorTest, AsyncWritePerformance) {
    std::vector<size_t> sizes = {1024, 10240, 102400}; // 1KB, 10KB, 100KB
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        for (auto size : sizes) {
            auto content = random_bytes(size);
            auto path = random_path();
            
            // Measure write performance
            auto start_time = std::chrono::high_resolution_clock::now();
            co_await op_->write(path, content);
            auto end_time = std::chrono::high_resolution_clock::now();
            
            auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time);
            
            // Verify the write was successful
            auto result = co_await op_->read(path);
            EXPECT_EQ(result.size(), size);
            
            std::cout << "Async write performance - Size: " << (size / 1024) << " KB, "
                      << "Time: " << duration.count() << " ms" << std::endl;
        }
        
        co_return;
    }());
}

} // namespace opendal::test

#endif // OPENDAL_ENABLE_ASYNC 