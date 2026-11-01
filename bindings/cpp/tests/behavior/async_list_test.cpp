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

class AsyncListBehaviorTest : public AsyncOpenDALTest {
protected:
    void SetUp() override {
        AsyncOpenDALTest::SetUp();
    }
};

// Test async listing empty directory
OPENDAL_TEST_F(AsyncListBehaviorTest, AsyncListEmptyDirectory) {
    auto dir_path = random_dir_path();
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Create empty directory
        co_await op_->create_dir(dir_path);
        
        // List the directory
        auto entries = co_await op_->list(dir_path);
        
        // Should contain at least the directory itself
        EXPECT_GE(entries.size(), 1);
        
        bool found_dir = false;
        for (const auto& entry : entries) {
            if (entry == dir_path) {
                found_dir = true;
            }
        }
        EXPECT_TRUE(found_dir);
        
        co_return;
    }());
}

// Test async listing directory with files
OPENDAL_TEST_F(AsyncListBehaviorTest, AsyncListDirectoryWithFiles) {
    auto dir_path = random_dir_path();
    auto file1_path = dir_path + "file1.txt";
    auto file2_path = dir_path + "file2.txt";
    auto file3_path = dir_path + "file3.txt";
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Create directory and files
        co_await op_->create_dir(dir_path);
        co_await op_->write(file1_path, random_bytes(100));
        co_await op_->write(file2_path, random_bytes(200));
        co_await op_->write(file3_path, random_bytes(300));
        
        // List the directory
        auto entries = co_await op_->list(dir_path);
        
        // Should contain directory and 3 files
        EXPECT_EQ(entries.size(), 4);
        
        std::set<std::string> expected_paths = {dir_path, file1_path, file2_path, file3_path};
        std::set<std::string> actual_paths(entries.begin(), entries.end());
        
        EXPECT_EQ(actual_paths, expected_paths);
        
        co_return;
    }());
}

// Test async listing with lister
OPENDAL_TEST_F(AsyncListBehaviorTest, AsyncListerTest) {
    auto dir_path = random_dir_path();
    auto file1_path = dir_path + "file1.txt";
    auto file2_path = dir_path + "file2.txt";
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Create directory and files
        co_await op_->create_dir(dir_path);
        co_await op_->write(file1_path, random_bytes(100));
        co_await op_->write(file2_path, random_bytes(200));
        
        // Use async lister
        auto lister_id = co_await op_->lister(dir_path);
        opendal::async::Lister lister(lister_id);
        
        std::set<std::string> expected_paths = {dir_path, file1_path, file2_path};
        std::set<std::string> actual_paths;
        
        // List all entries using lister
        while (true) {
            auto entry = co_await lister.next();
            if (entry.empty()) break; // End of iteration
            actual_paths.insert(entry);
        }
        
        EXPECT_EQ(actual_paths, expected_paths);
        
        co_return;
    }());
}

// Test async concurrent listing
OPENDAL_TEST_F(AsyncListBehaviorTest, AsyncConcurrentListing) {
    auto dir_path = random_dir_path();
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Create directory with some files
        co_await op_->create_dir(dir_path);
        for (int i = 0; i < 10; ++i) {
            auto file_path = dir_path + "file_" + std::to_string(i) + ".txt";
            co_await op_->write(file_path, random_bytes(100));
        }
        
        // Perform concurrent listings
        std::vector<cppcoro::task<std::vector<std::string>>> list_tasks;
        for (int i = 0; i < 5; ++i) {
            list_tasks.push_back(op_->list(dir_path));
        }
        
        // Wait for all listing operations to complete
        std::vector<std::vector<std::string>> results;
        for (auto& task : list_tasks) {
            results.push_back(co_await task);
        }
        
        // All results should be the same
        for (size_t i = 1; i < results.size(); ++i) {
            EXPECT_EQ(results[0].size(), results[i].size());
        }
        
        co_return;
    }());
}

// Test async listing performance
OPENDAL_TEST_F(AsyncListBehaviorTest, AsyncListPerformance) {
    auto dir_path = random_dir_path();
    const int num_files = 100;
    
    cppcoro::sync_wait([&]() -> cppcoro::task<void> {
        // Create directory with many files
        co_await op_->create_dir(dir_path);
        
        std::vector<cppcoro::task<void>> write_tasks;
        for (int i = 0; i < num_files; ++i) {
            auto file_path = dir_path + "file_" + std::to_string(i) + ".txt";
            write_tasks.push_back(op_->write(file_path, random_bytes(50)));
        }
        
        // Wait for all files to be created
        for (auto& task : write_tasks) {
            co_await task;
        }
        
        // Measure list performance
        auto start_time = std::chrono::high_resolution_clock::now();
        auto entries = co_await op_->list(dir_path);
        auto end_time = std::chrono::high_resolution_clock::now();
        
        auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time);
        
        // Should contain directory + num_files
        EXPECT_EQ(entries.size(), num_files + 1);
        
        std::cout << "Async list performance: Listed " << entries.size() 
                  << " entries in " << duration.count() << " ms" << std::endl;
        
        co_return;
    }());
}

} // namespace opendal::test

#endif // OPENDAL_ENABLE_ASYNC 