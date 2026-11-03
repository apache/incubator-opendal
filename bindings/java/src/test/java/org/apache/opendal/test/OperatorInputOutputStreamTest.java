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
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.InputStreamReader;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Random;
import java.util.concurrent.atomic.AtomicLong;
import java.util.stream.Stream;
import org.apache.commons.io.IOUtils;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.Operator;
import org.apache.opendal.OperatorInputStream;
import org.apache.opendal.OperatorOutputStream;
import org.apache.opendal.OperatorReader;
import org.apache.opendal.ReadOptions;
import org.apache.opendal.ReaderOptions;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.WriteOptions;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.Arguments;
import org.junit.jupiter.params.provider.MethodSource;
import org.junit.jupiter.params.provider.ValueSource;

public class OperatorInputOutputStreamTest {
    @TempDir
    private static Path tempDir;

    @Test
    void testReadWriteWithStream() throws Exception {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();

        try (final Operator op = Operator.of(fs)) {
            final String path = "OperatorInputOutputStreamTest.txt";
            final long multi = 1024 * 1024;

            try (final OperatorOutputStream os = op.createOutputStream(path)) {
                for (long i = 0; i < multi; i++) {
                    os.write("[content] OperatorInputStreamTest\n".getBytes());
                }
            }

            try (final OperatorInputStream is = op.createInputStream(path)) {
                final Stream<String> lines = new BufferedReader(new InputStreamReader(is)).lines();
                final AtomicLong count = new AtomicLong();
                lines.forEach((line) -> {
                    assertThat(line).isEqualTo("[content] OperatorInputStreamTest");
                    count.incrementAndGet();
                });
                assertThat(count.get()).isEqualTo(multi);
            }
        }
    }

    @Test
    void testCreateInputStreamWithOptions() {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            final String path = "testCreateInputStreamWithOptions.txt";
            final String content = "0123456789";
            op.write(path, content);

            try (final OperatorInputStream is = op.createInputStream(
                    path, ReadOptions.builder().offset(4L).length(5L).build())) {
                final byte[] buffer = new byte[5];
                final int read = is.read(buffer, 0, 5);
                assertThat(read).isEqualTo(5);
                assertThat(new String(buffer)).isEqualTo("45678");
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }
    }

