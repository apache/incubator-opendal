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
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Instant;
import org.apache.commons.io.IOUtils;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.Operator;
import org.apache.opendal.OperatorInputStream;
import org.apache.opendal.OperatorReader;
import org.apache.opendal.ReaderOptions;
import org.apache.opendal.ServiceConfig;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

public class OperatorReaderTest {
    @TempDir
    private Path tempDir;

    @Test
    void testReusableReaderOutlivesOperator() {
        final OperatorReader reader;
        try (Operator op =
                Operator.of(ServiceConfig.Fs.builder().root(tempDir.toString()).build())) {
            op.write("file", "0123456789");
            reader = op.createReader(
                    "file",
                    ReaderOptions.builder()
                            .concurrent(2)
                            .chunk(2)
                            .prefetch(1)
                            .contentLengthHint(10)
                            .build());
        }
        try (OperatorReader r = reader) {
            assertThat(r.read(4, 5)).isEqualTo("45678".getBytes(StandardCharsets.UTF_8));
            assertThat(r.read(1, 2)).isEqualTo("12".getBytes(StandardCharsets.UTF_8));
            assertThat(r.read(8, -1)).isEqualTo("89".getBytes(StandardCharsets.UTF_8));
            assertThat(r.read(0, 0)).isEmpty();
            assertThat(r.read()).isEqualTo("0123456789".getBytes(StandardCharsets.UTF_8));
        }
        reader.close();
        assertThatThrownBy(reader::read).isInstanceOf(IllegalStateException.class);
    }

    @ParameterizedTest
    @CsvSource({"-1, 1", "0, -2"})
    void testInvalidRange(long offset, long length) {
        try (Operator op = Operator.of(
                        ServiceConfig.Fs.builder().root(tempDir.toString()).build());
                OperatorReader reader = op.createReader("missing")) {
            assertThatThrownBy(() -> reader.read(offset, length))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.RangeNotSatisfied));
            assertThatThrownBy(() -> reader.createInputStream(offset, length))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.RangeNotSatisfied));
        }
    }

    @Test
    void testMissingFileFailsOnRead() {
        try (Operator op = Operator.of(
                        ServiceConfig.Fs.builder().root(tempDir.toString()).build());
                OperatorReader reader = op.createReader("missing")) {
            assertThatThrownBy(reader::read).is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));
        }
    }

    @Test
    void testIndependentStreams() throws Exception {
        try (Operator op =
                Operator.of(ServiceConfig.Fs.builder().root(tempDir.toString()).build())) {
            op.write("file", "0123456789");
            try (OperatorReader reader =
                    op.createReader("file", ReaderOptions.builder().chunk(2).build())) {
                try (OperatorInputStream in = reader.createInputStream(4, 3)) {
                    assertThat(IOUtils.toByteArray(in)).isEqualTo("456".getBytes(StandardCharsets.UTF_8));
                }
                assertThat(reader.read(0, 2)).isEqualTo("01".getBytes(StandardCharsets.UTF_8));
                try (OperatorInputStream first = reader.createInputStream();
                        OperatorInputStream second = reader.createInputStream(4, 5)) {
                    reader.close();
                    op.close();
                    assertThatThrownBy(reader::createInputStream).isInstanceOf(IllegalStateException.class);
                    assertThat(first.read()).isEqualTo('0');
                    assertThat(second.read()).isEqualTo('4');
                    first.close();
                    first.close();
                    assertThatThrownBy(first::read).isInstanceOf(IllegalStateException.class);
                    assertThatThrownBy(() -> first.read(new byte[2], 0, 2)).isInstanceOf(IllegalStateException.class);
                    assertThat(IOUtils.toByteArray(second)).isEqualTo("5678".getBytes(StandardCharsets.UTF_8));
                    assertThat(second.read()).isEqualTo(-1);
                }
            }
        }
    }

    @Test
    void testUnsupportedReaderCondition() {
        try (Operator op =
                Operator.of(ServiceConfig.Fs.builder().root(tempDir.toString()).build())) {
            assertThatThrownBy(() -> op.createReader(
                            "missing", ReaderOptions.builder().ifMatch("etag").build()))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.Unsupported));
        }
    }

    @Test
    void testInvalidTimestamp() {
        try (Operator op =
                Operator.of(ServiceConfig.Fs.builder().root(tempDir.toString()).build())) {
            assertThatThrownBy(() -> op.createReader(
                            "missing",
                            ReaderOptions.builder().ifModifiedSince(Instant.MIN).build()))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.Unexpected));
        }
    }
}
