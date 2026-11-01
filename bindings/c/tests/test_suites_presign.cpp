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

typedef struct presign_header_context {
    size_t content_length;
    int content_length_found;
} presign_header_context;

typedef struct presign_body_context {
    const char* expected;
    size_t expected_len;
    size_t offset;
    int mismatch;
} presign_body_context;

typedef struct presign_upload_context {
    const char* data;
    size_t len;
    size_t offset;
} presign_upload_context;

static const size_t PRESIGN_NO_OVERRIDE = (size_t)(-1);

static int presign_seek_callback(void* userdata, curl_off_t offset, int origin)
{
    presign_upload_context* ctx = (presign_upload_context*)userdata;

    if (ctx == NULL) {
        return CURL_SEEKFUNC_CANTSEEK;
    }

    if (origin != SEEK_SET) {
        return CURL_SEEKFUNC_CANTSEEK;
    }

    if (offset < 0 || (size_t)offset > ctx->len) {
        return CURL_SEEKFUNC_FAIL;
    }

    ctx->offset = (size_t)offset;
    return CURL_SEEKFUNC_OK;
}

static size_t presign_header_callback(char* buffer, size_t size, size_t nmemb, void* userdata)
{
    size_t total = size * nmemb;
    presign_header_context* ctx = (presign_header_context*)userdata;
    const char header_name[] = "content-length:";
    size_t header_len = sizeof(header_name) - 1;

    if (ctx == NULL || buffer == NULL) {
        return total;
    }

    if (total >= header_len) {
        size_t i = 0;
        for (; i < header_len && i < total; i++) {
            char c = buffer[i];
            if (c >= 'A' && c <= 'Z') {
                c = (char)(c - 'A' + 'a');
            }
            if (c != header_name[i]) {
                break;
            }
        }

        if (i == header_len) {
            size_t pos = header_len;
            while (pos < total && (buffer[pos] == ' ' || buffer[pos] == '\t')) {
                pos++;
            }
            size_t end = pos;
            while (end < total && buffer[end] != '\r' && buffer[end] != '\n') {
                end++;
            }
            if (end > pos) {
                size_t value_len = end - pos;
                char value_buf[64];
                if (value_len >= sizeof(value_buf)) {
                    value_len = sizeof(value_buf) - 1;
                }
                memcpy(value_buf, buffer + pos, value_len);
                value_buf[value_len] = '\0';
                ctx->content_length = (size_t)strtoull(value_buf, NULL, 10);
                ctx->content_length_found = 1;
            }
        }
    }

    return total;
}

static size_t presign_write_callback(char* buffer, size_t size, size_t nmemb, void* userdata)
{
    size_t total = size * nmemb;
    presign_body_context* ctx = (presign_body_context*)userdata;

    if (ctx == NULL || buffer == NULL) {
        return total;
    }

    if (ctx->offset + total > ctx->expected_len) {
        ctx->mismatch = 1;
        ctx->offset += total;
        return total;
    }

    if (memcmp(ctx->expected + ctx->offset, buffer, total) != 0) {
        ctx->mismatch = 1;
    }

    ctx->offset += total;
    return total;
}

static size_t presign_upload_callback(char* buffer, size_t size, size_t nmemb, void* userdata)
{
    presign_upload_context* ctx = (presign_upload_context*)userdata;
    size_t max_write = size * nmemb;
    size_t remaining = 0;

    if (ctx == NULL || buffer == NULL) {
        return 0;
    }

    if (ctx->offset >= ctx->len) {
        return 0;
    }

    remaining = ctx->len - ctx->offset;
    if (remaining > max_write) {
        remaining = max_write;
    }

    memcpy(buffer, ctx->data + ctx->offset, remaining);
    ctx->offset += remaining;
    return remaining;
}

static size_t presign_sink_callback(char* buffer, size_t size, size_t nmemb, void* userdata)
{
    (void)buffer;
    (void)userdata;
    return size * nmemb;
}