    @Test
    void testChunkedInputStream() throws Exception {
        final byte[] content = new byte[4 * 1024 + 13];
        new Random(8252).nextBytes(content);
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            final String path = "chunked.bin";
            op.write(path, content);
            final ReaderOptions options = ReaderOptions.builder()
                    .concurrent(4)
                    .chunk(1024)
                    .prefetch(2)
                    .build();
            try (final OperatorReader reader = op.createReader(path, options);
                    final OperatorInputStream in = reader.createInputStream()) {
                assertThat(IOUtils.toByteArray(in)).isEqualTo(content);
                assertThat(in.read()).isEqualTo(-1);
            }
        }
    }

    @Test
    void testEmptyInputStreamWithReaderOptions() throws Exception {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            final String path = "empty.bin";
            op.write(path, new byte[0]);
            final ReaderOptions options =
                    ReaderOptions.builder().chunk(2).contentLengthHint(0).build();
            try (final OperatorReader reader = op.createReader(path, options);
                    final OperatorInputStream in = reader.createInputStream()) {
                assertThat(in.read()).isEqualTo(-1);
            }
        }
    }

    static Stream<Arguments> invalidReaderOptions() {
        return Stream.of(
                Arguments.of(ReaderOptions.builder().concurrent(0).build(), "concurrent"),
                Arguments.of(ReaderOptions.builder().concurrent(-1).build(), "concurrent"),
                Arguments.of(ReaderOptions.builder().chunk(0).build(), "chunk"),
                Arguments.of(ReaderOptions.builder().chunk(-2).build(), "chunk"),
                Arguments.of(ReaderOptions.builder().prefetch(-1).build(), "prefetch"),
                Arguments.of(ReaderOptions.builder().contentLengthHint(-2).build(), "contentLengthHint"));
    }

    @ParameterizedTest
    @MethodSource("invalidReaderOptions")
    void testInvalidReaderOptions(ReaderOptions options, String field) {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            assertThatThrownBy(() -> op.createReader("invalid-options", options))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.ConfigInvalid))
                    .hasMessageContaining(field);
        }
    }

    @Test
    void testBulkWriteAvoidsScalarDispatch() throws Exception {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            final int[] scalarCalls = {0};
            final byte[] content = new byte[3 * 16384 + 17];
            new Random(8244).nextBytes(content);
            try (final OperatorOutputStream out =
                    new OperatorOutputStream(
                            op, "bulk-dispatch", WriteOptions.builder().build()) {
                        @Override
                        public void write(int b) throws java.io.IOException {
                            scalarCalls[0]++;
                            super.write(b);
                        }
                    }) {
                out.write(content);
            }
            assertThat(op.read("bulk-dispatch")).isEqualTo(content);
            assertThat(scalarCalls[0]).isZero();
        }
    }

    @ParameterizedTest
    @ValueSource(ints = {1, 7, 16384})
    void testMixedBulkWrites(int bufferSize) throws Exception {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        final byte[] content = new byte[3 * bufferSize + 13];
        new Random(8244).nextBytes(content);
        final ByteArrayOutputStream expected = new ByteArrayOutputStream();
        try (final Operator op = Operator.of(fs)) {
            try (final OperatorOutputStream out = new OperatorOutputStream(
                    op, "mixed", bufferSize, WriteOptions.builder().build())) {
                out.write(255);
                expected.write(255);
                out.write(content, 2, bufferSize - 1);
                expected.write(content, 2, bufferSize - 1);
                out.write(content, 1, content.length - 2);
                expected.write(content, 1, content.length - 2);
                out.flush();
                out.write(content, 0, 3);
                expected.write(content, 0, 3);
                Arrays.fill(content, (byte) 0);
                out.write(127);
                expected.write(127);
            }
            assertThat(op.read("mixed")).isEqualTo(expected.toByteArray());
        }
    }

    @ParameterizedTest
    @ValueSource(ints = {0, -1})
    void testInvalidOutputBufferSize(int bufferSize) {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            assertThatThrownBy(() -> op.createOutputStream("invalid-buffer", bufferSize))
                    .isInstanceOf(IllegalArgumentException.class)
                    .hasMessageContaining("maxBytes");
        }
    }

    @Test
    void testBulkWriteArgumentChecks() throws Exception {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            try (final OperatorOutputStream out = op.createOutputStream("bounds")) {
                final byte[] content = {1, 2, 3};
                assertThatThrownBy(() -> out.write(null, 0, 0)).isInstanceOf(NullPointerException.class);
                assertThatThrownBy(() -> out.write(content, -1, 1)).isInstanceOf(IndexOutOfBoundsException.class);
                assertThatThrownBy(() -> out.write(content, 0, -1)).isInstanceOf(IndexOutOfBoundsException.class);
                assertThatThrownBy(() -> out.write(content, 2, 2)).isInstanceOf(IndexOutOfBoundsException.class);
                assertThatThrownBy(() -> out.write(content, Integer.MAX_VALUE, 1))
                        .isInstanceOf(IndexOutOfBoundsException.class);
                out.write(content, content.length, 0);
                out.write(content);
            }
            assertThat(op.read("bounds")).isEqualTo(new byte[] {1, 2, 3});
        }
    }

    @Test
    void testCreateOutputStreamWithOptions() {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();
        try (final Operator op = Operator.of(fs)) {
            final String path = "testCreateOutputStreamWithOptions.txt";
            final String content = "0123456789";

            try (final OperatorOutputStream os = op.createOutputStream(
                    path, WriteOptions.builder().contentType("text/plain").build())) {
                os.write(content.getBytes());
            } catch (Exception e) {
                throw new RuntimeException(e);
            }

            assertThat(op.read(path)).isEqualTo(content.getBytes());
        }
    }
}
