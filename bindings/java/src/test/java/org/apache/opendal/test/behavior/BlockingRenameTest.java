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
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.UUID;
import org.apache.opendal.Capability;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class BlockingRenameTest extends BehaviorTestBase {

    @BeforeAll
    public void precondition() {
        final Capability capability = op().info.capability;
        assumeTrue(capability.read && capability.write && capability.rename && capability.createDir);
    }

    /**
     * Rename a file and test with stat.
     */
    @Test
    public void testBlockingRenameFile() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] sourceContent = generateBytes();

        op().write(sourcePath, sourceContent);

        final String targetPath = UUID.randomUUID().toString();

        op().rename(sourcePath, targetPath);

        assertThatThrownBy(() -> op().stat(sourcePath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));

        assertThat(op().stat(targetPath).getContentLength()).isEqualTo(sourceContent.length);

        op().delete(sourcePath);
        op().delete(targetPath);
    }

    /**
     * Rename a nonexistent source should return an error.
     */
    @Test
    public void testBlockingRenameNonExistingSource() {
        final String sourcePath = UUID.randomUUID().toString();
        final String targetPath = UUID.randomUUID().toString();

        assertThatThrownBy(() -> op().rename(sourcePath, targetPath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));
    }

    /**
     * Rename a dir as source should return an error.
     */
    @Test
    public void testBlockingRenameSourceDir() {
        final String sourcePath = UUID.randomUUID() + "/";
        final String targetPath = UUID.randomUUID().toString();

        op().createDir(sourcePath);

        assertThatThrownBy(() -> op().rename(sourcePath, targetPath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.IsADirectory));
    }

    /**
     * Rename to a dir should return an error.
     */
    @Test
    public void testBlockingRenameTargetDir() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] sourceContent = generateBytes();

        op().write(sourcePath, sourceContent);

        final String targetPath = UUID.randomUUID() + "/";

        op().createDir(targetPath);

        assertThatThrownBy(() -> op().rename(sourcePath, targetPath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.IsADirectory));

        op().delete(sourcePath);
        op().delete(targetPath);
    }

    /**
     * Rename a file to self should return an error.
     */
    @Test
    public void testBlockingRenameSelf() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] sourceContent = generateBytes();

        op().write(sourcePath, sourceContent);

        assertThatThrownBy(() -> op().rename(sourcePath, sourcePath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.IsSameFile));

        op().delete(sourcePath);
    }

    /**
     * Rename to a nested path, parent path should be created successfully.
     */
    @Test
    public void testBlockingRenameNested() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] sourceContent = generateBytes();

        op().write(sourcePath, sourceContent);

        final String targetPath = String.format("%s/%s/%s", UUID.randomUUID(), UUID.randomUUID(), UUID.randomUUID());

        op().rename(sourcePath, targetPath);

        assertThatThrownBy(() -> op().stat(sourcePath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));

        assertThat(op().read(targetPath)).isEqualTo(sourceContent);

        op().delete(sourcePath);
        op().delete(targetPath);
    }

    /**
     * Rename to an existing path should overwrite successfully.
     */
    @Test
    public void testBlockingRenameOverwrite() {
        final String sourcePath = UUID.randomUUID().toString();
        final byte[] sourceContent = generateBytes();

        op().write(sourcePath, sourceContent);

        final String targetPath = UUID.randomUUID().toString();
        final byte[] targetContent = generateBytes();

        assertNotEquals(sourceContent, targetContent);

        op().write(targetPath, targetContent);

        op().rename(sourcePath, targetPath);

        assertThatThrownBy(() -> op().stat(sourcePath))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));

        assertThat(op().read(targetPath)).isEqualTo(sourceContent);

        op().delete(sourcePath);
        op().delete(targetPath);
    }
}
