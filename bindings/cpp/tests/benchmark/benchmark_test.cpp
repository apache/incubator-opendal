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

class OpenDALBenchmarkTest : public BenchmarkTest {
protected:
    void SetUp() override {
        BenchmarkTest::SetUp();
    }
};

// Benchmark write operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkWriteOperations) {
    std::vector<size_t> sizes = {1024, 10240, 102400, 1024*1024}; // 1KB, 10KB, 100KB, 1MB
    
    for (auto size : sizes) {
        auto content = random_string(size);
        
        benchmark_operation(
            "Write " + std::to_string(size) + " bytes",
            [&]() {
                auto path = random_path();
                op_.write(path, content);
            },
            10 // iterations
        );
    }
}

// Benchmark read operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkReadOperations) {
    std::vector<size_t> sizes = {1024, 10240, 102400, 1024*1024}; // 1KB, 10KB, 100KB, 1MB
    
    for (auto size : sizes) {
        auto content = random_string(size);
        auto path = random_path();
        
        // Pre-write the file
        op_.write(path, content);
        
        benchmark_operation(
            "Read " + std::to_string(size) + " bytes",
            [&]() {
                auto result = op_.read(path);
                // Verify size to ensure read actually happened
                EXPECT_EQ(result.size(), size);
            },
            10 // iterations
        );
    }
}

// Benchmark exists operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkExistsOperations) {
    auto path = random_path();
    auto content = random_string(1024);
    
    // Create the file
    op_.write(path, content);
    
    benchmark_operation(
        "Exists check",
        [&]() {
            bool exists = op_.exists(path);
            EXPECT_TRUE(exists);
        },
        10 // iterations
    );
}

// Benchmark list operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkListOperations) {
    auto dir_path = random_dir_path();
    
    // Create directory with files
    op_.create_dir(dir_path);
    for (int i = 0; i < 100; ++i) {
        auto file_path = dir_path + "file_" + std::to_string(i) + ".txt";
        op_.write(file_path, random_string(100));
    }
    
    benchmark_operation(
        "List directory with 100 files",
        [&]() {
            auto entries = op_.list(dir_path);
            EXPECT_EQ(entries.size(), 101); // 100 files + 1 directory
        },
        10 // iterations
    );
}

// Benchmark delete operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkDeleteOperations) {
    benchmark_operation(
        "Delete operations",
        [&]() {
            auto path = random_path();
            auto content = random_string(1024);
            
            // Create and delete file
            op_.write(path, content);
            op_.remove(path);
        },
        10 // iterations
    );
}

// Benchmark create directory operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkCreateDirOperations) {
    benchmark_operation(
        "Create directory operations",
        [&]() {
            auto dir_path = random_dir_path();
            op_.create_dir(dir_path);
        },
        10 // iterations
    );
}

// Benchmark stat operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkStatOperations) {
    auto path = random_path();
    auto content = random_string(1024);
    
    // Create the file
    op_.write(path, content);
    
    benchmark_operation(
        "Stat operations",
        [&]() {
            auto metadata = op_.stat(path);
            EXPECT_EQ(metadata.type, opendal::EntryMode::FILE);
            EXPECT_EQ(metadata.content_length, content.size());
        },
        10 // iterations
    );
}

// Benchmark reader operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkReaderOperations) {
    auto path = random_path();
    const size_t file_size = 1024 * 1024; // 1MB
    auto content = random_string(file_size);
    
    // Create the file
    op_.write(path, content);
    
    benchmark_operation(
        "Reader sequential read",
        [&]() {
            auto reader = op_.reader(path);
            const size_t chunk_size = 4096; // 4KB chunks
            std::string buffer(chunk_size, 0);
            
            for (size_t offset = 0; offset < file_size; offset += chunk_size) {
                reader.seek(offset, std::ios::beg);
                size_t read_size = std::min(chunk_size, file_size - offset);
                reader.read(buffer.data(), read_size);
            }
        },
        10 // iterations
    );
}

// Benchmark concurrent operations
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkConcurrentOperations) {
    const int num_threads = 10;
    const int operations_per_thread = 100;
    
    auto start_time = std::chrono::high_resolution_clock::now();
    
    std::vector<std::thread> threads;
    std::atomic<int> completed_operations{0};
    
    for (int t = 0; t < num_threads; ++t) {
        threads.emplace_back([&, t]() {
            for (int i = 0; i < operations_per_thread; ++i) {
                auto path = random_path();
                auto content = random_string(1024);
                
                // Write, read, delete cycle
                op_.write(path, content);
                auto result = op_.read(path);
                op_.remove(path);
                
                completed_operations++;
            }
        });
    }
    
    for (auto& thread : threads) {
        thread.join();
    }
    
    auto end_time = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time);
    
    int total_operations = num_threads * operations_per_thread * 3; // write + read + delete
    double ops_per_sec = (total_operations * 1000.0) / duration.count();
    
    std::cout << "Concurrent benchmark results:" << std::endl;
    std::cout << "  Threads: " << num_threads << std::endl;
    std::cout << "  Total operations: " << total_operations << std::endl;
    std::cout << "  Duration: " << duration.count() << " ms" << std::endl;
    std::cout << "  Operations per second: " << ops_per_sec << std::endl;
    
    EXPECT_EQ(completed_operations, num_threads * operations_per_thread);
}

// Benchmark memory usage patterns
OPENDAL_TEST_F(OpenDALBenchmarkTest, BenchmarkMemoryUsage) {
    // Test with different file sizes to understand memory usage
    std::vector<size_t> sizes = {1024, 102400, 1024*1024, 10*1024*1024}; // 1KB to 10MB
    
    for (auto size : sizes) {
        auto path = random_path();
        auto content = random_string(size);
        
        timer_.start();
        
        // Write and read back to measure memory allocation patterns
        op_.write(path, content);
        auto result = op_.read(path);
        
        timer_.stop();
        
        EXPECT_EQ(result.size(), size);
        
        std::cout << "Memory usage test - Size: " << (size / 1024) << " KB, "
                  << "Time: " << timer_.elapsed_ms() << " ms" << std::endl;
        
        // Cleanup
        op_.remove(path);
    }
}

} // namespace opendal::test 