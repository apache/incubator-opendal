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

import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.assertj.core.api.AssertionsForClassTypes.assertThat;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.UUID;
import org.apache.opendal.Capability;
import org.apache.opendal.Metadata;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.StatOptions;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class BlockingStatOptionsTest extends BehaviorTestBase {

    @BeforeAll
    public void precondition() {
        Capability capability = op().info.capability;
        assumeTrue(capability.read && capability.write && capability.stat);
    }

    @Test
    void testStatWithIfMatch() {
        assumeTrue(op().info.capability.statWithIfMatch);

        String path = UUID.randomUUID().toString();
        byte[] content = generateBytes();
        op().write(path, content);
        Metadata meta = op().stat(path);
        StatOptions invalidOptions =
                StatOptions.builder().ifMatch("\"invalid\"").build();

        assertThatThrownBy(() -> op().stat(path, invalidOptions))
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.ConditionNotMatch));

        StatOptions validOptions = StatOptions.builder().ifMatch(meta.getEtag()).build();
        Metadata result = op().stat(path, validOptions);
        assertThat(result).isNotNull();

        op().delete(path);
    }

    @Test
    void testStatWithIfNoneMatch() {
        assumeTrue(op().info.capability.statWithIfNoneMatch);

        String path = UUID.randomUUID().toString();
        byte[] content = generateBytes();
        op().write(path, content);
        Metadata meta = op().stat(path);
        StatOptions matchingOptions =
                StatOptions.builder().ifNoneMatch(meta.getEtag()).build();

        assertThatThrownBy(() -> op().stat(path, matchingOptions))
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.ConditionNotMatch));

        StatOptions nonMatchingOptions =
                StatOptions.builder().ifNoneMatch("\"invalid\"").build();
        Metadata result = op().stat(path, nonMatchingOptions);
        assertThat(result.getContentLength()).isEqualTo(meta.getContentLength());

        op().delete(path);
    }

    @Test
    void testStatWithIfVersionMatch() {
        assumeTrue(op().info.capability.statWithIfVersionMatch);

        String path = UUID.randomUUID().toString();
        op().write(path, generateBytes());
        String staleVersion = op().stat(path).getVersion();
        op().write(path, generateBytes());
        String currentVersion = op().stat(path).getVersion();

        assertThat(staleVersion).isNotNull();
        assertThat(currentVersion).isNotNull().isNotEqualTo(staleVersion);

        StatOptions currentOptions =
                StatOptions.builder().ifVersionMatch(currentVersion).build();
        Metadata result = op().stat(path, currentOptions);
        assertThat(result.getVersion()).isEqualTo(currentVersion);

        StatOptions staleOptions =
                StatOptions.builder().ifVersionMatch(staleVersion).build();
        assertThatThrownBy(() -> op().stat(path, staleOptions))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.ConditionNotMatch));

        String missingPath = UUID.randomUUID().toString();
        assertThatThrownBy(() -> op().stat(missingPath, currentOptions))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));

        op().delete(path);
    }

    @Test
    void testStatWithIfVersionNotMatch() {
        assumeTrue(op().info.capability.statWithIfVersionNotMatch);

        String path = UUID.randomUUID().toString();
        op().write(path, generateBytes());
        String staleVersion = op().stat(path).getVersion();
        op().write(path, generateBytes());
        String currentVersion = op().stat(path).getVersion();

        assertThat(staleVersion).isNotNull();
        assertThat(currentVersion).isNotNull().isNotEqualTo(staleVersion);

        StatOptions staleOptions =
                StatOptions.builder().ifVersionNotMatch(staleVersion).build();
        Metadata result = op().stat(path, staleOptions);
        assertThat(result.getVersion()).isEqualTo(currentVersion);

        StatOptions currentOptions =
                StatOptions.builder().ifVersionNotMatch(currentVersion).build();
        assertThatThrownBy(() -> op().stat(path, currentOptions))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.ConditionNotMatch));

        String missingPath = UUID.randomUUID().toString();
        assertThatThrownBy(() -> op().stat(missingPath, currentOptions))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));

        op().delete(path);
    }

    @Test
    void testStatWithIfModifiedSince() {
        assumeTrue(op().info.capability.statWithIfModifiedSince);

        String path = UUID.randomUUID().toString();
        byte[] content = generateBytes();
        op().write(path, content);
        Metadata meta = op().stat(path);
        Instant since = meta.getLastModified().minus(1, ChronoUnit.SECONDS);
        StatOptions options1 = StatOptions.builder().ifModifiedSince(since).build();
        Metadata result1 = op().stat(path, options1);

        assertThat(result1.getLastModified()).isEqualTo(meta.getLastModified());

        try {
            Thread.sleep(1000);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }

        Instant futureTime = meta.getLastModified().plus(1, ChronoUnit.SECONDS);
        StatOptions options2 = StatOptions.builder().ifModifiedSince(futureTime).build();

        assertThatThrownBy(() -> op().stat(path, options2))
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.ConditionNotMatch));

        op().delete(path);
    }

    @Test
    void testStatWithIfUnmodifiedSince() {
        assumeTrue(op().info.capability.statWithIfUnmodifiedSince);

        String path = UUID.randomUUID().toString();
        byte[] content = generateBytes();
        op().write(path, content);
        Metadata meta = op().stat(path);
        Instant beforeTime = meta.getLastModified().minus(1, ChronoUnit.SECONDS);
        StatOptions options1 =
                StatOptions.builder().ifUnmodifiedSince(beforeTime).build();

        assertThatThrownBy(() -> op().stat(path, options1))
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.ConditionNotMatch));

        try {
            Thread.sleep(1000);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }

        Instant afterTime = meta.getLastModified().plus(1, ChronoUnit.SECONDS);
        StatOptions options2 =
                StatOptions.builder().ifUnmodifiedSince(afterTime).build();
        Metadata result2 = op().stat(path, options2);

        assertThat(result2.getLastModified()).isEqualTo(meta.getLastModified());

        op().delete(path);
    }

    @Test
    void testStatWithVersion() {
        assumeTrue(op().info.capability.statWithVersion);

        String path = UUID.randomUUID().toString();
        byte[] content = generateBytes();
        op().write(path, content);
        Metadata metadata = op().stat(path);
        String version = metadata.getVersion();

        assertThat(version).isNotNull();

        StatOptions versionOptions = StatOptions.builder().version(version).build();
        Metadata versionedMeta = op().stat(path, versionOptions);

        assertThat(versionedMeta.getVersion()).isEqualTo(version);
        op().write(path, content);
        Metadata metadata2 = op().stat(path);

        assertThat(metadata2.getVersion()).isNotEqualTo(version);
        Metadata oldVersionMetadata = op().stat(path, versionOptions);
        assertThat(oldVersionMetadata.getVersion()).isEqualTo(version);

        op().delete(path);
    }

    @Test
    void testStatWithNotExistingVersion() {
        assumeTrue(op().info.capability.statWithVersion);

        String path = UUID.randomUUID().toString();
        byte[] content = generateBytes();
        op().write(path, content);
        String version = op().stat(path).getVersion();

        String path2 = UUID.randomUUID().toString();
        op().write(path2, content);
        StatOptions options = StatOptions.builder().version(version).build();

        assertThatThrownBy(() -> op().stat(path2, options))
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));

        op().delete(path);
        op().delete(path2);
    }
}
