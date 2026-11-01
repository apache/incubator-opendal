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
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.concurrent.TimeUnit;
import lombok.Cleanup;
import org.apache.opendal.AsyncExecutor;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.ServiceConfig;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

public class AsyncExecutorTest {
    @TempDir
    Path tempDir;

    @Test
    void testInvalidCoresDoNotTerminateJvm() throws Exception {
        final String executable = System.getProperty("os.name").startsWith("Windows") ? "java.exe" : "java";
        final Path java = Paths.get(System.getProperty("java.home"), "bin", executable);
        final Path log = tempDir.resolve("invalid-cores.log");
        final Process process = new ProcessBuilder(
                        java.toString(),
                        "-Xcheck:jni",
                        "-Djava.library.path=" + System.getProperty("java.library.path"),
                        "-cp",
                        System.getProperty("surefire.test.class.path", System.getProperty("java.class.path")),
                        InvalidCoresApplication.class.getName())
                .directory(tempDir.toFile())
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
        assertThat(exited).as("JVM must exit after main returns: %s", output).isTrue();
        assertThat(process.exitValue()).as(output).isZero();
        assertThat(output).contains("INVALID_CORES_REJECTED");
    }

    public static class InvalidCoresApplication {
        public static void main(String[] args) {
            for (int cores : new int[] {0, -1, Integer.MIN_VALUE}) {
                assertThatThrownBy(() -> AsyncExecutor.createTokioExecutor(cores))
                        .isInstanceOf(IllegalArgumentException.class);
            }
            try (final AsyncExecutor executor = AsyncExecutor.createTokioExecutor(1);
                    final AsyncOperator op =
                            AsyncOperator.of(ServiceConfig.Memory.builder().build(), executor)) {
                final byte[] content = "Hello, OpenDAL!".getBytes(StandardCharsets.UTF_8);
                op.write("example.txt", content).join();
                assertThat(op.read("example.txt").join()).isEqualTo(content);
            }
            System.out.println("INVALID_CORES_REJECTED");
        }
    }

    @Test
    void testDedicatedTokioExecutor() {
        final ServiceConfig.Memory memory =
                ServiceConfig.Memory.builder().root("/opendal/").build();
        final int cores = Runtime.getRuntime().availableProcessors();
        @Cleanup final AsyncExecutor executor = AsyncExecutor.createTokioExecutor(cores);
        @Cleanup final AsyncOperator op = AsyncOperator.of(memory, executor);
        assertThat(op.info).isNotNull();

        final String key = "key";
        final byte[] v0 = "v0".getBytes(StandardCharsets.UTF_8);
        final byte[] v1 = "v1".getBytes(StandardCharsets.UTF_8);
        op.write(key, v0).join();
        assertThat(op.read(key).join()).isEqualTo(v0);
        op.write(key, v1).join();
        assertThat(op.read(key).join()).isEqualTo(v1);

        assertThat(executor.isDisposed()).isFalse();
        executor.close();
        assertThat(executor.isDisposed()).isTrue();

        // @Cleanup will close executor once more, but we don't crash with the guard.
    }
}
