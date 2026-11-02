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

#include "gtest/gtest.h"

extern "C" {
#include "opendal.h"
}

class OpendalBddTest : public ::testing::Test {
protected:
    const opendal_operator* p;

    std::string scheme;
    std::string path;
    std::string content;

    void SetUp() override
    {
        this->scheme = std::string("memory");
        this->path = std::string("test");
        this->content = std::string("Hello, World!");

        opendal_operator_options* options = opendal_operator_options_new();
        opendal_operator_options_set(options, "root", "/myroot");

        // Given A new OpenDAL Blocking Operator
        opendal_result_operator_new result = opendal_operator_new(scheme.c_str(), options);
        EXPECT_TRUE(result.error == nullptr);

        this->p = result.op;
        EXPECT_TRUE(this->p);

        opendal_operator_options_free(options);
    }

    void TearDown() override { opendal_operator_free(this->p); }
};

// Scenario: OpenDAL Blocking Operations
TEST_F(OpendalBddTest, FeatureTest)
{
    // When Blocking write path "test" with content "Hello, World!"
    const opendal_bytes data = {
        .data = (uint8_t*)this->content.c_str(),
        .len = this->content.length(),
    };
    opendal_error* error = opendal_operator_check(this->p);
    EXPECT_EQ(error, nullptr);

    error = opendal_operator_write(this->p, this->path.c_str(), &data);
    EXPECT_EQ(error, nullptr);

    // The blocking file "test" should exist
    opendal_result_exists e = opendal_operator_exists(this->p, this->path.c_str());
    EXPECT_EQ(e.error, nullptr);
    EXPECT_TRUE(e.exists);

    // The blocking file "test" entry mode must be file
    opendal_result_stat s = opendal_operator_stat(this->p, this->path.c_str());
    EXPECT_EQ(s.error, nullptr);
    opendal_metadata* meta = s.meta;
    EXPECT_TRUE(opendal_metadata_is_file(meta));

    // The blocking file "test" content length must be 13
    EXPECT_EQ(opendal_metadata_content_length(meta), 13);

    // the blocking file "test" last modified time must not be -1
    EXPECT_FALSE(opendal_metadata_last_modified_ms(meta) != -1);
    opendal_metadata_free(meta);

    // The blocking file "test" must have content "Hello, World!"
    struct opendal_result_read r = opendal_operator_read(this->p, this->path.c_str());
    EXPECT_EQ(r.error, nullptr);
    EXPECT_EQ(r.data.len, this->content.length());
    for (int i = 0; i < r.data.len; i++) {
        EXPECT_EQ(this->content[i], (char)(r.data.data[i]));
    }

    // The blocking file should be deleted
    error = opendal_operator_delete(this->p, this->path.c_str());
    EXPECT_EQ(error, nullptr);
    e = opendal_operator_exists(this->p, this->path.c_str());
    EXPECT_EQ(e.error, nullptr);
    EXPECT_FALSE(e.exists);

    opendal_result_operator_writer writer = opendal_operator_writer(this->p, this->path.c_str());
    EXPECT_EQ(writer.error, nullptr);
    opendal_result_writer_write w = opendal_writer_write(writer.writer, &data);
    EXPECT_EQ(w.error, nullptr);
    EXPECT_EQ(w.size, this->content.length());
    opendal_error* close_err = opendal_writer_close(writer.writer);
    EXPECT_EQ(close_err, nullptr);
    opendal_writer_free(writer.writer);

    // The blocking file "test" must have content "Hello, World!" and read into buffer
    int length = this->content.length();
    unsigned char buffer[this->content.length()];
    opendal_result_operator_reader reader = opendal_operator_reader(this->p, this->path.c_str());
    EXPECT_EQ(reader.error, nullptr);
    auto rst = opendal_reader_read(reader.reader, buffer, length);
    EXPECT_EQ(rst.size, length);
    for (int i = 0; i < this->content.length(); i++) {
        EXPECT_EQ(this->content[i], buffer[i]);
    }
    opendal_reader_free(reader.reader);

    // The deletion operation should be idempotent
    error = opendal_operator_delete(this->p, this->path.c_str());
    EXPECT_EQ(error, nullptr);

    opendal_bytes_free(&r.data);

    // The directory "tmpdir/" should exist and should be a directory
    error = opendal_operator_create_dir(this->p, "tmpdir/");
    EXPECT_EQ(error, nullptr);
    auto stat = opendal_operator_stat(this->p, "tmpdir/");
    EXPECT_EQ(stat.error, nullptr);
    EXPECT_TRUE(opendal_metadata_is_dir(stat.meta));
    EXPECT_FALSE(opendal_metadata_is_file(stat.meta));
    opendal_metadata_free(stat.meta);
    error = opendal_operator_delete(this->p, "tmpdir/");
    EXPECT_EQ(error, nullptr);
}