static int presign_str_ieq(const char* a, const char* b)
{
    if (a == NULL || b == NULL) {
        return 0;
    }

    while (*a != '\0' && *b != '\0') {
        char ca = *a;
        char cb = *b;
        if (ca >= 'A' && ca <= 'Z') {
            ca = (char)(ca - 'A' + 'a');
        }
        if (cb >= 'A' && cb <= 'Z') {
            cb = (char)(cb - 'A' + 'a');
        }
        if (ca != cb) {
            return 0;
        }
        a++;
        b++;
    }

    return *a == '\0' && *b == '\0';
}

static int presign_build_header_list(const opendal_http_header_pair* headers,
    uintptr_t headers_len,
    size_t override_content_len,
    struct curl_slist** out)
{
    struct curl_slist* chunk = NULL;

    if (headers == NULL || headers_len == 0) {
        *out = NULL;
        return 1;
    }

    for (uintptr_t i = 0; i < headers_len; i++) {
        const char* key = headers[i].key;
        const char* value = headers[i].value;
        if (key == NULL || value == NULL) {
            if (chunk != NULL) {
                curl_slist_free_all(chunk);
            }
            *out = NULL;
            return 0;
        }
        size_t key_len = strlen(key);
        size_t value_len = strlen(value);
        const char* value_to_use = value;
        char length_override[32];
        if (override_content_len != PRESIGN_NO_OVERRIDE && presign_str_ieq(key, "content-length")) {
            int written = snprintf(length_override, sizeof(length_override), "%zu", override_content_len);
            if (written <= 0 || (size_t)written >= sizeof(length_override)) {
                if (chunk != NULL) {
                    curl_slist_free_all(chunk);
                }
                *out = NULL;
                return 0;
            }
            value_to_use = length_override;
            value_len = (size_t)written;
        }
        size_t header_len = key_len + 2 + value_len;
        char* header_line = (char*)malloc(header_len + 1);
        if (header_line == NULL) {
            if (chunk != NULL) {
                curl_slist_free_all(chunk);
            }
            *out = NULL;
            return 0;
        }
        memcpy(header_line, key, key_len);
        header_line[key_len] = ':';
        header_line[key_len + 1] = ' ';
        memcpy(header_line + key_len + 2, value_to_use, value_len);
        header_line[header_len] = '\0';
        struct curl_slist* new_chunk = curl_slist_append(chunk, header_line);
        free(header_line);
        if (new_chunk == NULL) {
            if (chunk != NULL) {
                curl_slist_free_all(chunk);
            }
            *out = NULL;
            return 0;
        }
        chunk = new_chunk;
    }

    *out = chunk;
    return 1;
}

static void presign_cleanup_curl(CURL* curl, struct curl_slist* chunk)
{
    if (chunk != NULL) {
        curl_slist_free_all(chunk);
    }
    if (curl != NULL) {
        curl_easy_cleanup(curl);
    }
}

static CURLcode presign_set_standard_options(CURL* curl, const char* url, const char* method)
{
    CURLcode opt_res = curl_easy_setopt(curl, CURLOPT_URL, url);
    if (opt_res != CURLE_OK) {
        return opt_res;
    }

    opt_res = curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    if (opt_res != CURLE_OK) {
        return opt_res;
    }

    if (method != NULL) {
        if (presign_str_ieq(method, "GET")) {
            opt_res = curl_easy_setopt(curl, CURLOPT_HTTPGET, 1L);
        } else if (presign_str_ieq(method, "HEAD")) {
            opt_res = curl_easy_setopt(curl, CURLOPT_NOBODY, 1L);
            if (opt_res != CURLE_OK) {
                return opt_res;
            }
            opt_res = curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, method);
        } else if (presign_str_ieq(method, "POST")) {
            opt_res = curl_easy_setopt(curl, CURLOPT_POST, 1L);
            if (opt_res != CURLE_OK) {
                return opt_res;
            }
            opt_res = curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, method);
        } else {
            opt_res = curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, method);
        }
    }

    return opt_res;
}

