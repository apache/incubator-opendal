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
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.security.NoSuchAlgorithmException;
import java.util.UUID;
import org.apache.opendal.Capability;
import org.apache.opendal.Metadata;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.ReadOptions;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class BlockingReadOnlyTest extends BehaviorTestBase {

    private static final String NORMAL_FILE_NAME = "normal_file.txt";
    private static final String NORMAL_DIR_NAME = "normal_dir/";
    private static final String SPECIAL_FILE_NAME = "special_file  !@#$%^&()_+-=;',.txt";
    private static final String SPECIAL_DIR_NAME = "special_dir  !@#$%^&()_+-=;',/";
    private static final String FILE_SHA256_DIGEST = "943048ba817cdcd786db07d1f42d5500da7d10541c2f9353352cd2d3f66617e5";
    private static final long FILE_LENGTH = 30482L;

    @BeforeAll
    public void precondition() {
        final Capability capability = op().info.capability;
        assumeTrue(capability.read && !capability.write);
    }

    /**
     * Stat normal file and dir should return metadata
     */
    @Test
    public void testBlockingReadOnlyStatFileAndDir() {
        final Metadata fileMeta = op().stat(NORMAL_FILE_NAME);
        assertTrue(fileMeta.isFile());
        assertEquals(FILE_LENGTH, fileMeta.getContentLength());

        final Metadata dirMeta = op().stat(NORMAL_DIR_NAME);
        assertTrue(dirMeta.isDir());
    }

    /**
     * Stat special file and dir should return metadata
     */
    @Test
    public void testBlockingReadOnlyStatSpecialChars() {
        final Metadata fileMeta = op().stat(SPECIAL_FILE_NAME);
        assertTrue(fileMeta.isFile());
        assertEquals(FILE_LENGTH, fileMeta.getContentLength());

        final Metadata dirMeta = op().stat(SPECIAL_DIR_NAME);
        assertTrue(dirMeta.isDir());
    }

    /**
     * Stat not exist file should return NotFound
     */
    @Test
    public void testBlockingReadOnlyStatNotExist() {
        final String path = UUID.randomUUID().toString();
        assertThatThrownBy(() -> op().stat(path)).is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));
    }

    /**
     * Read full content should match.
     */
    @Test
    public void testBlockingReadonlyReadFull() throws NoSuchAlgorithmException {
        final byte[] content = op().read(NORMAL_FILE_NAME);
        assertEquals(FILE_LENGTH, content.length);
        assertEquals(FILE_SHA256_DIGEST, sha256Digest(content));
    }

    /**
     * Read with offset and length should match.
     */
    @Test
    public void testBlockingReadonlyReadWithOffsetAndLength() throws NoSuchAlgorithmException {
        final byte[] content = op().read(NORMAL_FILE_NAME, 0, FILE_LENGTH);
        assertEquals(FILE_LENGTH, content.length);
        assertEquals(FILE_SHA256_DIGEST, sha256Digest(content));
    }

    /**
     * Read with positive offset and length should match.
     */
    @Test
    public void testBlockingReadonlyReadWithPositiveOffsetAndLength() {
        final byte[] content = op().read(NORMAL_FILE_NAME, 100, 200);
        assertEquals(200, content.length);
    }

    /**
     * Read with offset and -1 length should match.
     */
    @Test
    public void testBlockingReadonlyReadWithOffsetAndUnlimitedLength() {
        final byte[] content = op().read(NORMAL_FILE_NAME, 100, -1);
        assertEquals(FILE_LENGTH - 100, content.length);
    }

    /**
     * Read with length larger than file length should throw error.
     */
    @Test
    public void testBlockingReadonlyReadWithLengthLargerThanFileLength() {
        assertThatThrownBy(() -> op().read(NORMAL_FILE_NAME, 100, FILE_LENGTH))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.Unexpected))
                .hasMessageContaining("reader got too little data");
    }

    /**
     * Read with invalid offset or length should throw error.
     */
    @Test
    public void testBlockingReadonlyReadWithNegativeOffset() {
        assertThatThrownBy(() -> op().read(NORMAL_FILE_NAME, -1, FILE_LENGTH))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("offset must be non-negative");
        assertThatThrownBy(() -> op().read(NORMAL_FILE_NAME, 0, -2))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("length must be non-negative");
    }

    /**
     * Read with options should match.
     */
    @Test
    public void testBlockingReadonlyReadWithOptions() {
        final byte[] content = op().read(NORMAL_FILE_NAME, ReadOptions.builder().build());
        assertEquals(FILE_LENGTH, content.length);
    }

    /**
     * Read with options of offset and length should match.
     */
    @Test
    public void testBlockingReadonlyReadWithOptionsWithOffsetAndLength() {
        final byte[] content = op().read(
                        NORMAL_FILE_NAME,
                        ReadOptions.builder().offset(0).length(FILE_LENGTH).build());
        assertEquals(FILE_LENGTH, content.length);
    }

    /**
     * Read with options of positive offset and length should match.
     */
    @Test
    public void testBlockingReadonlyReadWithOptionsWithPositiveOffsetAndHalfLength() {
        final byte[] content = op().read(
                        NORMAL_FILE_NAME,
                        ReadOptions.builder().offset(100).length(200).build());
        assertEquals(200, content.length);
    }

    /**
     * Read with options of offset and default length should match.
     */
    @Test
    public void testBlockingReadonlyReadWithOptionsWithNegativeLength() {
        final byte[] content =
                op().read(NORMAL_FILE_NAME, ReadOptions.builder().offset(100).build());
        assertEquals(FILE_LENGTH - 100, content.length);
    }

    /**
     * Read with options of length larger than file length should throw error.
     */
    @Test
    public void testBlockingReadonlyReadWithOptionsWithLengthLargerThanFileLength() {
        assertThatThrownBy(() -> op().read(
                                NORMAL_FILE_NAME,
                                ReadOptions.builder().length(FILE_LENGTH + 1).build()))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.Unexpected))
                .hasMessageContaining("reader got too little data");
    }

    /**
     * Read with options of invalid offset or length should throw error.
     */
    @Test
    public void testBlockingReadonlyReadWithOptionsWithInvalidOffsetOrLength() {
        assertThatThrownBy(() -> op().read(
                                NORMAL_FILE_NAME,
                                ReadOptions.builder().offset(-1).build()))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("offset must be non-negative");
        assertThatThrownBy(() -> op().read(
                                NORMAL_FILE_NAME,
                                ReadOptions.builder().length(-2).build()))
                .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("length must be non-negative");
    }

    /**
     * Read not exist file should return NotFound
     */
    @Test
    public void testBlockingReadOnlyReadNotExist() {
        final String path = UUID.randomUUID().toString();
        assertThatThrownBy(() -> op().read(path)).is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));
    }
}