TEST_F(OpendalBddTest, WriteEmptyNullBytes)
{
    const opendal_bytes empty = {
        .data = nullptr,
        .len = 0,
        .capacity = 0,
    };

    opendal_error* error = opendal_operator_write(this->p, "empty", &empty);
    EXPECT_EQ(error, nullptr);

    opendal_result_read read = opendal_operator_read(this->p, "empty");
    EXPECT_EQ(read.error, nullptr);
    EXPECT_EQ(read.data.len, 0);
    opendal_bytes_free(&read.data);

    // Cover the separate write_with entry point that shares this validation.
    error = opendal_operator_write_with(this->p, "empty-with-options", &empty, nullptr);
    EXPECT_EQ(error, nullptr);

    read = opendal_operator_read(this->p, "empty-with-options");
    EXPECT_EQ(read.error, nullptr);
    EXPECT_EQ(read.data.len, 0);
    opendal_bytes_free(&read.data);

    opendal_result_operator_writer writer = opendal_operator_writer(this->p, "empty-writer");
    EXPECT_EQ(writer.error, nullptr);
    ASSERT_NE(writer.writer, nullptr);

    opendal_result_writer_write write = opendal_writer_write(writer.writer, &empty);
    EXPECT_EQ(write.error, nullptr);
    EXPECT_EQ(write.size, 0);

    error = opendal_writer_close(writer.writer);
    EXPECT_EQ(error, nullptr);
    opendal_writer_free(writer.writer);

    read = opendal_operator_read(this->p, "empty-writer");
    EXPECT_EQ(read.error, nullptr);
    EXPECT_EQ(read.data.len, 0);
    opendal_bytes_free(&read.data);

    EXPECT_EQ(opendal_operator_delete(this->p, "empty"), nullptr);
    EXPECT_EQ(opendal_operator_delete(this->p, "empty-with-options"), nullptr);
    EXPECT_EQ(opendal_operator_delete(this->p, "empty-writer"), nullptr);
}

TEST_F(OpendalBddTest, RejectNonEmptyNullBytes)
{
    const opendal_bytes invalid = {
        .data = nullptr,
        .len = 1,
        .capacity = 0,
    };

    opendal_error* error = opendal_operator_write(this->p, "invalid", &invalid);
    ASSERT_NE(error, nullptr);
    EXPECT_EQ(error->code, OPENDAL_UNEXPECTED);
    opendal_error_free(error);

    error = opendal_operator_write_with(this->p, "invalid-with-options", &invalid, nullptr);
    ASSERT_NE(error, nullptr);
    EXPECT_EQ(error->code, OPENDAL_UNEXPECTED);
    opendal_error_free(error);

    opendal_result_operator_writer writer = opendal_operator_writer(this->p, "invalid-writer");
    EXPECT_EQ(writer.error, nullptr);
    ASSERT_NE(writer.writer, nullptr);

    opendal_result_writer_write write = opendal_writer_write(writer.writer, &invalid);
    EXPECT_EQ(write.size, 0);
    ASSERT_NE(write.error, nullptr);
    EXPECT_EQ(write.error->code, OPENDAL_UNEXPECTED);
    opendal_error_free(write.error);
    opendal_writer_free(writer.writer);
}

TEST_F(OpendalBddTest, RejectEmptyNonNullBytes)
{
    uint8_t sentinel = 0;
    const opendal_bytes invalid = {
        .data = &sentinel,
        .len = 0,
        .capacity = 0,
    };

    opendal_error* error = opendal_operator_write(this->p, "invalid-empty", &invalid);
    ASSERT_NE(error, nullptr);
    EXPECT_EQ(error->code, OPENDAL_UNEXPECTED);
    opendal_error_free(error);

    error = opendal_operator_write_with(this->p, "invalid-empty-with-options", &invalid, nullptr);
    ASSERT_NE(error, nullptr);
    EXPECT_EQ(error->code, OPENDAL_UNEXPECTED);
    opendal_error_free(error);

    opendal_result_operator_writer writer = opendal_operator_writer(this->p, "invalid-empty-writer");
    EXPECT_EQ(writer.error, nullptr);
    ASSERT_NE(writer.writer, nullptr);

    opendal_result_writer_write write = opendal_writer_write(writer.writer, &invalid);
    EXPECT_EQ(write.size, 0);
    ASSERT_NE(write.error, nullptr);
    EXPECT_EQ(write.error->code, OPENDAL_UNEXPECTED);
    opendal_error_free(write.error);
    opendal_writer_free(writer.writer);
}