typedef enum presign_prepare_result {
    PRESIGN_PREPARE_OK = 0,
    PRESIGN_PREPARE_BUILD_HEADERS_FAIL,
    PRESIGN_PREPARE_STANDARD_OPTIONS_FAIL,
    PRESIGN_PREPARE_SET_HEADERS_FAIL,
} presign_prepare_result;

static presign_prepare_result presign_prepare_curl(CURL* curl,
    const char* url,
    const char* method,
    const opendal_http_header_pair* headers,
    uintptr_t headers_len,
    size_t override_content_len,
    struct curl_slist** out_chunk,
    CURLcode* out_error)
{
    if (out_chunk == NULL) {
        if (out_error != NULL) {
            *out_error = CURLE_FAILED_INIT;
        }
        return PRESIGN_PREPARE_STANDARD_OPTIONS_FAIL;
    }

    *out_chunk = NULL;

    if (!presign_build_header_list(headers, headers_len, override_content_len, out_chunk)) {
        if (*out_chunk != NULL) {
            curl_slist_free_all(*out_chunk);
            *out_chunk = NULL;
        }
        if (out_error != NULL) {
            *out_error = CURLE_OK;
        }
        return PRESIGN_PREPARE_BUILD_HEADERS_FAIL;
    }

    CURLcode opt_res = presign_set_standard_options(curl, url, method);
    if (opt_res != CURLE_OK) {
        if (*out_chunk != NULL) {
            curl_slist_free_all(*out_chunk);
            *out_chunk = NULL;
        }
        if (out_error != NULL) {
            *out_error = opt_res;
        }
        return PRESIGN_PREPARE_STANDARD_OPTIONS_FAIL;
    }

    if (*out_chunk != NULL) {
        opt_res = curl_easy_setopt(curl, CURLOPT_HTTPHEADER, *out_chunk);
        if (opt_res != CURLE_OK) {
            curl_slist_free_all(*out_chunk);
            *out_chunk = NULL;
            if (out_error != NULL) {
                *out_error = opt_res;
            }
            return PRESIGN_PREPARE_SET_HEADERS_FAIL;
        }
    }

    if (out_error != NULL) {
        *out_error = CURLE_OK;
    }
    return PRESIGN_PREPARE_OK;
}

#define PRESIGN_ASSERT_PREPARE_OK(curl_handle, chunk_handle, prepare_result, prepare_error) \
    do { \
        if ((prepare_result) != PRESIGN_PREPARE_OK) { \
            presign_cleanup_curl((curl_handle), (chunk_handle)); \
            if ((prepare_result) == PRESIGN_PREPARE_BUILD_HEADERS_FAIL) { \
                OPENDAL_ASSERT(0, "Building CURL headers should succeed"); \
            } else if ((prepare_result) == PRESIGN_PREPARE_STANDARD_OPTIONS_FAIL) { \
                OPENDAL_ASSERT_EQ(CURLE_OK, (prepare_error), "Setting standard CURL options should succeed"); \
            } else { \
                OPENDAL_ASSERT_EQ(CURLE_OK, (prepare_error), "Setting CURL headers should succeed"); \
            } \
        } \
    } while (0)

