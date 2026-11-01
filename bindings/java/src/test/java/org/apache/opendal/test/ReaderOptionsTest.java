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
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import com.sun.net.httpserver.Headers;
import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import org.apache.commons.io.IOUtils;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.Operator;
import org.apache.opendal.OperatorInputStream;
import org.apache.opendal.OperatorReader;
import org.apache.opendal.ReaderOptions;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;

/** Verifies Java-to-core option forwarding against recorded local HTTP requests. */
@Timeout(10)
public class ReaderOptionsTest {
    private static final byte[] CONTENT = "0123456789".getBytes(StandardCharsets.UTF_8);
    private final List<Headers> requests = new CopyOnWriteArrayList<>();
    private final List<URI> urls = new CopyOnWriteArrayList<>();
    private HttpServer server;
    private String endpoint;

    @BeforeEach
    void startServer() throws Exception {
        server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        server.createContext("/", exchange -> {
            try {
                requests.add(exchange.getRequestHeaders());
                urls.add(exchange.getRequestURI());
                if ("changed".equals(exchange.getRequestHeaders().getFirst("If-Match"))) {
                    exchange.sendResponseHeaders(412, -1);
                    return;
                }
                final String range = exchange.getRequestHeaders().getFirst("Range");
                final byte[] body;
                if (range != null) {
                    String[] bounds = range.substring("bytes=".length()).split("-", -1);
                    int start = Integer.parseInt(bounds[0]);
                    int end = bounds[1].isEmpty() ? CONTENT.length - 1 : Integer.parseInt(bounds[1]);
                    body = Arrays.copyOfRange(CONTENT, start, end + 1);
                    exchange.getResponseHeaders()
                            .set("Content-Range", "bytes " + start + "-" + end + "/" + CONTENT.length);
                } else {
                    body = CONTENT;
                }
                exchange.getResponseHeaders().set("ETag", "\"etag\"");
                exchange.sendResponseHeaders(range == null ? 200 : 206, body.length);
                exchange.getResponseBody().write(body);
            } finally {
                exchange.close();
            }
        });
        server.start();
        endpoint = "http://127.0.0.1:" + server.getAddress().getPort();
    }

    @AfterEach
    void stopServer() {
        server.stop(0);
    }

    @Test
    void testConditionsReachEveryChunkAndStream() throws Exception {
        ReaderOptions options = ReaderOptions.builder()
                .ifMatch("\"etag\"")
                .ifNoneMatch("\"other\"")
                .ifModifiedSince(Instant.parse("2024-01-01T00:00:00Z"))
                .ifUnmodifiedSince(Instant.parse("2024-01-02T00:00:00Z"))
                .chunk(2)
                .concurrent(2)
                .prefetch(1)
                .contentLengthHint(CONTENT.length)
                .build();
        try (Operator op = Operator.of(
                        ServiceConfig.Http.builder().endpoint(endpoint).build());
                OperatorReader reader = op.createReader("file", options)) {
            assertThat(reader.read(0, 4)).isEqualTo("0123".getBytes(StandardCharsets.UTF_8));
            try (OperatorInputStream in = reader.createInputStream(4, 4)) {
                assertThat(IOUtils.toByteArray(in)).isEqualTo("4567".getBytes(StandardCharsets.UTF_8));
            }
        }
        assertThat(requests).hasSize(4);
        for (Headers headers : requests) {
            assertThat(headers.getFirst("If-Match")).isEqualTo("\"etag\"");
            assertThat(headers.getFirst("If-None-Match")).isEqualTo("\"other\"");
            assertThat(headers.getFirst("If-Modified-Since")).isEqualTo("Mon, 01 Jan 2024 00:00:00 GMT");
            assertThat(headers.getFirst("If-Unmodified-Since")).isEqualTo("Tue, 02 Jan 2024 00:00:00 GMT");
        }
    }

    @Test
    void testVersionReachesReadsAndStreams() throws Exception {
        try (Operator op = Operator.of(ServiceConfig.S3
                        .builder()
                        .bucket("bucket")
                        .region("us-east-1")
                        .endpoint(endpoint)
                        .skipSignature(true)
                        .build());
                OperatorReader reader = op.createReader(
                        "file", ReaderOptions.builder().version("version-one").build())) {
            assertThat(reader.read(0, 2)).isEqualTo("01".getBytes(StandardCharsets.UTF_8));
            try (OperatorInputStream in = reader.createInputStream(4, 2)) {
                assertThat(IOUtils.toByteArray(in)).isEqualTo("45".getBytes(StandardCharsets.UTF_8));
            }
        }
        assertThat(urls).hasSize(2);
        for (URI uri : urls) {
            assertThat(uri.getQuery()).contains("versionId=version-one");
        }
    }

    @Test
    void testVersionConditionsReachReads() {
        try (Operator op = Operator.of(ServiceConfig.Gcs.builder()
                        .bucket("bucket")
                        .endpoint(endpoint)
                        .skipSignature(true)
                        .build());
                OperatorReader reader = op.createReader(
                        "file",
                        ReaderOptions.builder()
                                .ifVersionMatch("17")
                                .ifVersionNotMatch("18")
                                .build())) {
            assertThat(reader.read(0, 2)).isEqualTo("01".getBytes(StandardCharsets.UTF_8));
        }
        assertThat(urls).hasSize(1);
        assertThat(urls.get(0).getQuery()).contains("ifGenerationMatch=17", "ifGenerationNotMatch=18");
    }

    @Test
    void testConditionalErrorsKeepTheirCode() {
        try (Operator op = Operator.of(
                        ServiceConfig.Http.builder().endpoint(endpoint).build());
                OperatorReader reader = op.createReader(
                        "file", ReaderOptions.builder().ifMatch("changed").build())) {
            assertThatThrownBy(() -> reader.read(0, 2))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.ConditionNotMatch));
            try (OperatorInputStream in = reader.createInputStream(0, 2)) {
                assertThatThrownBy(in::read)
                        .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.ConditionNotMatch));
            }
        }
    }
}
