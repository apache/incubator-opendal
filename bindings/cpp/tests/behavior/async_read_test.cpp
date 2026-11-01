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

class AsyncReadBehaviorTest : public AsyncOpenDALTest {
protected:
    void SetUp() override {
        AsyncOpenDALTest::SetUp();
    }
};

// Test async reading non-existent file
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadNonExistentFile) {
    auto path = random_path();
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        bool caught_exception = false;
        try {
            auto content = co_await op_->read(path);
        } catch (const std::exception& e) {
            caught_exception = true;
        }
        EXPECT_TRUE(caught_exception);
        co_return;
    }());
}

// Test async reading empty file
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadEmptyFile) {
    auto path = random_path();
    std::vector<uint8_t> empty_content;
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write empty file
        co_await op_->write(path, empty_content);
        
        // Read it back
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, empty_content);
        co_return;
    }());
}

// Test async reading small file
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadSmallFile) {
    auto path = random_path();
    auto content = random_bytes(100);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write the file
        co_await op_->write(path, content);
        
        // Read it back
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        co_return;
    }());
}

// Test async reading large file
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadLargeFile) {
    auto path = random_path();
    auto content = random_bytes(1024 * 1024); // 1MB
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write the file
        co_await op_->write(path, content);
        
        // Read it back
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        co_return;
    }());
}

// Test async reading multiple files concurrently
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadMultipleFilesConcurrent) {
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        std::vector<std::string> paths;
        std::vector<std::vector<uint8_t>> contents;
        
        // Create multiple files
        for (int i = 0; i < 10; ++i) {
            auto path = random_path();
            auto content = random_bytes(100 + i * 10);
            
            paths.push_back(path);
            contents.push_back(content);
            
            co_await op_->write(path, content);
        }
        
        // Read all files concurrently
        std::vector<cppcoro::task<std::vector<uint8_t>>> read_tasks;
        for (const auto& path : paths) {
            read_tasks.push_back(op_->read(path));
        }
        
        // Wait for all reads to complete
        for (size_t i = 0; i < read_tasks.size(); ++i) {
            auto result = co_await read_tasks[i];
            EXPECT_EQ(result, contents[i]);
        }
        
        co_return;
    }());
}

// Test async reading with reader
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReaderTest) {
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        auto path = random_path();
        auto content = random_bytes(2000);
        
        // Write the file
        co_await op_->write(path, content);
        
        // Create async reader
        auto reader_id = co_await op_->reader(path);
        opendal::async::Reader reader(reader_id);
        
        // Read full file
        auto full_data = co_await reader.read(0, content.size());
        EXPECT_EQ(full_data, content);
        
        // Read in chunks
        const size_t chunk_size = 100;
        std::vector<uint8_t> chunked_data;
        
        for (size_t offset = 0; offset < content.size(); offset += chunk_size) {
            size_t read_size = std::min(chunk_size, content.size() - offset);
            auto chunk = co_await reader.read(offset, read_size);
            
            EXPECT_EQ(chunk.size(), read_size);
            chunked_data.insert(chunked_data.end(), chunk.begin(), chunk.end());
        }
        
        EXPECT_EQ(chunked_data, content);
        
        co_return;
    }());
}

// Test async reading after modification
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadAfterModification) {
    auto path = random_path();
    auto original_content = random_bytes(100);
    auto modified_content = random_bytes(200);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write original content
        co_await op_->write(path, original_content);
        
        // Verify original content
        auto result1 = co_await op_->read(path);
        EXPECT_EQ(result1, original_content);
        
        // Modify the file
        co_await op_->write(path, modified_content);
        
        // Verify modified content
        auto result2 = co_await op_->read(path);
        EXPECT_EQ(result2, modified_content);
        EXPECT_NE(result2, original_content);
        
        co_return;
    }());
}

// Test async reading with special characters in path
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadSpecialCharPath) {
    auto path = "test_with-special.chars_123/file.txt";
    auto content = random_bytes(100);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Create directory first
        co_await op_->create_dir("test_with-special.chars_123/");
        
        // Write the file
        co_await op_->write(path, content);
        
        // Read it back
        auto result = co_await op_->read(path);
        EXPECT_EQ(result, content);
        
        co_return;
    }());
}

// Test async reading with timeout (if supported)
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadTimeout) {
    auto path = random_path();
    auto content = random_bytes(1000);
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write the file
        co_await op_->write(path, content);
        
        // Read with reasonable timeout
        auto start_time = std::chrono::high_resolution_clock::now();
        auto result = co_await op_->read(path);
        auto end_time = std::chrono::high_resolution_clock::now();
        
        EXPECT_EQ(result, content);
        
        // Should complete within reasonable time
        auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time);
        EXPECT_LT(duration.count(), 5000); // 5 seconds should be plenty
        
        co_return;
    }());
}

// Test async reading with error handling
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadErrorHandling) {
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        std::vector<std::string> invalid_paths = {
            "",  // empty path
            "/invalid/very/deep/path/that/should/not/exist",
            "invalid\0path", // null character
        };
        
        for (const auto& invalid_path : invalid_paths) {
            bool caught_exception = false;
            try {
                auto content = co_await op_->read(invalid_path);
            } catch (const std::exception& e) {
                caught_exception = true;
            }
            
            // Should either throw or return empty (depending on implementation)
            // For most cases, we expect an exception
            if (!invalid_path.empty()) {
                EXPECT_TRUE(caught_exception);
            }
        }
        
        co_return;
    }());
}

// Test async reading performance
OPENDAL_TEST_F(AsyncReadBehaviorTest, AsyncReadPerformance) {
    auto path = random_path();
    auto content = random_bytes(1024 * 1024); // 1MB
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Write the file
        co_await op_->write(path, content);
        
        // Measure read performance
        const int iterations = 10;
        auto start_time = std::chrono::high_resolution_clock::now();
        
        for (int i = 0; i < iterations; ++i) {
            auto result = co_await op_->read(path);
            EXPECT_EQ(result.size(), content.size());
        }
        
        auto end_time = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time);
        
        std::cout << "Async read performance: " << iterations << " reads of " 
                  << content.size() << " bytes in " << duration.count() << " ms" << std::endl;
        std::cout << "Average: " << (duration.count() / iterations) << " ms per read" << std::endl;
        
        co_return;
    }());
}

} // namespace opendal::test

#endif // OPENDAL_ENABLE_ASYNC 