// Test: Presign read operation
void test_presign_read(opendal_test_context* ctx)
{
    const char* path = "test_presign.txt";
    const char* content = "Presign test content";

    opendal_bytes data;
    data.data = (uint8_t*)content;
    data.len = strlen(content);
    data.capacity = strlen(content);

    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    opendal_result_presign presign_result = opendal_operator_presign_read(ctx->config->operator_instance, path, 3600);
    OPENDAL_ASSERT_NO_ERROR(presign_result.error, "Presign read should succeed");
    OPENDAL_ASSERT_NOT_NULL(presign_result.req, "Presigned request should not be null");

    const char* method = opendal_presigned_request_method(presign_result.req);
    const char* url = opendal_presigned_request_uri(presign_result.req);
    const opendal_http_header_pair* headers = opendal_presigned_request_headers(presign_result.req);
    uintptr_t headers_len = opendal_presigned_request_headers_len(presign_result.req);
    OPENDAL_ASSERT_NOT_NULL(method, "Presigned method should not be null");
    OPENDAL_ASSERT_STR_EQ("GET", method, "Presigned method should be GET");
    OPENDAL_ASSERT_NOT_NULL(url, "Presigned URL should not be null");
    OPENDAL_ASSERT(headers_len == 0 || headers != NULL, "Headers pointer must be valid when headers exist");

    CURL* curl = curl_easy_init();
    OPENDAL_ASSERT_NOT_NULL(curl, "CURL initialization should succeed");

    size_t expected_len = strlen(content);
    struct curl_slist* chunk = NULL;
    CURLcode setup_error = CURLE_OK;
    presign_prepare_result prepare_res = presign_prepare_curl(curl, url, method,
        headers, headers_len, PRESIGN_NO_OVERRIDE, &chunk, &setup_error);
    PRESIGN_ASSERT_PREPARE_OK(curl, chunk, prepare_res, setup_error);

    CURLcode opt_res = CURLE_OK;

    presign_header_context header_ctx;
    header_ctx.content_length = 0;
    header_ctx.content_length_found = 0;
    opt_res = curl_easy_setopt(curl, CURLOPT_HEADERFUNCTION, presign_header_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL header callback should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_HEADERDATA, &header_ctx);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL header data should succeed");
    }

    presign_body_context body_ctx;
    body_ctx.expected = content;
    body_ctx.expected_len = expected_len;
    body_ctx.offset = 0;
    body_ctx.mismatch = 0;
    opt_res = curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, presign_write_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL write callback should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_WRITEDATA, &body_ctx);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL write data should succeed");
    }

    CURLcode res = curl_easy_perform(curl);
    if (res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, res, "CURL perform should succeed");
    }

    presign_cleanup_curl(curl, chunk);

    int header_found = header_ctx.content_length_found;
    size_t header_length = header_ctx.content_length;
    size_t received_len = body_ctx.offset;
    int mismatch = body_ctx.mismatch;

    opendal_presigned_request_free(presign_result.req);

    opendal_error* delete_error = opendal_operator_delete(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(delete_error, "Cleanup delete should succeed");

    OPENDAL_ASSERT(header_found, "Content-Length header should be present");
    OPENDAL_ASSERT_EQ(expected_len, header_length,
        "Content-Length header should match object size");
    OPENDAL_ASSERT_EQ(expected_len, received_len,
        "Downloaded data length should match object size");
    OPENDAL_ASSERT(mismatch == 0, "Downloaded content should match stored content");
}

