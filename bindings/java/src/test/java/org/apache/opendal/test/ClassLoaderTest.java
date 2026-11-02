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
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import org.apache.opendal.AsyncExecutor;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.Entry;
import org.apache.opendal.OpenDALException;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

public class ClassLoaderTest {
    @TempDir
    Path tempDir;

    @ParameterizedTest
    @ValueSource(strings = {"default", "explicit"})
    void testAsyncOperationsWithIsolatedClassLoader(String mode) throws Exception {
        final String executable = System.getProperty("os.name").startsWith("Windows") ? "java.exe" : "java";
        final Path java = Paths.get(System.getProperty("java.home"), "bin", executable);
        final String tests = Paths.get(ClassLoaderTest.class
                        .getProtectionDomain()
                        .getCodeSource()
                        .getLocation()
                        .toURI())
                .toString();
        final String classes = AsyncOperator.class
                .getProtectionDomain()
                .getCodeSource()
                .getLocation()
                .toExternalForm();
        final Path log = tempDir.resolve(mode + ".log");
        // The system loader sees only test classes; OpenDAL is exclusively in the child loader.
        final Process process = new ProcessBuilder(
                        java.toString(),
                        "-ea",
                        "-Xcheck:jni",
                        "-Djava.library.path=" + System.getProperty("java.library.path"),
                        "-cp",
                        tests,
                        Application.class.getName(),
                        classes,
                        mode)
                .redirectErrorStream(true)
                .redirectOutput(log.toFile())
                .start();
        final boolean exited;
        try {
            exited = process.waitFor(20, TimeUnit.SECONDS);
        } finally {
            if (process.isAlive()) {
                process.destroyForcibly();
                process.waitFor(5, TimeUnit.SECONDS);
            }
        }
        final String output = new String(Files.readAllBytes(log), StandardCharsets.UTF_8);
        assertThat(output).contains("ASYNC_OPERATIONS_COMPLETED");
        assertThat(exited)
                .as("JVM must exit after releasing the executor: %s", output)
                .isTrue();
        assertThat(process.exitValue()).as(output).isZero();
    }

    public static class Application {
        public static void main(String[] args) throws Exception {
            final URL tests =
                    Application.class.getProtectionDomain().getCodeSource().getLocation();
            try (final URLClassLoader loader = new URLClassLoader(new URL[] {tests, new URL(args[0])}, null)) {
                // Keep the caller's context loader unable to see OpenDAL. Native initialization
                // must use the defining loader rather than inheriting the caller's context loader.
                Class.forName("org.apache.opendal.test.ClassLoaderTest$Operations", true, loader)
                        .getMethod("run", String.class)
                        .invoke(null, args[1]);
            }
            System.out.println("ASYNC_OPERATIONS_COMPLETED");
        }
    }

    public static class Operations {
        public static void run(String mode) throws Exception {
            for (int round = 0; round < 3; round++) {
                try (final AsyncExecutor executor =
                                mode.equals("explicit") ? AsyncExecutor.createTokioExecutor(1) : null;
                        final AsyncOperator op = AsyncOperator.of("memory", Collections.emptyMap(), executor)) {
                    final byte[] content = "hello".getBytes(StandardCharsets.UTF_8);
                    op.write("example.txt", content).get(5, TimeUnit.SECONDS);
                    assert java.util.Arrays.equals(op.read("example.txt").get(5, TimeUnit.SECONDS), content);
                    assert op.stat("example.txt").get(5, TimeUnit.SECONDS).getContentLength() == content.length;
                    final List<Entry> entries = op.list("/").get(5, TimeUnit.SECONDS);
                    assert entries.size() == 1;
                    assert entries.get(0).getPath().equals("example.txt");
                    try {
                        op.read("missing.txt").get(5, TimeUnit.SECONDS);
                        throw new AssertionError("Reading a missing file must fail");
                    } catch (ExecutionException e) {
                        assert e.getCause() instanceof OpenDALException : e;
                        assert ((OpenDALException) e.getCause()).getCode() == OpenDALException.Code.NotFound;
                    }
                    // A failed operation must not poison later callbacks on the executor.
                    op.delete("example.txt").get(5, TimeUnit.SECONDS);
                }
            }
        }
    }
}
