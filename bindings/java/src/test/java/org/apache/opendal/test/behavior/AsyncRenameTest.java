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
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.UUID;
import org.apache.opendal.Capability;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.assertj.core.api.Assertions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class AsyncRenameTest extends BehaviorTestBase {
    @BeforeAll
    public void precondition() {
        final Capability capability = asyncOp().info.capability;
        assumeTrue(capability.read && capability.write && capability.rename && capability.createDir);
    }

    /**
     * Rename a file and test with stat.
     */
    @Test
    public void testRenameFile() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] content = generateBytes();

        asyncOp().write(sourcePath, content).join();

        final String targetPath = UUID.randomUUID().toString();

        asyncOp().rename(sourcePath, targetPath).join();

        assertThatThrownBy(() -> asyncOp().stat(sourcePath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));

        Assertions.assertThat(asyncOp().read(targetPath).join()).isEqualTo(content);

        asyncOp().delete(sourcePath).join();
        asyncOp().delete(targetPath).join();
    }

    /**
     * Rename a nonexistent source should return an error.
     */
    @Test
    public void testRenameNonExistingSource() {
        final String sourcePath = UUID.randomUUID().toString();
        final String targetPath = UUID.randomUUID().toString();

        assertThatThrownBy(() -> asyncOp().rename(sourcePath, targetPath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));
    }

    /**
     * Rename a dir as source should return an error.
     */
    @Test
    public void testRenameSourceDir() {
        final String sourcePath = UUID.randomUUID() + "/";
        final String targetPath = UUID.randomUUID().toString();

        asyncOp().createDir(sourcePath).join();

        assertThatThrownBy(() -> asyncOp().rename(sourcePath, targetPath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.IsADirectory));

        asyncOp().delete(sourcePath).join();
    }

    /**
     * Rename to a dir should return an error.
     */
    @Test
    public void testRenameTargetDir() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] content = generateBytes();

        asyncOp().write(sourcePath, content).join();

        final String targetPath = UUID.randomUUID() + "/";

        asyncOp().createDir(targetPath).join();

        assertThatThrownBy(() -> asyncOp().rename(sourcePath, targetPath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.IsADirectory));

        asyncOp().delete(sourcePath).join();
        asyncOp().delete(targetPath).join();
    }

    /**
     * Rename a file to self should return an error.
     */
    @Test
    public void testRenameSelf() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] content = generateBytes();

        asyncOp().write(sourcePath, content).join();

        assertThatThrownBy(() -> asyncOp().rename(sourcePath, sourcePath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.IsSameFile));

        asyncOp().delete(sourcePath).join();
    }

    /**
     * Rename to a nested path, parent path should be created successfully.
     */
    @Test
    public void testRenameNested() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] content = generateBytes();

        asyncOp().write(sourcePath, content).join();

        final String targetPath = String.format("%s/%s/%s", UUID.randomUUID(), UUID.randomUUID(), UUID.randomUUID());

        asyncOp().rename(sourcePath, targetPath).join();

        assertThatThrownBy(() -> asyncOp().stat(sourcePath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));

        Assertions.assertThat(asyncOp().read(targetPath).join()).isEqualTo(content);

        asyncOp().delete(sourcePath).join();
        asyncOp().delete(targetPath).join();
    }

    /**
     * Rename to an existing path should overwrite successfully.
     */
    @Test
    public void testRenameOverwrite() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] sourceContent = generateBytes();

        asyncOp().write(sourcePath, sourceContent).join();

        final String targetPath = UUID.randomUUID().toString();
        final byte[] targetContent = generateBytes();
        assertNotEquals(sourceContent, targetContent);

        asyncOp().write(targetPath, targetContent).join();

        asyncOp().rename(sourcePath, targetPath).join();

        assertThatThrownBy(() -> asyncOp().stat(sourcePath).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));

        Assertions.assertThat(asyncOp().read(targetPath).join()).isEqualTo(sourceContent);

        asyncOp().delete(sourcePath).join();
        asyncOp().delete(targetPath).join();
    }
}