// Test: Presign read operation with options
void test_presign_read_with_range(opendal_test_context* ctx)
{
    const char* path = "test_presign_read_with_range.txt";
    const char* content = "0123456789";
    const char* expected = "2345";
    size_t content_len = strlen(content);
    size_t expected_len = strlen(expected);

    opendal_bytes data;
    data.data = (uint8_t*)content;
    data.len = content_len;
    data.capacity = content_len;

    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    opendal_read_options* opts = opendal_read_options_new();
    OPENDAL_ASSERT_NOT_NULL(opts, "Read options should not be null");
    opendal_read_options_set_range(opts, 2, expected_len);

    opendal_result_presign presign_result = opendal_operator_presign_read_with(ctx->config->operator_instance, path, 3600, opts);
    opendal_read_options_free(opts);
    OPENDAL_ASSERT_NO_ERROR(presign_result.error, "Presign read with range should succeed");
    OPENDAL_ASSERT_NOT_NULL(presign_result.req, "Presigned request should not be null");

    const char* method = opendal_presigned_request_method(presign_result.req);
    const char* url = opendal_presigned_request_uri(presign_result.req);
    const opendal_http_header_pair* headers = opendal_presigned_request_headers(presign_result.req);
    uintptr_t headers_len = opendal_presigned_request_headers_len(presign_result.req);
    OPENDAL_ASSERT_NOT_NULL(method, "Presigned method should not be null");
    OPENDAL_ASSERT_STR_EQ("GET", method, "Presigned method should be GET");
    OPENDAL_ASSERT_NOT_NULL(url, "Presigned URL should not be null");
    OPENDAL_ASSERT(headers_len == 0 || headers != NULL, "Headers pointer must be valid when headers exist");

    CURL* curl = curl_easy_init();
    OPENDAL_ASSERT_NOT_NULL(curl, "CURL initialization should succeed");

    struct curl_slist* chunk = NULL;
    CURLcode setup_error = CURLE_OK;
    presign_prepare_result prepare_res = presign_prepare_curl(curl, url, method,
        headers, headers_len, PRESIGN_NO_OVERRIDE, &chunk, &setup_error);
    PRESIGN_ASSERT_PREPARE_OK(curl, chunk, prepare_res, setup_error);

    presign_body_context body_ctx;
    body_ctx.expected = expected;
    body_ctx.expected_len = expected_len;
    body_ctx.offset = 0;
    body_ctx.mismatch = 0;

    CURLcode opt_res = curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, presign_write_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL write callback should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_WRITEDATA, &body_ctx);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL write data should succeed");
    }

    CURLcode res = curl_easy_perform(curl);
    if (res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, res, "CURL perform should succeed");
    }

    presign_cleanup_curl(curl, chunk);

    size_t received_len = body_ctx.offset;
    int mismatch = body_ctx.mismatch;

    opendal_presigned_request_free(presign_result.req);

    opendal_error* delete_error = opendal_operator_delete(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(delete_error, "Cleanup delete should succeed");

    OPENDAL_ASSERT_EQ(expected_len, received_len,
        "Downloaded range length should match requested range");
    OPENDAL_ASSERT(mismatch == 0, "Downloaded range content should match");
}

