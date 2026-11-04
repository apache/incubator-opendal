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
import java.lang.reflect.Field;
import java.net.InetSocketAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.stream.Stream;
import org.apache.commons.io.IOUtils;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.Operator;
import org.apache.opendal.OperatorInputStream;
import org.apache.opendal.ReadOptions;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.Arguments;
import org.junit.jupiter.params.provider.MethodSource;
import org.junit.jupiter.params.provider.ValueSource;

/** Verifies option forwarding through blocking, async, and compatibility stream reads. */
@Timeout(10)
public class ReadOptionsTest {
    private static final byte[] CONTENT = "0123456789".getBytes(StandardCharsets.UTF_8);
    private final List<Headers> requests = new CopyOnWriteArrayList<>();
    private final List<String> methods = new CopyOnWriteArrayList<>();
    private final List<URI> urls = new CopyOnWriteArrayList<>();
    private HttpServer server;
    private String endpoint;

    private void startServer() throws Exception {
        server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        server.createContext("/", exchange -> {
            try {
                requests.add(exchange.getRequestHeaders());
                methods.add(exchange.getRequestMethod());
                urls.add(exchange.getRequestURI());
                if ("changed".equals(exchange.getRequestHeaders().getFirst("If-Match"))) {
                    exchange.sendResponseHeaders(412, -1);
                    return;
                }
                if ("HEAD".equals(exchange.getRequestMethod())) {
                    exchange.getResponseHeaders().set("Content-Length", Integer.toString(CONTENT.length));
                    exchange.sendResponseHeaders(200, -1);
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
        if (server != null) {
            server.stop(0);
        }
    }

    @Test
    void testConditionsAndRangeReachEveryChunk() throws Exception {
        startServer();
        ReadOptions options = ReadOptions.builder()
                .offset(2)
                .length(4)
                .ifMatch("\"etag\"")
                .ifNoneMatch("\"other\"")
                .ifModifiedSince(Instant.parse("2024-01-01T00:00:00Z"))
                .ifUnmodifiedSince(Instant.parse("2024-01-02T00:00:00Z"))
                .chunk(2)
                .concurrent(2)
                .contentLengthHint(CONTENT.length)
                .build();
        try (AsyncOperator async = AsyncOperator.of(
                        ServiceConfig.Http.builder().endpoint(endpoint).build());
                Operator op = async.blocking()) {
            assertReads(op, async, "file", options, "2345");
        }
        assertThat(methods).containsOnly("GET").hasSize(6);
        assertThat(requests)
                .extracting(headers -> headers.getFirst("Range"))
                .containsExactlyInAnyOrder(
                        "bytes=2-3", "bytes=4-5", "bytes=2-3", "bytes=4-5", "bytes=2-3", "bytes=4-5");
        for (Headers headers : requests) {
            assertThat(headers.getFirst("If-Match")).isEqualTo("\"etag\"");
            assertThat(headers.getFirst("If-None-Match")).isEqualTo("\"other\"");
            assertThat(headers.getFirst("If-Modified-Since")).isEqualTo("Mon, 01 Jan 2024 00:00:00 GMT");
            assertThat(headers.getFirst("If-Unmodified-Since")).isEqualTo("Tue, 02 Jan 2024 00:00:00 GMT");
        }
    }

    @ParameterizedTest
    @ValueSource(longs = {-1, 10})
    void testUnboundedChunkedReadsUseContentLengthHint(long hint) throws Exception {
        startServer();
        ReadOptions options = ReadOptions.builder()
                .chunk(2)
                .concurrent(2)
                .contentLengthHint(hint)
                .build();
        try (AsyncOperator async = AsyncOperator.of(
                        ServiceConfig.Http.builder().endpoint(endpoint).build());
                Operator op = async.blocking()) {
            assertThat(op.read("file", options)).isEqualTo(CONTENT);
            assertThat(methods.stream().filter("HEAD"::equals).count()).isEqualTo(hint == -1 ? 1 : 0);
            assertThat(async.read("file", options).join()).isEqualTo(CONTENT);
            assertThat(methods.stream().filter("HEAD"::equals).count()).isEqualTo(hint == -1 ? 2 : 0);
            try (OperatorInputStream in = op.createInputStream("file", options)) {
                assertThat(IOUtils.toByteArray(in)).isEqualTo(CONTENT);
            }
        }
        assertThat(methods.stream().filter("GET"::equals).count()).isEqualTo(15);
        assertThat(methods.stream().filter("HEAD"::equals).count()).isEqualTo(hint == -1 ? 3 : 0);
        for (String range : Arrays.asList("bytes=0-1", "bytes=2-3", "bytes=4-5", "bytes=6-7", "bytes=8-9")) {
            assertThat(requests.stream()
                            .filter(headers -> range.equals(headers.getFirst("Range")))
                            .count())
                    .isEqualTo(3);
        }
    }

    @Test
    void testVersionSelection() throws Exception {
        startServer();
        try (AsyncOperator async = AsyncOperator.of(ServiceConfig.S3
                        .builder()
                        .bucket("bucket")
                        .region("us-east-1")
                        .endpoint(endpoint)
                        .skipSignature(true)
                        .build());
                Operator op = async.blocking()) {
            assertReads(
                    op,
                    async,
                    "file",
                    ReadOptions.builder().version("version-one").length(2).build(),
                    "01");
        }
        assertThat(urls).hasSize(3);
        for (URI uri : urls) {
            assertThat(uri.getQuery()).contains("versionId=version-one");
        }
    }

    @Test
    void testVersionConditions() throws Exception {
        startServer();
        try (AsyncOperator async = AsyncOperator.of(ServiceConfig.Gcs.builder()
                        .bucket("bucket")
                        .endpoint(endpoint)
                        .skipSignature(true)
                        .build());
                Operator op = async.blocking()) {
            assertReads(
                    op,
                    async,
                    "file",
                    ReadOptions.builder()
                            .ifVersionMatch("17")
                            .ifVersionNotMatch("18")
                            .length(2)
                            .build(),
                    "01");
        }
        assertThat(urls).hasSize(3);
        for (URI uri : urls) {
            assertThat(uri.getQuery()).contains("ifGenerationMatch=17", "ifGenerationNotMatch=18");
        }
    }

    @Test
    void testConditionFailureKeepsErrorCode() throws Exception {
        startServer();
        try (AsyncOperator async = AsyncOperator.of(
                        ServiceConfig.Http.builder().endpoint(endpoint).build());
                Operator op = async.blocking()) {
            assertReadErrors(
                    op,
                    async,
                    ReadOptions.builder().ifMatch("changed").length(2).build(),
                    OpenDALException.Code.ConditionNotMatch);
        }
        assertThat(methods).containsExactly("GET", "GET", "GET");
    }

    @Test
    void testUnsupportedConditionReachesCore() {
        ReadOptions options = ReadOptions.builder().ifMatch("etag").build();
        try (AsyncOperator async =
                        AsyncOperator.of(ServiceConfig.Memory.builder().build());
                Operator op = async.blocking()) {
            op.write("file", CONTENT);
            assertReadErrors(op, async, options, OpenDALException.Code.Unsupported);
        }
    }

    @Test
    void testDefaultsAndRangeBoundaries() throws Exception {
        ReadOptions defaults = ReadOptions.builder().build();
        assertThat(defaults.offset).isZero();
        assertThat(defaults.length).isEqualTo(-1);
        assertThat(defaults.concurrent).isEqualTo(1);
        assertThat(defaults.chunk).isEqualTo(-1);
        assertThat(defaults.contentLengthHint).isEqualTo(-1);
        try (AsyncOperator async =
                        AsyncOperator.of(ServiceConfig.Memory.builder().build());
                Operator op = async.blocking()) {
            op.write("file", CONTENT);
            op.write("empty", new byte[0]);
            assertReads(op, async, "file", defaults, "0123456789");
            assertReads(op, async, "file", ReadOptions.builder().offset(8).build(), "89");
            assertReads(op, async, "file", ReadOptions.builder().length(0).build(), "");
            assertReads(
                    op,
                    async,
                    "empty",
                    ReadOptions.builder().contentLengthHint(0).chunk(2).build(),
                    "");
        }
    }

    static Stream<Arguments> invalidOptions() {
        return Stream.of(
                Arguments.of(ReadOptions.builder().concurrent(0).build(), OpenDALException.Code.ConfigInvalid),
                Arguments.of(ReadOptions.builder().concurrent(-1).build(), OpenDALException.Code.ConfigInvalid),
                Arguments.of(ReadOptions.builder().chunk(0).build(), OpenDALException.Code.ConfigInvalid),
                Arguments.of(ReadOptions.builder().chunk(-2).build(), OpenDALException.Code.ConfigInvalid),
                Arguments.of(ReadOptions.builder().contentLengthHint(-2).build(), OpenDALException.Code.ConfigInvalid),
                Arguments.of(ReadOptions.builder().offset(-1).build(), OpenDALException.Code.RangeNotSatisfied),
                Arguments.of(ReadOptions.builder().length(-2).build(), OpenDALException.Code.RangeNotSatisfied),
                Arguments.of(
                        ReadOptions.builder().ifModifiedSince(Instant.MIN).build(), OpenDALException.Code.Unexpected),
                Arguments.of(
                        ReadOptions.builder().ifUnmodifiedSince(Instant.MIN).build(),
                        OpenDALException.Code.Unexpected));
    }

    @ParameterizedTest
    @MethodSource("invalidOptions")
    void testInvalidOptions(ReadOptions options, OpenDALException.Code code) {
        try (AsyncOperator async =
                        AsyncOperator.of(ServiceConfig.Memory.builder().build());
                Operator op = async.blocking()) {
            op.write("file", CONTENT);
            assertReadErrors(op, async, options, code);
        }
    }

    @Test
    void testInvalidAsyncOptionsDoNotRegisterFutures() throws Exception {
        // Invalid inputs throw before Java can take a registered future, so inspect pending IDs.
        Class<?> registryType = Class.forName(AsyncOperator.class.getName() + "$AsyncRegistry");
        Field registryField = registryType.getDeclaredField("registry");
        registryField.setAccessible(true);
        Map<?, ?> registry = (Map<?, ?>) registryField.get(registryType.getEnumConstants()[0]);
        Set<?> pendingBefore = new HashSet<>(registry.keySet());
        try (AsyncOperator async =
                AsyncOperator.of(ServiceConfig.Memory.builder().build())) {
            ReadOptions invalid = ReadOptions.builder().chunk(0).build();
            assertThatThrownBy(() -> async.read("file", invalid))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.ConfigInvalid));
        }
        assertThat(pendingBefore.containsAll(registry.keySet()))
                .as("invalid options must not leave registered futures")
                .isTrue();
    }

    private static void assertReads(Operator op, AsyncOperator async, String path, ReadOptions options, String expected)
            throws Exception {
        byte[] bytes = expected.getBytes(StandardCharsets.UTF_8);
        assertThat(op.read(path, options)).isEqualTo(bytes);
        assertThat(async.read(path, options).join()).isEqualTo(bytes);
        try (OperatorInputStream in = op.createInputStream(path, options)) {
            assertThat(IOUtils.toByteArray(in)).isEqualTo(bytes);
        }
    }

    private static void assertReadErrors(
            Operator op, AsyncOperator async, ReadOptions options, OpenDALException.Code code) {
        assertThatThrownBy(() -> op.read("file", options)).is(OpenDALExceptionCondition.ofSync(code));
        assertThatThrownBy(() -> async.read("file", options).join()).is(OpenDALExceptionCondition.ofAsync(code));
        assertThatThrownBy(() -> {
                    try (OperatorInputStream in = op.createInputStream("file", options)) {
                        IOUtils.toByteArray(in);
                    }
                })
                .is(OpenDALExceptionCondition.ofSync(code));
    }
}
