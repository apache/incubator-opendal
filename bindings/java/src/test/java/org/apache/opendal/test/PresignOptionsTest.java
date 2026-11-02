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

package org.apache.opendal.test;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.tuple;
import java.time.Duration;
import java.util.Collections;
import org.apache.hc.core5.http.NameValuePair;
import org.apache.hc.core5.net.URIBuilder;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.PresignedRequest;
import org.apache.opendal.ReadOptions;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.StatOptions;
import org.apache.opendal.WriteOptions;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;

@Timeout(10)
public class PresignOptionsTest {
    // Static test credentials let S3 sign locally without contacting a storage service.
    private static final ServiceConfig.S3 CONFIG = ServiceConfig.S3
            .builder()
            .bucket("opendal-test")
            .region("us-east-1")
            .endpoint("https://s3.us-east-1.amazonaws.com")
            .accessKeyId("opendal-test-access-key")
            .secretAccessKey("opendal-test-secret-key")
            .disableConfigLoad(true)
            .build();

    private static final Duration EXPIRY = Duration.ofMinutes(10);
    private static final String DISPOSITION = "attachment; filename=\"report 2026.csv\"";

    @Test
    public void testPresignReadOptions() throws Exception {
        try (final AsyncOperator op = AsyncOperator.of(CONFIG)) {
            final ReadOptions options = ReadOptions.builder()
                    .offset(2)
                    .length(4)
                    .overrideContentType("text/csv")
                    .overrideCacheControl("private, max-age=60")
                    .overrideContentDisposition(DISPOSITION)
                    .build();
            final PresignedRequest request =
                    op.presignRead("report.csv", EXPIRY, options).join();

            assertThat(request.getMethod()).isEqualTo("GET");
            assertThat(request.getHeaders()).containsEntry("range", "bytes=2-5");
            assertThat(new URIBuilder(request.getUri()).getQueryParams())
                    .extracting(NameValuePair::getName, NameValuePair::getValue)
                    .contains(
                            tuple("response-content-type", "text/csv"),
                            tuple("response-cache-control", "private, max-age=60"),
                            tuple("response-content-disposition", DISPOSITION),
                            tuple("X-Amz-Expires", "600"));
        }
    }

    @Test
    public void testPresignStatOptions() throws Exception {
        try (final AsyncOperator op = AsyncOperator.of(CONFIG)) {
            final StatOptions options = StatOptions.builder()
                    .version("version-1")
                    .ifMatch("\"etag\"")
                    .overrideContentType("text/csv")
                    .overrideCacheControl("private, max-age=60")
                    .overrideContentDisposition(DISPOSITION)
                    .build();
            final PresignedRequest request =
                    op.presignStat("report.csv", EXPIRY, options).join();

            assertThat(request.getMethod()).isEqualTo("HEAD");
            assertThat(request.getHeaders()).containsEntry("if-match", "\"etag\"");
            assertThat(new URIBuilder(request.getUri()).getQueryParams())
                    .extracting(NameValuePair::getName, NameValuePair::getValue)
                    .contains(
                            tuple("versionId", "version-1"),
                            tuple("response-content-type", "text/csv"),
                            tuple("response-cache-control", "private, max-age=60"),
                            tuple("response-content-disposition", DISPOSITION));
        }
    }

    @Test
    public void testPresignWriteOptions() {
        try (final AsyncOperator op = AsyncOperator.of(CONFIG)) {
            final WriteOptions options = WriteOptions.builder()
                    .contentType("text/csv")
                    .contentDisposition(DISPOSITION)
                    .cacheControl("private, max-age=60")
                    .userMetadata(Collections.singletonMap("source", "java"))
                    .build();
            final PresignedRequest request =
                    op.presignWrite("report.csv", EXPIRY, options).join();

            assertThat(request.getMethod()).isEqualTo("PUT");
            assertThat(request.getHeaders())
                    .containsEntry("content-type", "text/csv")
                    .containsEntry("content-disposition", DISPOSITION)
                    .containsEntry("cache-control", "private, max-age=60")
                    .containsEntry("x-amz-meta-source", "java");
        }
    }

    @Test
    public void testPresignWithoutOptions() throws Exception {
        try (final AsyncOperator op = AsyncOperator.of(CONFIG)) {
            final PresignedRequest read = op.presignRead("report.csv", EXPIRY).join();
            final PresignedRequest write = op.presignWrite("report.csv", EXPIRY).join();
            final PresignedRequest stat = op.presignStat("report.csv", EXPIRY).join();

            assertThat(read.getMethod()).isEqualTo("GET");
            assertThat(write.getMethod()).isEqualTo("PUT");
            assertThat(stat.getMethod()).isEqualTo("HEAD");
            for (final PresignedRequest request : new PresignedRequest[] {read, write, stat}) {
                assertThat(new URIBuilder(request.getUri()).getQueryParams())
                        .extracting(NameValuePair::getName)
                        .contains("X-Amz-Signature")
                        .doesNotContain(
                                "response-content-type", "response-cache-control", "response-content-disposition");
                assertThat(request.getHeaders()).doesNotContainKey("range");
            }
        }
    }
}
