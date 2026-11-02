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

#include "test_framework.h"

// Test: Basic reader operations
void test_reader_basic(opendal_test_context* ctx)
{
    const char* path = "test_reader.txt";
    const char* content = "Hello, OpenDAL Reader!";
    size_t content_len = strlen(content);

    // Write test data first
    opendal_bytes data = {
        .data = (uint8_t*)content, .len = content_len, .capacity = content_len
    };

    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    // Create reader
    opendal_result_operator_reader reader_result = opendal_operator_reader(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(reader_result.error,
        "Reader creation should succeed");
    OPENDAL_ASSERT_NOT_NULL(reader_result.reader, "Reader should not be null");

    // Read entire content
    uint8_t buffer[100];
    opendal_result_reader_read read_result = opendal_reader_read(reader_result.reader, buffer, sizeof(buffer));
    OPENDAL_ASSERT_NO_ERROR(read_result.error, "Read operation should succeed");
    OPENDAL_ASSERT_EQ(content_len, read_result.size,
        "Read size should match content length");

    // Verify content
    OPENDAL_ASSERT(memcmp(content, buffer, content_len) == 0,
        "Read content should match written content");

    // Cleanup
    opendal_reader_free(reader_result.reader);
    opendal_operator_delete(ctx->config->operator_instance, path);
}

// Test: Reader seek operations
void test_reader_seek(opendal_test_context* ctx)
{
    const char* path = "test_reader_seek.txt";
    const char* content = "0123456789ABCDEFGHIJ";
    size_t content_len = strlen(content);

    // Write test data
    opendal_bytes data = {
        .data = (uint8_t*)content, .len = content_len, .capacity = content_len
    };

    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    // Create reader
    opendal_result_operator_reader reader_result = opendal_operator_reader(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(reader_result.error,
        "Reader creation should succeed");

    // Test seek from current position
    opendal_result_reader_seek seek_result = opendal_reader_seek(reader_result.reader, 5, OPENDAL_SEEK_CUR);
    OPENDAL_ASSERT_NO_ERROR(seek_result.error,
        "Seek from current should succeed");
    OPENDAL_ASSERT_EQ(5, seek_result.pos, "Position should be 5");

    // Read after seek
    uint8_t buffer[5];
    opendal_result_reader_read read_result = opendal_reader_read(reader_result.reader, buffer, 5);
    OPENDAL_ASSERT_NO_ERROR(read_result.error, "Read after seek should succeed");
    OPENDAL_ASSERT_EQ(5, read_result.size, "Should read 5 bytes");
    OPENDAL_ASSERT(memcmp("56789", buffer, 5) == 0,
        "Should read correct content after seek");

    // Test seek from beginning
    seek_result = opendal_reader_seek(reader_result.reader, 0, OPENDAL_SEEK_SET);
    OPENDAL_ASSERT_NO_ERROR(seek_result.error,
        "Seek from beginning should succeed");
    OPENDAL_ASSERT_EQ(0, seek_result.pos, "Position should be 0");

    // Read from beginning
    read_result = opendal_reader_read(reader_result.reader, buffer, 5);
    OPENDAL_ASSERT_NO_ERROR(read_result.error,
        "Read from beginning should succeed");
    OPENDAL_ASSERT(memcmp("01234", buffer, 5) == 0,
        "Should read correct content from beginning");

    // Test seek from end
    seek_result = opendal_reader_seek(reader_result.reader, -5, OPENDAL_SEEK_END);
    OPENDAL_ASSERT_NO_ERROR(seek_result.error, "Seek from end should succeed");
    OPENDAL_ASSERT_EQ(content_len - 5, seek_result.pos,
        "Position should be content_len - 5");

    // Read from near end
    read_result = opendal_reader_read(reader_result.reader, buffer, 5);
    OPENDAL_ASSERT_NO_ERROR(read_result.error,
        "Read from near end should succeed");
    OPENDAL_ASSERT(memcmp("FGHIJ", buffer, 5) == 0,
        "Should read correct content from near end");

    // Cleanup
    opendal_reader_free(reader_result.reader);
    opendal_operator_delete(ctx->config->operator_instance, path);
}

// Test: Basic writer operations
void test_writer_basic(opendal_test_context* ctx)
{
    const char* path = "test_writer.txt";
    const char* content1 = "Hello, ";
    const char* content2 = "OpenDAL Writer!";

    // Create writer
    opendal_result_operator_writer writer_result = opendal_operator_writer(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(writer_result.error,
        "Writer creation should succeed");
    OPENDAL_ASSERT_NOT_NULL(writer_result.writer, "Writer should not be null");

    // Write first part
    opendal_bytes data1;
    data1.data = (uint8_t*)content1;
    data1.len = strlen(content1);
    data1.capacity = strlen(content1);

    opendal_result_writer_write write_result = opendal_writer_write(writer_result.writer, &data1);
    OPENDAL_ASSERT_NO_ERROR(write_result.error, "First write should succeed");
    OPENDAL_ASSERT_EQ(strlen(content1), write_result.size,
        "Write size should match content length");

    // Write second part - handle OneShotWriter limitation
    opendal_bytes data2;
    data2.data = (uint8_t*)content2;
    data2.len = strlen(content2);
    data2.capacity = strlen(content2);

    write_result = opendal_writer_write(writer_result.writer, &data2);

    // Check if this is a OneShotWriter limitation
    if (write_result.error != NULL && write_result.error->message.data != NULL && strstr((char*)write_result.error->message.data, "OneShotWriter doesn't support multiple write") != NULL) {
        printf("Note: Service uses OneShotWriter, skipping multiple write test\n");

        // Close current writer and verify single write worked
        opendal_error* error = opendal_writer_close(writer_result.writer);
        OPENDAL_ASSERT_NO_ERROR(error, "Writer close should succeed");

        // Verify first write content
        opendal_result_read read_result = opendal_operator_read(ctx->config->operator_instance, path);
        OPENDAL_ASSERT_NO_ERROR(read_result.error, "Read should succeed");
        OPENDAL_ASSERT_EQ(strlen(content1), read_result.data.len,
            "Content length should match first write");
        OPENDAL_ASSERT(
            memcmp(content1, read_result.data.data, read_result.data.len) == 0,
            "Content should match first write");

        // Cleanup
        opendal_bytes_free(&read_result.data);
        opendal_writer_free(writer_result.writer);
        opendal_operator_delete(ctx->config->operator_instance, path);
        return;
    }

    OPENDAL_ASSERT_NO_ERROR(write_result.error, "Second write should succeed");
    OPENDAL_ASSERT_EQ(strlen(content2), write_result.size,
        "Write size should match content length");

    // Close writer
    opendal_error* error = opendal_writer_close(writer_result.writer);
    OPENDAL_ASSERT_NO_ERROR(error, "Writer close should succeed");

    // Verify written content
    opendal_result_read read_result = opendal_operator_read(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(read_result.error, "Read should succeed");

    size_t expected_len = strlen(content1) + strlen(content2);
    OPENDAL_ASSERT_EQ(expected_len, read_result.data.len,
        "Total content length should match");

    // Verify combined content
    char expected_content[100];
    snprintf(expected_content, sizeof(expected_content), "%s%s", content1,
        content2);
    OPENDAL_ASSERT(memcmp(expected_content, read_result.data.data,
                       read_result.data.len)
            == 0,
        "Combined content should match");

    // Cleanup
    opendal_bytes_free(&read_result.data);
    opendal_writer_free(writer_result.writer);
    opendal_operator_delete(ctx->config->operator_instance, path);
}

// Test: Writer with large data
void test_writer_large_data(opendal_test_context* ctx)
{
    const char* path = "test_writer_large.txt";
    const size_t chunk_size = 1024;
    const size_t num_chunks = 10;

    // Create writer
    opendal_result_operator_writer writer_result = opendal_operator_writer(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(writer_result.error,
        "Writer creation should succeed");

    // Generate and write chunks
    char* chunk_data = opendal_generate_random_content(chunk_size);
    OPENDAL_ASSERT_NOT_NULL(chunk_data, "Chunk data generation should succeed");

    opendal_bytes chunk;
    chunk.data = (uint8_t*)chunk_data;
    chunk.len = chunk_size;
    chunk.capacity = chunk_size;

    size_t total_written = 0;
    bool is_one_shot_writer = false;

    for (size_t i = 0; i < num_chunks; i++) {
        opendal_result_writer_write write_result = opendal_writer_write(writer_result.writer, &chunk);

        // Check for OneShotWriter limitation on subsequent writes
        if (i > 0 && write_result.error != NULL && write_result.error->message.data != NULL && strstr((char*)write_result.error->message.data, "OneShotWriter doesn't support multiple write") != NULL) {
            printf("Note: Service uses OneShotWriter, completed %zu write(s)\n", i);
            is_one_shot_writer = true;
            break;
        }

        OPENDAL_ASSERT_NO_ERROR(write_result.error, "Write should succeed");
        OPENDAL_ASSERT_EQ(chunk_size, write_result.size,
            "Write size should match chunk size");
        total_written += write_result.size;
    }

    // Close writer
    opendal_error* error = opendal_writer_close(writer_result.writer);
    OPENDAL_ASSERT_NO_ERROR(error, "Writer close should succeed");

    // Verify total size - adjust expectations for OneShotWriter
    opendal_result_stat stat_result = opendal_operator_stat(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(stat_result.error, "Stat should succeed");

    if (is_one_shot_writer) {
        // For OneShotWriter, we expect only one chunk to be written
        OPENDAL_ASSERT_EQ(chunk_size,
            opendal_metadata_content_length(stat_result.meta),
            "OneShotWriter should have written one chunk");
    } else {
        // For normal writers, we expect all chunks
        OPENDAL_ASSERT_EQ(chunk_size * num_chunks,
            opendal_metadata_content_length(stat_result.meta),
            "Total file size should match all chunks");
    }

    // Cleanup
    free(chunk_data);
    opendal_metadata_free(stat_result.meta);
    opendal_writer_free(writer_result.writer);
    opendal_operator_delete(ctx->config->operator_instance, path);
}

// Test: Reader partial read
void test_reader_partial_read(opendal_test_context* ctx)
{
    const char* path = "test_reader_partial.txt";
    const char* content = "0123456789ABCDEFGHIJ0123456789";
    size_t content_len = strlen(content);

    // Write test data
    opendal_bytes data;
    data.data = (uint8_t*)content;
    data.len = content_len;
    data.capacity = content_len;

    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    // Create reader
    opendal_result_operator_reader reader_result = opendal_operator_reader(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(reader_result.error,
        "Reader creation should succeed");

    // Read in small chunks
    const size_t chunk_size = 5;
    uint8_t buffer[chunk_size];
    size_t total_read = 0;

    while (total_read < content_len) {
        opendal_result_reader_read read_result = opendal_reader_read(reader_result.reader, buffer, chunk_size);
        OPENDAL_ASSERT_NO_ERROR(read_result.error, "Read should succeed");

        if (read_result.size == 0) {
            break; // EOF
        }

        // Verify chunk content
        OPENDAL_ASSERT(memcmp(content + total_read, buffer, read_result.size) == 0,
            "Chunk content should match");

        total_read += read_result.size;
    }

    OPENDAL_ASSERT_EQ(content_len, total_read,
        "Total read should match content length");

    // Cleanup
    opendal_reader_free(reader_result.reader);
    opendal_operator_delete(ctx->config->operator_instance, path);
}

// Test: Reader with options
void test_reader_with_options(opendal_test_context* ctx)
{
    const char* path = "test_reader_with_options.txt";
    const char* content = "Hello, OpenDAL Reader With Options!";
    size_t content_len = strlen(content);

    // Write test data first
    opendal_bytes data = {
        .data = (uint8_t*)content, .len = content_len, .capacity = content_len
    };

    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    // Build reader options exercising the concurrency knobs.
    opendal_reader_options* options = opendal_reader_options_new();
    OPENDAL_ASSERT_NOT_NULL(options, "Reader options should not be null");
    opendal_reader_options_set_concurrent(options, 2);
    opendal_reader_options_set_chunk(options, 1024 * 1024);
    opendal_reader_options_set_gap(options, 4096);
    opendal_reader_options_set_prefetch(options, 2);

    opendal_result_operator_reader reader_result
        = opendal_operator_reader_with(ctx->config->operator_instance, path, options);
    opendal_reader_options_free(options);
    OPENDAL_ASSERT_NO_ERROR(reader_result.error,
        "Reader-with-options creation should succeed");
    OPENDAL_ASSERT_NOT_NULL(reader_result.reader, "Reader should not be null");

    // A single read() may return a short count under chunked/concurrent reads,
    // so accumulate until the whole object has been read.
    uint8_t buffer[100];
    size_t total_read = 0;
    while (total_read < content_len) {
        opendal_result_reader_read read_result = opendal_reader_read(
            reader_result.reader, buffer + total_read, sizeof(buffer) - total_read);
        OPENDAL_ASSERT_NO_ERROR(read_result.error, "Read operation should succeed");
        if (read_result.size == 0) {
            break; // EOF
        }
        total_read += read_result.size;
    }
    OPENDAL_ASSERT_EQ(content_len, total_read,
        "Total read size should match content length");
    OPENDAL_ASSERT(memcmp(content, buffer, content_len) == 0,
        "Read content should match written content");

    // A NULL options pointer must behave like opendal_operator_reader.
    opendal_result_operator_reader default_result
        = opendal_operator_reader_with(ctx->config->operator_instance, path, NULL);
    OPENDAL_ASSERT_NO_ERROR(default_result.error,
        "Reader-with-NULL-options creation should succeed");
    OPENDAL_ASSERT_NOT_NULL(default_result.reader, "Reader should not be null");

    // Cleanup
    opendal_reader_free(reader_result.reader);
    opendal_reader_free(default_result.reader);
    opendal_operator_delete(ctx->config->operator_instance, path);
}

// Define the reader/writer test suite
opendal_test_case reader_writer_tests[] = {
    { "reader_basic", test_reader_basic, make_capability_read_write() },
    { "reader_seek", test_reader_seek, make_capability_read_write() },
    { "reader_with_options", test_reader_with_options,
        make_capability_read_write() },
    { "writer_basic", test_writer_basic, make_capability_read_write() },
    { "writer_large_data", test_writer_large_data, make_capability_write_stat() },
    { "reader_partial_read", test_reader_partial_read,
        make_capability_read_write() },
};

opendal_test_suite reader_writer_suite = {
    "Reader and Writer Operations", // name
    reader_writer_tests, // tests
    sizeof(reader_writer_tests) / sizeof(reader_writer_tests[0]) // test_count
};
