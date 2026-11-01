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

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.UUID;
import org.apache.opendal.Capability;
import org.apache.opendal.Metadata;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class AsyncWriteTest extends BehaviorTestBase {
    @BeforeAll
    public void precondition() {
        final Capability capability = asyncOp().info.capability;
        assumeTrue(capability.read && capability.write);
    }

    /**
     * Read not exist file should return NotFound.
     */
    @Test
    public void testReadNotExist() {
        final String path = UUID.randomUUID().toString();
        assertThatThrownBy(() -> asyncOp().read(path).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));
    }

    /**
     * Read full content should match.
     */
    @Test
    public void testReadFull() {
        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        asyncOp().write(path, content).join();
        final byte[] actualContent = asyncOp().read(path).join();
        assertThat(actualContent).isEqualTo(content);
        asyncOp().delete(path).join();
    }

    /**
     * Stat not exist file should return NotFound.
     */
    @Test
    public void testStatNotExist() {
        final String path = UUID.randomUUID().toString();
        assertThatThrownBy(() -> asyncOp().stat(path).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));
    }

    /**
     * Stat existing file should return metadata.
     */
    @Test
    public void testStatFile() {
        final String path = UUID.randomUUID().toString();
        final byte[] content = generateBytes();
        asyncOp().write(path, content).join();
        final Metadata meta = asyncOp().stat(path).join();
        assertThat(meta.isFile()).isTrue();
        assertThat(meta.getContentLength()).isEqualTo(content.length);

        asyncOp().delete(path).join();
    }
}
