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
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.UUID;
import org.apache.opendal.Capability;
import org.apache.opendal.Metadata;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class BlockingCreateDirTest extends BehaviorTestBase {
    @BeforeAll
    public void precondition() {
        final Capability capability = op().info.capability;
        assumeTrue(capability.createDir);
    }

    /**
     * Create dir with dir path should succeed.
     */
    @Test
    public void testBlockingCreateDir() {
        final String path = UUID.randomUUID() + "/";
        op().createDir(path);

        final Metadata meta = op().stat(path);
        assertThat(meta.isFile()).isFalse();

        op().delete(path);
    }

    /**
     * Create dir on existing dir should succeed.
     */
    @Test
    public void testBlockingDirExisting() {
        final String path = UUID.randomUUID() + "/";
        op().createDir(path);
        op().createDir(path);

        final Metadata meta = op().stat(path);
        assertThat(meta.isFile()).isFalse();

        op().delete(path);
    }
}
