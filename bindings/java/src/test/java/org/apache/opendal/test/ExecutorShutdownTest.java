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
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.Operator;
import org.apache.opendal.layer.RetryLayer;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

public class ExecutorShutdownTest {
    @TempDir
    Path tempDir;

    @ParameterizedTest
    @ValueSource(strings = {"blocking", "async", "derived", "streams", "callback", "recreate"})
    void testJvmExitsAfterClosingOperator(String mode) throws Exception {
        final String executable = System.getProperty("os.name").startsWith("Windows") ? "java.exe" : "java";
        final Path java = Paths.get(System.getProperty("java.home"), "bin", executable);
        final Path log = tempDir.resolve(mode + ".log");
        final Process process = new ProcessBuilder(
                        java.toString(),
                        "-Xcheck:jni",
                        "-Djava.library.path=" + System.getProperty("java.library.path"),
                        "-cp",
                        System.getProperty("surefire.test.class.path", System.getProperty("java.class.path")),
                        Application.class.getName(),
                        mode)
                .redirectErrorStream(true)
                .redirectOutput(log.toFile())
                .start();
        final boolean exited;
        try {
            exited = process.waitFor(15, TimeUnit.SECONDS);
        } finally {
            if (process.isAlive()) {
                process.destroyForcibly();
                process.waitFor(5, TimeUnit.SECONDS);
            }
        }
        final String output = new String(Files.readAllBytes(log), StandardCharsets.UTF_8);
        assertThat(output).contains("OPERATORS_CLOSED");
        assertThat(exited).as("JVM must exit after main returns: %s", output).isTrue();
        assertThat(process.exitValue()).as(output).isZero();
    }

    public static class Application {
        public static void main(String[] args) throws Exception {
            final byte[] content = "Hello, OpenDAL!".getBytes(StandardCharsets.UTF_8);
            if (args[0].equals("blocking")) {
                try (final Operator op = Operator.of("memory", Collections.emptyMap())) {
                    op.write("example.txt", content);
                    assertThat(op.read("example.txt")).isEqualTo(content);
                }
            } else if (args[0].equals("async")) {
                try (final AsyncOperator op = AsyncOperator.of("memory", Collections.emptyMap())) {
                    op.write("example.txt", content).join();
                    assertThat(op.read("example.txt").join()).isEqualTo(content);
                    assertThatThrownBy(() -> op.read("missing.txt").join()).hasCauseInstanceOf(OpenDALException.class);
                    assertThat(op.read("example.txt").join()).isEqualTo(content);
                }
            } else if (args[0].equals("derived")) {
                try (final AsyncOperator original = AsyncOperator.of("memory", Collections.emptyMap());
                        final AsyncOperator independent = AsyncOperator.of("memory", Collections.emptyMap());
                        final AsyncOperator duplicate = original.duplicate();
                        final AsyncOperator layered =
                                duplicate.layer(RetryLayer.builder().build());
                        final Operator blocking = layered.blocking();
                        final Operator blockingDuplicate = blocking.duplicate()) {
                    original.write("example.txt", content).join();
                    original.close();
                    independent.write("other.txt", content).join();
                    assertThat(independent.read("other.txt").join()).isEqualTo(content);
                    independent.close();
                    assertThat(duplicate.read("example.txt").join()).isEqualTo(content);
                    duplicate.close();
                    assertThat(layered.read("example.txt").join()).isEqualTo(content);
                    layered.close();
                    assertThat(blocking.read("example.txt")).isEqualTo(content);
                    blocking.close();
                    assertThat(blockingDuplicate.read("example.txt")).isEqualTo(content);
                }
            } else if (args[0].equals("streams")) {
                try (final Operator op = Operator.of("memory", Collections.emptyMap())) {
                    op.write("example.txt", content);
                    try (final InputStream reader = op.createInputStream("example.txt")) {
                        op.close();
                        final byte[] actual = new byte[content.length];
                        assertThat(reader.read(actual)).isEqualTo(content.length);
                        assertThat(actual).isEqualTo(content);
                    }
                }
                try (final Operator op = Operator.of("memory", Collections.emptyMap());
                        final OutputStream writer = op.createOutputStream("example.txt")) {
                    op.close();
                    writer.write(content);
                }
            } else if (args[0].equals("callback")) {
                try (final AsyncOperator op = AsyncOperator.of("memory", Collections.emptyMap())) {
                    op.write("example.txt", content).join();
                    op.read("example.txt")
                            .thenAccept(actual -> {
                                assertThat(actual).isEqualTo(content);
                                op.close();
                            })
                            .join();
                }
            } else if (args[0].equals("recreate")) {
                for (int round = 0; round < 3; round++) {
                    final CompletableFuture<?>[] operations = new CompletableFuture<?>[4];
                    for (int i = 0; i < operations.length; i++) {
                        operations[i] = CompletableFuture.runAsync(() -> {
                            try (final AsyncOperator op = AsyncOperator.of("memory", Collections.emptyMap())) {
                                final CompletableFuture<Void> write = op.write("example.txt", content);
                                op.close();
                                write.join();
                            }
                        });
                    }
                    CompletableFuture.allOf(operations).join();
                }
            } else {
                throw new IllegalArgumentException(args[0]);
            }
            System.out.println("OPERATORS_CLOSED");
        }
    }
}
