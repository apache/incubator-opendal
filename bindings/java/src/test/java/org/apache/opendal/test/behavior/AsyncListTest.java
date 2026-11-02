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
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assumptions.assumeTrue;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import java.util.stream.Collectors;
import org.apache.opendal.Capability;
import org.apache.opendal.Entry;
import org.apache.opendal.ListOptions;
import org.apache.opendal.Metadata;
import org.apache.opendal.OpenDALException;
import org.apache.opendal.test.condition.OpenDALExceptionCondition;
import org.assertj.core.util.Lists;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class AsyncListTest extends BehaviorTestBase {

    @BeforeAll
    public void precondition() {
        final Capability capability = asyncOp().info.capability;
        assumeTrue(capability.read && capability.write && capability.list && capability.createDir);
    }

    /**
     * List dir should return newly created file.
     */
    @Test
    public void testListDir() {
        final String parent = UUID.randomUUID().toString();
        final String path = String.format("%s/%s", parent, UUID.randomUUID());
        final byte[] content = generateBytes();

        asyncOp().write(path, content).join();

        final List<Entry> entries = asyncOp().list(parent + "/").join();
        boolean found = false;
        for (Entry entry : entries) {
            if (entry.getPath().equals(path)) {
                Metadata meta = asyncOp().stat(path).join();
                assertTrue(meta.isFile());
                assertThat(meta.getContentLength()).isEqualTo(content.length);

                found = true;
            }
        }
        assertTrue(found);
        asyncOp().delete(path).join();
    }

    /**
     * listing a directory, which contains more objects than a single page can take.
     */
    @Test
    public void testListRichDir() {
        final String parentPath = "test_list_rich_dir/";
        asyncOp().createDir(parentPath).join();
        final List<String> expected = new ArrayList<>();
        for (int i = 0; i < 10; i++) {
            expected.add(String.format("%sfile-%d", parentPath, i));
        }

        for (String path : expected) {
            asyncOp().write(path, parentPath).join();
        }

        final List<Entry> entries = asyncOp().list(parentPath).join();
        final List<String> actual =
                entries.stream().map(Entry::getPath).sorted().collect(Collectors.toList());

        expected.add(parentPath);
        Collections.sort(expected);
        assertThat(actual).isEqualTo(expected);
        asyncOp().removeAll(parentPath).join();
    }

    /**
     * List empty dir should return nothing.
     */
    @Test
    public void testListEmptyDir() {
        final String dir = String.format("%s/", UUID.randomUUID());
        asyncOp().createDir(dir).join();

        final List<Entry> entries = asyncOp().list(dir).join();
        final List<String> actual = entries.stream().map(Entry::getPath).collect(Collectors.toList());
        assertThat(actual).hasSize(1);
        assertThat(actual.get(0)).isEqualTo(dir);

        asyncOp().delete(dir).join();
    }

    /**
     * List non exist dir should return nothing.
     */
    @Test
    public void testListNotExistDir() {
        final String dir = String.format("%s/", UUID.randomUUID());

        final List<Entry> entries = asyncOp().list(dir).join();
        assertThat(entries).isEmpty();
    }

    /**
     * List dir should return correct sub dir.
     */
    @Test
    public void testListSubDir() {
        final String path = String.format("%s/", UUID.randomUUID());
        asyncOp().createDir(path).join();

        final List<Entry> entries = asyncOp().list("/").join();
        boolean found = false;
        for (Entry entry : entries) {
            if (entry.getPath().equals(path)) {
                Metadata metadata = entry.getMetadata();
                assertTrue(metadata.isDir());
                found = true;
            }
        }
        assertTrue(found);

        asyncOp().delete(path).join();
    }

    /**
     * List dir should also to list nested dir.
     */
    @Test
    public void testListNestedDir() {
        final String dir = String.format("%s/%s/", UUID.randomUUID(), UUID.randomUUID());
        final String fileName = UUID.randomUUID().toString();
        final String filePath = String.format("%s%s", dir, fileName);
        final String dirName = String.format("%s/", UUID.randomUUID());
        final String dirPath = String.format("%s%s", dir, dirName);
        final String content = "test_list_nested_dir";

        asyncOp().createDir(dir).join();
        asyncOp().write(filePath, content).join();
        asyncOp().createDir(dirPath).join();

        final List<Entry> entries = asyncOp().list(dir).join();
        assertThat(entries).hasSize(3);

        final List<String> expectedPaths = Lists.newArrayList(dir, dirPath, filePath);
        Collections.sort(expectedPaths);
        final List<String> actualPaths =
                entries.stream().map(Entry::getPath).sorted().collect(Collectors.toList());
        assertThat(actualPaths).isEqualTo(expectedPaths);

        for (Entry entry : entries) {
            // check file
            if (entry.getPath().equals(filePath)) {
                Metadata metadata = asyncOp().stat(filePath).join();
                assertTrue(metadata.isFile());
                assertThat(metadata.getContentLength()).isEqualTo(content.length());
                // check dir
            } else if (entry.getPath().equals(dirPath)) {
                Metadata metadata = entry.getMetadata();
                assertTrue(metadata.isDir());
            } else {
                assertThat(entry.getPath()).isEqualTo(dir);
                assertTrue(entry.metadata.isDir());
            }
        }

        asyncOp().removeAll(dir).join();
    }

    /**
     * Remove all should remove all in this path.
     */
    @Test
    public void testRemoveAll() {
        final String parent = UUID.randomUUID().toString();
        final String[] expected = new String[] {
            "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
        };
        for (String path : expected) {
            if (path.endsWith("/")) {
                asyncOp().createDir(String.format("%s/%s", parent, path)).join();
            } else {
                asyncOp()
                        .write(String.format("%s/%s", parent, path), "test_scan")
                        .join();
            }
        }

        asyncOp().removeAll(parent + "/x/").join();

        for (String path : expected) {
            if (path.endsWith("/")) {
                continue;
            }
            assertThatThrownBy(() ->
                            asyncOp().stat(String.format("%s/%s", parent, path)).join())
                    .is(OpenDALExceptionCondition.ofAsync(OpenDALException.Code.NotFound));
        }

        asyncOp().removeAll(parent + "/").join();
    }

    @Test
    public void testListRecursive() {
        final String dir = String.format("%s/%s/", UUID.randomUUID(), UUID.randomUUID());
        final String fileName = UUID.randomUUID().toString();
        final String filePath = String.format("%s%s", dir, fileName);
        final String dirName = String.format("%s/", UUID.randomUUID());
        final String dirPath = String.format("%s%s", dir, dirName);
        final String content = "test_list_nested_dir";
        final String nestedFile = String.format("%s%s", dirPath, UUID.randomUUID());

        asyncOp().createDir(dir).join();
        asyncOp().write(filePath, content).join();
        asyncOp().createDir(dirPath).join();
        asyncOp().write(nestedFile, content).join();

        final List<Entry> entries = asyncOp()
                .list(dir, ListOptions.builder().recursive(true).build())
                .join();
        assertThat(entries).hasSize(4);

        final List<Entry> noRecursiveEntries = asyncOp()
                .list(dir, ListOptions.builder().recursive(false).build())
                .join();
        assertThat(noRecursiveEntries).hasSize(3);
    }

    @Test
    void testListWithLimitCollectsAllPages() {
        assumeTrue(asyncOp().info.capability.listWithLimit);

        String dir = String.format("%s/", UUID.randomUUID());
        asyncOp().createDir(dir).join();
        for (int i = 0; i < 5; i++) {
            String file = dir + "file-" + i;
            asyncOp().write(file, "data").join();
        }

        ListOptions options = ListOptions.builder().limit(3).build();
        List<Entry> entries = asyncOp().list(dir, options).join();
        assertThat(entries.size()).isEqualTo(6);

        asyncOp().removeAll(dir).join();
    }

    @Test
    void testListWithStartAfter() {
        assumeTrue(asyncOp().info.capability.listWithStartAfter);

        String dir = String.format("%s/", UUID.randomUUID());
        asyncOp().createDir(dir).join();

        List<String> filesToCreate = Lists.newArrayList();
        for (int i = 0; i < 5; i++) {
            String file = dir + "file-" + i;
            asyncOp().write(file, "data").join();
            filesToCreate.add(file);
        }

        ListOptions options =
                ListOptions.builder().startAfter(filesToCreate.get(2)).build();
        List<Entry> entries = asyncOp().list(dir, options).join();
        List<String> actual = entries.stream().map(Entry::getPath).sorted().collect(Collectors.toList());

        assertThat(actual).containsAnyElementsOf(filesToCreate.subList(3, 5));
        asyncOp().removeAll(dir).join();
    }

    @Test
    void testListWithVersions() {
        assumeTrue(asyncOp().info.capability.listWithVersions);

        String dir = String.format("%s/", UUID.randomUUID());
        String path = dir + "versioned-file";

        asyncOp().createDir(dir).join();
        asyncOp().write(path, "data-1").join();
        asyncOp().write(path, "data-2").join();

        ListOptions options = ListOptions.builder().versions(true).build();
        List<Entry> entries = asyncOp().list(dir, options).join();

        assertThat(entries).isNotEmpty();
        asyncOp().removeAll(dir).join();
    }

    @Test
    void testListWithDeleted() {
        assumeTrue(asyncOp().info.capability.listWithDeleted);

        String dir = String.format("%s/", UUID.randomUUID());
        String path = dir + "file";
        asyncOp().createDir(dir).join();

        asyncOp().write(path, "data").join();
        asyncOp().delete(path).join();

        ListOptions options = ListOptions.builder().deleted(true).build();
        List<Entry> entries = asyncOp().list(dir, options).join();

        List<String> actual = entries.stream().map(Entry::getPath).sorted().collect(Collectors.toList());
        assertThat(actual).contains(path);

        asyncOp().removeAll(dir).join();
    }
}
