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

// The Getting Started example for the C binding, embedded into the website
// guide. It runs against the in-memory service with no credentials, so CI can
// execute it directly. The region between the ANCHOR markers is what the docs
// show — keep it copy-pasteable.

// ANCHOR: quickstart
#include <assert.h>
#include <stdio.h>
#include "opendal.h"

int main(void)
{
    /* Build an operator for the "memory" service with no options. */
    opendal_result_operator_new result = opendal_operator_new("memory", NULL);
    assert(result.op != NULL);
    assert(result.error == NULL);
    opendal_operator *op = result.op;

    /* Write bytes to a path. */
    const char *msg = "Hello, OpenDAL!";
    opendal_bytes data = {
        .data = (uint8_t *)msg,
        .len  = 15,
    };
    opendal_error *err = opendal_operator_write(op, "/hello.txt", &data);
    assert(err == NULL);

    /* Read the bytes back. */
    opendal_result_read r = opendal_operator_read(op, "/hello.txt");
    assert(r.error == NULL);
    assert(r.data.len == 15);
    printf("%.*s\n", (int)r.data.len, r.data.data); /* Hello, OpenDAL! */
    opendal_bytes_free(&r.data);

    /* Stat the path to get metadata. */
    opendal_result_stat s = opendal_operator_stat(op, "/hello.txt");
    assert(s.error == NULL);
    printf("size = %llu\n", (unsigned long long)opendal_metadata_content_length(s.meta));
    opendal_metadata_free(s.meta);

    /* Delete the file. */
    opendal_error *del_err = opendal_operator_delete(op, "/hello.txt");
    assert(del_err == NULL);

    opendal_operator_free(op);
    return 0;
}
// ANCHOR_END: quickstart
