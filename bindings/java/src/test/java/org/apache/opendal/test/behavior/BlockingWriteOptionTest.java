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

package org.apache.opendal.test.behavior;

import static org.assertj.core.api.AssertionsForClassTypes.assertThat;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.Collections;
import java.util.Map;
import java.util.UUID;
import org.apache.opendal.WriteOptions;
import org.junit.jupiter.api.Test;

public class BlockingWriteOptionTest extends BehaviorTestBase {

    @Test
    void testWriteWithCacheControl() {
        assumeTrue(op().info.capability.writeWithCacheControl);

        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        final String cacheControl = "max-age=3600";

        WriteOptions options = WriteOptions.builder().cacheControl(cacheControl).build();
        op().write(path, content, options);

        String actualCacheControl = op().stat(path).getCacheControl();
        assertThat(actualCacheControl).isEqualTo(cacheControl);
    }

    @Test
    void testWriteWithContentType() {
        assumeTrue(op().info.capability.writeWithContentType);

        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        final String contentType = "application/json";

        WriteOptions options = WriteOptions.builder().contentType(contentType).build();
        op().write(path, content, options);

        String actualContentType = op().stat(path).getContentType();
        assertThat(actualContentType).isEqualTo(contentType);
    }

    @Test
    void testWriteWithContentDisposition() {
        assumeTrue(op().info.capability.writeWithContentDisposition);

        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        final String disposition = "attachment; filename=\"test.txt\"";

        WriteOptions options =
                WriteOptions.builder().contentDisposition(disposition).build();
        op().write(path, content, options);

        String actualDisposition = op().stat(path).getContentDisposition();
        assertThat(actualDisposition).isEqualTo(disposition);
    }

    @Test
    void testWriteWithUserMetadata() {
        assumeTrue(op().info.capability.writeWithUserMetadata);

        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        final Map<String, String> userMetadata = Collections.singletonMap("location", "everywhere");

        WriteOptions options = WriteOptions.builder().userMetadata(userMetadata).build();
        op().write(path, content, options);

        Map<String, String> actualUserMetadata = op().stat(path).getUserMetadata();
        assertThat(actualUserMetadata).isEqualTo(userMetadata);
    }

    @Test
    void testWriteWithAppend() {
        assumeTrue(op().info.capability.writeCanAppend);

        final String path = UUID.randomUUID().toString();
        final byte[] contentOne = "Test".getBytes();
        final byte[] contentTwo = " Data".getBytes();
        WriteOptions appendOptions = WriteOptions.builder().append(true).build();

        op().write(path, contentOne, appendOptions);
        op().write(path, contentTwo, appendOptions);

        byte[] result = op().read(path);
        assertThat(result.length).isEqualTo(contentOne.length + contentTwo.length);
        assertThat(result).isEqualTo("Test Data".getBytes());
    }

    @Test
    void testWriteWithChunk() {
        assumeTrue(op().info.capability.writeCanMulti);

        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes(5 * 1024 * 1024);

        WriteOptions options = WriteOptions.builder().chunk(1024 * 1024L).build();

        op().write(path, content, options);

        byte[] result = op().read(path);
        assertThat(result).isEqualTo(content);
    }

    @Test
    void testWriteWithConcurrentChunk() {
        assumeTrue(op().info.capability.writeCanMulti);

        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes(5 * 1024 * 1024);

        WriteOptions options =
                WriteOptions.builder().chunk(1024 * 1024L).concurrent(4).build();

        op().write(path, content, options);

        byte[] result = op().read(path);
        assertThat(result).isEqualTo(content);
    }
}