// Test: Presign write operation
void test_presign_write(opendal_test_context* ctx)
{
    const char* path = "test_presign_write.txt";
    const char* content = "Presign write content";
    size_t content_len = strlen(content);

    opendal_result_presign presign_result = opendal_operator_presign_write(ctx->config->operator_instance, path, 3600);
    OPENDAL_ASSERT_NO_ERROR(presign_result.error, "Presign write should succeed");
    OPENDAL_ASSERT_NOT_NULL(presign_result.req, "Presigned request should not be null");

    const char* method = opendal_presigned_request_method(presign_result.req);
    const char* url = opendal_presigned_request_uri(presign_result.req);
    const opendal_http_header_pair* headers = opendal_presigned_request_headers(presign_result.req);
    uintptr_t headers_len = opendal_presigned_request_headers_len(presign_result.req);
    OPENDAL_ASSERT_NOT_NULL(method, "Presigned method should not be null");
    OPENDAL_ASSERT_NOT_NULL(url, "Presigned URL should not be null");
    OPENDAL_ASSERT(headers_len == 0 || headers != NULL, "Headers pointer must be valid when headers exist");

    CURL* curl = curl_easy_init();
    OPENDAL_ASSERT_NOT_NULL(curl, "CURL initialization should succeed");

    struct curl_slist* chunk = NULL;
    CURLcode setup_error = CURLE_OK;
    presign_prepare_result prepare_res = presign_prepare_curl(curl, url, method,
        headers, headers_len, content_len, &chunk, &setup_error);
    PRESIGN_ASSERT_PREPARE_OK(curl, chunk, prepare_res, setup_error);

    CURLcode opt_res = CURLE_OK;

    presign_upload_context upload_ctx;
    upload_ctx.data = content;
    upload_ctx.len = content_len;
    upload_ctx.offset = 0;

    opt_res = curl_easy_setopt(curl, CURLOPT_UPLOAD, 1L);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Enabling CURL upload should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_READFUNCTION, presign_upload_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL read callback should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_READDATA, &upload_ctx);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL read data should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_SEEKFUNCTION, presign_seek_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL seek callback should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_SEEKDATA, &upload_ctx);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL seek data should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_INFILESIZE_LARGE, (curl_off_t)content_len);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL upload size should succeed");
    }

    opt_res = curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, presign_sink_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL sink callback should succeed");
    }

    CURLcode res = curl_easy_perform(curl);
    if (res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, res, "CURL perform should succeed");
    }

    presign_cleanup_curl(curl, chunk);
    opendal_presigned_request_free(presign_result.req);

    opendal_result_stat stat_res = opendal_operator_stat(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(stat_res.error, "Stat after presign write should succeed");
    OPENDAL_ASSERT_NOT_NULL(stat_res.meta, "Stat metadata should not be null");
    OPENDAL_ASSERT_EQ(content_len, (size_t)opendal_metadata_content_length(stat_res.meta),
        "Stat size should match uploaded content length");

    opendal_metadata_free(stat_res.meta);

    opendal_result_read read_res = opendal_operator_read(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(read_res.error, "Read after presign write should succeed");
    OPENDAL_ASSERT_EQ(content_len, read_res.data.len, "Read length should match uploaded content length");
    OPENDAL_ASSERT(memcmp(content, read_res.data.data, read_res.data.len) == 0,
        "Read content should match uploaded content");
    opendal_bytes_free(&read_res.data);

    opendal_error* delete_error = opendal_operator_delete(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(delete_error, "Cleanup delete should succeed");
}

// Test: Presign stat operation
void test_presign_stat(opendal_test_context* ctx)
{
    const char* path = "test_presign_stat.txt";
    const char* content = "Presign stat content";
    size_t content_len = strlen(content);

    opendal_bytes data;
    data.data = (uint8_t*)content;
    data.len = content_len;
    data.capacity = content_len;
    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    opendal_result_presign presign_result = opendal_operator_presign_stat(ctx->config->operator_instance, path, 3600);
    OPENDAL_ASSERT_NO_ERROR(presign_result.error, "Presign stat should succeed");
    OPENDAL_ASSERT_NOT_NULL(presign_result.req, "Presigned request should not be null");

    const char* method = opendal_presigned_request_method(presign_result.req);
    const char* url = opendal_presigned_request_uri(presign_result.req);
    const opendal_http_header_pair* headers = opendal_presigned_request_headers(presign_result.req);
    uintptr_t headers_len = opendal_presigned_request_headers_len(presign_result.req);
    OPENDAL_ASSERT_NOT_NULL(method, "Presigned method should not be null");
    OPENDAL_ASSERT_NOT_NULL(url, "Presigned URL should not be null");

    CURL* curl = curl_easy_init();
    OPENDAL_ASSERT_NOT_NULL(curl, "CURL initialization should succeed");

    struct curl_slist* chunk = NULL;
    CURLcode setup_error = CURLE_OK;
    presign_prepare_result prepare_res = presign_prepare_curl(curl, url, method,
        headers, headers_len, PRESIGN_NO_OVERRIDE, &chunk, &setup_error);
    PRESIGN_ASSERT_PREPARE_OK(curl, chunk, prepare_res, setup_error);

    CURLcode opt_res = CURLE_OK;

    presign_header_context header_ctx;
    header_ctx.content_length = 0;
    header_ctx.content_length_found = 0;
    opt_res = curl_easy_setopt(curl, CURLOPT_HEADERFUNCTION, presign_header_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL header callback should succeed");
    }
    opt_res = curl_easy_setopt(curl, CURLOPT_HEADERDATA, &header_ctx);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL header data should succeed");
    }

    opt_res = curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, presign_sink_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL sink callback should succeed");
    }

    CURLcode res = curl_easy_perform(curl);
    if (res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, res, "CURL perform should succeed");
    }

    long response_code = 0;
    CURLcode info_res = curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &response_code);
    if (info_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, info_res, "Retrieving response code should succeed");
    }

    presign_cleanup_curl(curl, chunk);
    opendal_presigned_request_free(presign_result.req);

    OPENDAL_ASSERT_EQ(200, response_code, "Presign stat should return 200 status");
    OPENDAL_ASSERT(header_ctx.content_length_found, "Stat response should include Content-Length");
    OPENDAL_ASSERT_EQ(content_len, header_ctx.content_length,
        "Stat Content-Length should match written data");

    opendal_error* delete_error = opendal_operator_delete(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(delete_error, "Cleanup delete should succeed");
}

