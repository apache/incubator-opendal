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
public class BlockingListTest extends BehaviorTestBase {

    @BeforeAll
    public void precondition() {
        final Capability capability = op().info.capability;
        assumeTrue(capability.read && capability.write && capability.copy && capability.list && capability.createDir);
    }

    @Test
    public void testBlockingListDir() {
        final String parent = UUID.randomUUID().toString();
        final String path = String.format("%s/%s", parent, UUID.randomUUID());
        final byte[] content = generateBytes();

        op().write(path, content);

        final List<Entry> list = op().list(parent + "/");
        boolean found = false;
        for (Entry entry : list) {
            if (entry.getPath().equals(path)) {
                Metadata meta = op().stat(path);
                assertTrue(meta.isFile());
                assertThat(meta.getContentLength()).isEqualTo(content.length);

                found = true;
            }
        }
        assertTrue(found);

        op().delete(path);
    }

    @Test
    public void testBlockingListNonExistDir() {
        final String dir = String.format("%s/", UUID.randomUUID());

        final List<Entry> list = op().list(dir);
        assertTrue(list.isEmpty());
    }

    /**
     * Remove all should remove all in this path.
     */
    @Test
    public void testBlockingRemoveAll() {
        final String parent = UUID.randomUUID().toString();
        final String[] expected = new String[] {
            "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
        };
        for (String path : expected) {
            if (path.endsWith("/")) {
                op().createDir(String.format("%s/%s", parent, path));
            } else {
                op().write(String.format("%s/%s", parent, path), "test_scan");
            }
        }

        op().removeAll(parent + "/x/");

        for (String path : expected) {
            if (path.endsWith("/")) {
                continue;
            }
            assertThatThrownBy(() -> op().stat(String.format("%s/%s", parent, path)))
                    .is(OpenDALExceptionCondition.ofSync(OpenDALException.Code.NotFound));
        }

        op().removeAll(parent + "/");
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

        op().createDir(dir);
        op().write(filePath, content);
        op().createDir(dirPath);
        op().write(nestedFile, content);

        final List<Entry> entries =
                op().list(dir, ListOptions.builder().recursive(true).build());
        assertThat(entries).hasSize(4);

        final List<Entry> noRecursiveEntries =
                op().list(dir, ListOptions.builder().recursive(false).build());
        assertThat(noRecursiveEntries).hasSize(3);
    }

    @Test
    void testListWithLimitCollectsAllPages() {
        assumeTrue(op().info.capability.listWithLimit);

        String dir = String.format("%s/", UUID.randomUUID());
        op().createDir(dir);
        for (int i = 0; i < 5; i++) {
            String file = dir + "file-" + i;
            op().write(file, "data");
        }

        ListOptions options = ListOptions.builder().limit(3).build();
        List<Entry> entries = op().list(dir, options);
        assertThat(entries.size()).isEqualTo(6);

        op().removeAll(dir);
    }

    @Test
    void testListWithStartAfter() {
        assumeTrue(op().info.capability.listWithStartAfter);

        String dir = String.format("%s/", UUID.randomUUID());
        op().createDir(dir);

        List<String> filesToCreate = Lists.newArrayList();
        for (int i = 0; i < 5; i++) {
            String file = dir + "file-" + i;
            op().write(file, "data");
            filesToCreate.add(file);
        }

        ListOptions options =
                ListOptions.builder().startAfter(filesToCreate.get(2)).build();
        List<Entry> entries = op().list(dir, options);
        List<String> actual = entries.stream().map(Entry::getPath).sorted().collect(Collectors.toList());

        assertThat(actual).containsAnyElementsOf(filesToCreate.subList(3, 5));
        op().removeAll(dir);
    }

    @Test
    void testListWithVersions() {
        assumeTrue(op().info.capability.listWithVersions);

        String dir = String.format("%s/", UUID.randomUUID());
        String path = dir + "versioned-file";

        op().createDir(dir);
        op().write(path, "data-1");
        op().write(path, "data-2");

        ListOptions options = ListOptions.builder().versions(true).build();
        List<Entry> entries = op().list(dir, options);

        assertThat(entries).isNotEmpty();
        op().removeAll(dir);
    }

    @Test
    void testListWithDeleted() {
        assumeTrue(op().info.capability.listWithDeleted);

        String dir = String.format("%s/", UUID.randomUUID());
        String path = dir + "file";

        op().createDir(dir);
        op().write(path, "data");
        op().delete(path);

        ListOptions options = ListOptions.builder().deleted(true).build();
        List<Entry> entries = op().list(dir, options);

        List<String> actual = entries.stream().map(Entry::getPath).sorted().collect(Collectors.toList());
        assertThat(actual).contains(path);

        op().removeAll(dir);
    }
}
