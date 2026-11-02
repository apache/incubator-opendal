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

class OpendalOperatorInfoTest : public ::testing::Test {
protected:
    opendal_operator* p;
    opendal_operator_info* info;
    std::string root;
    std::string scheme;

    // set up a brand new operator
    void SetUp() override
    {
        this->root = std::string("/myroot/");
        this->scheme = std::string("memory");

        opendal_operator_options* options = opendal_operator_options_new();
        opendal_operator_options_set(options, "root", this->root.c_str());

        opendal_result_operator_new result = opendal_operator_new(this->scheme.c_str(), options);
        EXPECT_TRUE(result.error == nullptr);

        this->p = result.op;
        EXPECT_TRUE(this->p);

        this->info = opendal_operator_info_new(this->p);
        EXPECT_TRUE(this->info);

        opendal_operator_options_free(options);
    }

    void TearDown() override
    {
        opendal_operator_free(this->p);
        opendal_operator_info_free(this->info);
    }
};

// We test the capability set by **memory** service.
TEST_F(OpendalOperatorInfoTest, CapabilityTest)
{
    opendal_capability cap = opendal_operator_info_get_capability(this->info);

    EXPECT_TRUE(cap.read);
    EXPECT_TRUE(cap.stat);
    EXPECT_TRUE(cap.write);
    EXPECT_TRUE(cap.write_can_empty);
    EXPECT_TRUE(cap.create_dir);
    EXPECT_TRUE(cap.delete_);
    EXPECT_TRUE(cap.list);
    EXPECT_TRUE(cap.list_with_recursive);
}

TEST_F(OpendalOperatorInfoTest, InfoTest)
{
    char *scheme, *root;
    scheme = opendal_operator_info_get_scheme(this->info);
    root = opendal_operator_info_get_root(this->info);

    EXPECT_TRUE(!strcmp(scheme, this->scheme.c_str()));
    EXPECT_TRUE(!strcmp(root, this->root.c_str()));

    // remember to free the strings
    opendal_string_free(scheme);
    opendal_string_free(root);
}