// Test: Presign delete operation
void test_presign_delete(opendal_test_context* ctx)
{
    opendal_operator_info* info = opendal_operator_info_new(ctx->config->operator_instance);
    OPENDAL_ASSERT_NOT_NULL(info, "Operator info should not be null");
    opendal_capability cap = opendal_operator_info_get_capability(info);
    opendal_operator_info_free(info);
    if (!cap.presign_delete) {
        return;
    }
    const char* path = "test_presign_delete.txt";
    const char* content = "Presign delete content";
    size_t content_len = strlen(content);

    opendal_bytes data;
    data.data = (uint8_t*)content;
    data.len = content_len;
    data.capacity = content_len;
    opendal_error* error = opendal_operator_write(ctx->config->operator_instance, path, &data);
    OPENDAL_ASSERT_NO_ERROR(error, "Write operation should succeed");

    opendal_result_presign presign_result = opendal_operator_presign_delete(ctx->config->operator_instance, path, 3600);
    OPENDAL_ASSERT_NO_ERROR(presign_result.error, "Presign delete should succeed");
    OPENDAL_ASSERT_NOT_NULL(presign_result.req, "Presigned request should not be null");

    const char* method = opendal_presigned_request_method(presign_result.req);
    const char* url = opendal_presigned_request_uri(presign_result.req);
    const opendal_http_header_pair* headers = opendal_presigned_request_headers(presign_result.req);
    uintptr_t headers_len = opendal_presigned_request_headers_len(presign_result.req);
    OPENDAL_ASSERT_NOT_NULL(method, "Presigned method should not be null");
    OPENDAL_ASSERT_NOT_NULL(url, "Presigned URL should not be null");

    CURL* curl = curl_easy_init();
    OPENDAL_ASSERT_NOT_NULL(curl, "CURL initialization should succeed");

    struct curl_slist* chunk = NULL;
    CURLcode setup_error = CURLE_OK;
    presign_prepare_result prepare_res = presign_prepare_curl(curl, url, method,
        headers, headers_len, PRESIGN_NO_OVERRIDE, &chunk, &setup_error);
    PRESIGN_ASSERT_PREPARE_OK(curl, chunk, prepare_res, setup_error);

    CURLcode opt_res = curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, presign_sink_callback);
    if (opt_res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, opt_res, "Setting CURL sink callback should succeed");
    }

    CURLcode res = curl_easy_perform(curl);
    if (res != CURLE_OK) {
        presign_cleanup_curl(curl, chunk);
        OPENDAL_ASSERT_EQ(CURLE_OK, res, "CURL perform should succeed");
    }

    presign_cleanup_curl(curl, chunk);
    opendal_presigned_request_free(presign_result.req);

    opendal_result_exists exists_res = opendal_operator_exists(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(exists_res.error, "Exists after presign delete should succeed");
    OPENDAL_ASSERT(!exists_res.exists, "Object should not exist after presign delete");

    // Ensure cleanup is idempotent
    opendal_error* delete_error = opendal_operator_delete(ctx->config->operator_instance, path);
    OPENDAL_ASSERT_NO_ERROR(delete_error, "Delete after presign delete should be idempotent");
}

opendal_test_case presign_tests[] = {
    { "presign_read", test_presign_read, make_capability_presign() },
    { "presign_read_with_range", test_presign_read_with_range, make_capability_presign() },
    { "presign_write", test_presign_write, make_capability_presign() },
    { "presign_stat", test_presign_stat, make_capability_presign() },
    { "presign_delete", test_presign_delete, make_capability_presign() },
};

opendal_test_suite presign_suite = {
    "Presign Operations",
    presign_tests,
    sizeof(presign_tests) / sizeof(presign_tests[0]),
};
