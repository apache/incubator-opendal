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
public class AsyncReadOnlyTest extends BehaviorTestBase {

    private static final String NORMAL_FILE_NAME = "normal_file.txt";
    private static final String NORMAL_DIR_NAME = "normal_dir/";
    private static final String SPECIAL_FILE_NAME = "special_file  !@#$%^&()_+-=;',.txt";
    private static final String SPECIAL_DIR_NAME = "special_dir  !@#$%^&()_+-=;',/";
    private static final String FILE_SHA256_DIGEST = "943048ba817cdcd786db07d1f42d5500da7d10541c2f9353352cd2d3f66617e5";
    private static final long FILE_LENGTH = 30482L;

    @BeforeAll
    public void precondition() {
        final Capability capability = asyncOp().info.capability;
        assumeTrue(capability.read && !capability.write);
    }

    /**
     * Stat normal file and dir should return metadata
     */
    @Test
    public void testReadOnlyStatFileAndDir() {
        final Metadata fileMeta = asyncOp().stat(NORMAL_FILE_NAME).join();
        assertTrue(fileMeta.isFile());
        assertEquals(FILE_LENGTH, fileMeta.getContentLength());

        final Metadata dirMeta = asyncOp().stat(NORMAL_DIR_NAME).join();
        assertTrue(dirMeta.isDir());
    }

    /**
     * Stat special file and dir should return metadata
     */
    @Test
    public void testReadOnlyStatSpecialChars() {
        final Metadata fileMeta = asyncOp().stat(SPECIAL_FILE_NAME).join();
        assertTrue(fileMeta.isFile());
        assertEquals(FILE_LENGTH, fileMeta.getContentLength());

        final Metadata dirMeta = asyncOp().stat(SPECIAL_DIR_NAME).join();
        assertTrue(dirMeta.isDir());
    }

    /**
     * Stat not cleaned path should also succeed.
     */
    @Test
    public void testReadOnlyStatNotCleanedPath() {
        final Metadata fileMeta = asyncOp().stat("//" + NORMAL_FILE_NAME).join();
        assertTrue(fileMeta.isFile());
        assertEquals(FILE_LENGTH, fileMeta.getContentLength());
    }

