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

class OpendalReaderTest : public ::testing::Test {
protected:
    const opendal_operator* p;

    // set up a brand new operator
    void SetUp() override
    {
        opendal_operator_options* options = opendal_operator_options_new();
        opendal_operator_options_set(options, "root", "/myroot");

        opendal_result_operator_new result = opendal_operator_new("memory", options);
        EXPECT_TRUE(result.error == nullptr);

        this->p = result.op;
        EXPECT_TRUE(this->p);

        opendal_operator_options_free(options);
    }

    void TearDown() override { opendal_operator_free(this->p); }
};

// Test seek
TEST_F(OpendalReaderTest, SeekTest)
{
    std::string payload = "ABCDEFGabcdefg1234567";
    opendal_bytes data = {
        .data = (uint8_t*)(payload.c_str()),
        .len = payload.length(),
    };

    // Prepare the file to be read immediately
    opendal_error* err = opendal_operator_write(this->p, "/testseek", &data);
    EXPECT_EQ(err, nullptr);

    opendal_result_operator_reader r = opendal_operator_reader(this->p, "/testseek");
    EXPECT_EQ(r.error, nullptr);

    //  Test seek set
    opendal_result_reader_seek seek_result = opendal_reader_seek(r.reader, 6, OPENDAL_SEEK_SET);
    EXPECT_EQ(seek_result.pos, 6);
    EXPECT_EQ(seek_result.error, nullptr);

    char buf1[64] = { 0 };
    opendal_result_reader_read read_result = opendal_reader_read(r.reader, (uint8_t*)buf1, 7);
    EXPECT_EQ(read_result.error, nullptr);
    EXPECT_EQ(read_result.size, 7);
    EXPECT_EQ(std::string(buf1), "Gabcdef");

    // Test seek cur, now we step on '3'
    seek_result = opendal_reader_seek(r.reader, 3, OPENDAL_SEEK_CUR);
    EXPECT_EQ(seek_result.pos, 16);
    EXPECT_EQ(seek_result.error, nullptr);

    char buf2[64] = { 0 };
    read_result = opendal_reader_read(r.reader, (uint8_t*)buf2, 32 /* no more 32 bytes*/);
    EXPECT_EQ(read_result.error, nullptr);
    EXPECT_EQ(read_result.size, 5);
    EXPECT_EQ(std::string(buf2), "34567");

    // Test seek end, now we step on 'g'
    seek_result = opendal_reader_seek(r.reader, -8, OPENDAL_SEEK_END);
    EXPECT_EQ(seek_result.pos, 13);
    EXPECT_EQ(seek_result.error, nullptr);

    char buf3[64] = { 0 };
    read_result = opendal_reader_read(r.reader, (uint8_t*)buf3, 32 /* no more 32 bytes*/);
    EXPECT_EQ(read_result.error, nullptr);
    EXPECT_EQ(read_result.size, 8);
    EXPECT_EQ(std::string(buf3), "g1234567");

    opendal_reader_free(r.reader);
}
