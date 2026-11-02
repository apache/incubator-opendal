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

#include "common.hpp"
#include "gtest/gtest.h"
#include <cstring>

extern "C" {
#include "opendal.h"
}

class OpendalListTest : public ::testing::Test {
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

// Basic use case of list
TEST_F(OpendalListTest, ListDirTest)
{
    std::string dname = "some_random_dir_name_152312";
    std::string fname = "some_random_file_name_21389";

    // 4 MiB of random bytes
    uintptr_t nbytes = 4 * 1024 * 1024;
    auto rand = generateRandomBytes(4 * 1024 * 1024);
    auto random_bytes = reinterpret_cast<uint8_t*>(rand.data());

    std::string path = dname + "/" + fname;
    opendal_bytes data = {
        .data = random_bytes,
        .len = nbytes,
    };

    // write must succeed
    EXPECT_EQ(opendal_operator_write(this->p, path.c_str(), &data),
        nullptr);

    // list must succeed since the write succeeded
    opendal_result_list l = opendal_operator_list(this->p, (dname + "/").c_str());
    EXPECT_EQ(l.error, nullptr);

    opendal_lister* lister = l.lister;

    // start checking the lister's result
    bool found = false;

    opendal_result_lister_next result = opendal_lister_next(lister);
    EXPECT_EQ(result.error, nullptr);
    opendal_entry* entry = result.entry;
    while (entry) {
        char* de_path = opendal_entry_path(entry);

        // stat must succeed
        opendal_result_stat s = opendal_operator_stat(this->p, de_path);
        EXPECT_EQ(s.error, nullptr);

        if (!strcmp(de_path, path.c_str())) {
            found = true;

            // the path we found has to be a file, and the length must be coherent
            EXPECT_TRUE(opendal_metadata_is_file(s.meta));
            EXPECT_EQ(opendal_metadata_content_length(s.meta), nbytes);
        }

        opendal_string_free(de_path);
        opendal_metadata_free(s.meta);
        opendal_entry_free(entry);

        result = opendal_lister_next(lister);
        EXPECT_EQ(result.error, nullptr);
        entry = result.entry;
    }

    // we must have found the file we wrote
    EXPECT_TRUE(found);

    // delete
    EXPECT_EQ(opendal_operator_delete(this->p, path.c_str()),
        nullptr);

    opendal_lister_free(lister);
}

// todo: Try list an empty directory
TEST_F(OpendalListTest, ListEmptyDirTest) { }

// todo: Try list a directory that does not exist
TEST_F(OpendalListTest, ListNotExistDirTest) { }