    /**
     * Stat not exist file should return NotFound
     */
    @Test
    public void testReadOnlyStatNotExist() {
        final String path = UUID.randomUUID().toString();

        assertThatThrownBy(() -> asyncOp().stat(path).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));
    }

    /**
     * Root should be able to stat and returns DIR.
     */
    @Test
    public void testReadOnlyStatRoot() {
        final Metadata meta1 = asyncOp().stat("").join();
        assertTrue(meta1.isDir());

        final Metadata meta2 = asyncOp().stat("/").join();
        assertTrue(meta2.isDir());
    }

    /**
     * Read full content should match.
     */
    @Test
    public void testReadOnlyReadFull() throws NoSuchAlgorithmException {
        final byte[] content = asyncOp().read(NORMAL_FILE_NAME).join();
        assertEquals(FILE_LENGTH, content.length);

        assertEquals(FILE_SHA256_DIGEST, sha256Digest(content));
    }

    /**
     * Read full content should match.
     */
    @Test
    public void testReadOnlyReadFullWithSpecialChars() throws NoSuchAlgorithmException {
        final byte[] content = asyncOp().read(SPECIAL_FILE_NAME).join();
        assertEquals(FILE_LENGTH, content.length);

        assertEquals(FILE_SHA256_DIGEST, sha256Digest(content));
    }

    /**
     * Read with offset and length should match.
     */
    @Test
    public void testReadonlyReadWithOffsetAndLength() throws NoSuchAlgorithmException {
        final byte[] content = asyncOp().read(NORMAL_FILE_NAME, 0, FILE_LENGTH).join();
        assertEquals(FILE_LENGTH, content.length);
        assertEquals(FILE_SHA256_DIGEST, sha256Digest(content));
    }

    /**
     * Read with positive offset and length should match.
     */
    @Test
    public void testReadonlyReadWithPositiveOffsetAndLength() {
        final byte[] content = asyncOp().read(NORMAL_FILE_NAME, 100, 200).join();
        assertEquals(200, content.length);
    }

    /**
     * Read with offset and -1 length should match.
     */
    @Test
    public void testReadonlyReadWithOffsetAndUnlimitedLength() {
        final byte[] content = asyncOp().read(NORMAL_FILE_NAME, 100, -1).join();
        assertEquals(FILE_LENGTH - 100, content.length);
    }

    /**
     * Read with length larger than file length should throw error.
     */
    @Test
    public void testReadonlyReadWithLengthLargerThanFileLength() {
        assertThatThrownBy(
                        () -> asyncOp().read(NORMAL_FILE_NAME, 100, FILE_LENGTH).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.Unexpected))
                .hasMessageContaining("reader got too little data");
    }

    /**
     * Read with invalid offset or length should throw error.
     */
    @Test
    public void testReadonlyReadWithNegativeOffset() {
        assertThatThrownBy(
                        () -> asyncOp().read(NORMAL_FILE_NAME, -1, FILE_LENGTH).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("offset must be non-negative");
        assertThatThrownBy(() -> asyncOp().read(NORMAL_FILE_NAME, 0, -2).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("length must be non-negative");
    }

    /**
     * Read with options should match.
     */
    @Test
    public void testReadonlyReadWithOptions() {
        final byte[] content =
                asyncOp().read(NORMAL_FILE_NAME, ReadOptions.builder().build()).join();
        assertEquals(FILE_LENGTH, content.length);
    }

    /**
     * Read with options of offset and length should match.
     */
    @Test
    public void testReadonlyReadWithOptionsWithOffsetAndLength() {
        final byte[] content = asyncOp()
                .read(
                        NORMAL_FILE_NAME,
                        ReadOptions.builder().offset(0).length(FILE_LENGTH).build())
                .join();
        assertEquals(FILE_LENGTH, content.length);
    }

    /**
     * Read with options of positive offset and length should match.
     */
    @Test
    public void testReadonlyReadWithOptionsWithPositiveOffsetAndHalfLength() {
        final byte[] content = asyncOp()
                .read(
                        NORMAL_FILE_NAME,
                        ReadOptions.builder().offset(100).length(200).build())
                .join();
        assertEquals(200, content.length);
    }

    /**
     * Read with options of offset and default length should match.
     */
    @Test
    public void testReadonlyReadWithOptionsWithNegativeLength() {
        final byte[] content = asyncOp()
                .read(NORMAL_FILE_NAME, ReadOptions.builder().offset(100).build())
                .join();
        assertEquals(FILE_LENGTH - 100, content.length);
    }

    /**
     * Read with options of length larger than file length should throw error.
     */
    @Test
    public void testReadonlyReadWithOptionsWithLengthLargerThanFileLength() {
        assertThatThrownBy(() -> asyncOp()
                        .read(
                                NORMAL_FILE_NAME,
                                ReadOptions.builder().length(FILE_LENGTH + 1).build())
                        .join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.Unexpected))
                .hasMessageContaining("reader got too little data");
    }

    /**
     * Read with options of invalid offset or length should throw error.
     */
    @Test
    public void testReadonlyReadWithOptionsWithInvalidOffsetOrLength() {
        assertThatThrownBy(() -> asyncOp()
                        .read(NORMAL_FILE_NAME, ReadOptions.builder().offset(-1).build())
                        .join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("offset must be non-negative");
        assertThatThrownBy(() -> asyncOp()
                        .read(NORMAL_FILE_NAME, ReadOptions.builder().length(-2).build())
                        .join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.RangeNotSatisfied))
                .hasMessageContaining("length must be non-negative");
    }

    /**
     * Read not exist file should return NotFound
     */
    @Test
    public void testReadOnlyReadNotExist() {
        final String path = UUID.randomUUID().toString();

        assertThatThrownBy(() -> asyncOp().read(path).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));
    }

    /**
     * Read with dir path should return an error.
     */
    @Test
    public void testReadOnlyReadWithDirPath() {
        final String path = UUID.randomUUID().toString() + "/";

        assertThatThrownBy(() -> asyncOp().read(path).join())
                .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.IsADirectory));
    }
}
