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

import static org.apache.opendal.test.behavior.BehaviorTestBase.generateBytes;
import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertTrue;
import java.nio.file.Path;
import java.util.UUID;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.Metadata;
import org.apache.opendal.Operator;
import org.apache.opendal.ServiceConfig;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

public class MetadataTest {
    @TempDir
    private static Path tempDir;

    @Test
    public void testAsyncMetadata() {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();

        try (final AsyncOperator op = AsyncOperator.of(fs)) {
            final String dir = UUID.randomUUID() + "/";
            op.createDir(dir).join();
            final Metadata dirMetadata = op.stat(dir).join();
            assertTrue(dirMetadata.isDir());

            final String path = UUID.randomUUID().toString();
            final byte[] content = generateBytes();
            op.write(path, content).join();

            final Metadata metadata = op.stat(path).join();
            assertTrue(metadata.isFile());
            assertThat(metadata.contentLength).isEqualTo(content.length);
            assertThat(metadata.lastModified).isNotNull();
            assertThat(metadata.cacheControl).isNull();
            assertThat(metadata.contentDisposition).isNull();
            assertThat(metadata.contentMd5).isNull();
            assertThat(metadata.contentType).isNull();
            assertThat(metadata.etag).isNull();
            assertThat(metadata.version).isNull();
            assertThat(metadata.userMetadata).isEmpty();

            op.delete(dir).join();
            op.delete(path).join();
        }
    }

    @Test
    public void testBlockingMetadata() {
        final ServiceConfig.Fs fs =
                ServiceConfig.Fs.builder().root(tempDir.toString()).build();

        try (final Operator op = Operator.of(fs)) {
            final String dir = UUID.randomUUID() + "/";
            op.createDir(dir);
            final Metadata dirMetadata = op.stat(dir);
            assertTrue(dirMetadata.isDir());

            final String path = UUID.randomUUID().toString();
            final byte[] content = generateBytes();
            op.write(path, content);

            final Metadata metadata = op.stat(path);
            assertTrue(metadata.isFile());
            assertThat(metadata.contentLength).isEqualTo(content.length);
            assertThat(metadata.lastModified).isNotNull();
            assertThat(metadata.cacheControl).isNull();
            assertThat(metadata.contentDisposition).isNull();
            assertThat(metadata.contentMd5).isNull();
            assertThat(metadata.contentType).isNull();
            assertThat(metadata.etag).isNull();
            assertThat(metadata.version).isNull();
            assertThat(metadata.userMetadata).isEmpty();

            op.delete(dir);
            op.delete(path);
        }
    }
}
