// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

package opendal_test

import (
	"fmt"
	"slices"
	"strings"

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func testsList(cap *opendal.Capability) []behaviorTest {
	if !cap.Read() || !cap.Write() || !cap.List() || !cap.CreateDir() {
		return nil
	}
	tests := []behaviorTest{
		testListCheck,
		testListDir,
		testListRichDir,
		testListEmptyDir,
		testListNonExistDir,
		testListSubDir,
		testListNestedDir,
		testListDirWithFilePath,
		testListEntryMetadata,
		testListWithDefaultOptions,
	}
	if cap.ListWithRecursive() {
		tests = append(tests, testListWithRecursive)
	}
	if cap.ListWithLimit() {
		tests = append(tests, testListWithLimit)
	}
	if cap.ListWithStartAfter() {
		tests = append(tests, testListWithStartAfter)
	}
	if isCapEnabled(cap.ListWithVersions, "list_with_versions") {
		tests = append(tests, testListWithVersions)
	}
	if isCapEnabled(cap.ListWithDeleted, "list_with_deleted") {
		tests = append(tests, testListWithDeleted)
	}
	return tests
}

func testListCheck(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	assert.Nil(op.Check(), "operator check must succeed")
}

func testListDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	path, content, size := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	obs, err := op.List(parent)
	assert.Nil(err)
	defer obs.Close()

	var found bool
	for obs.Next() {
		entry := obs.Entry()

		if entry.Path() != path {
			continue
		}

		meta, err := op.Stat(entry.Path())
		assert.Nil(err)
		assert.True(meta.IsFile())
		assert.Equal(uint64(size), meta.ContentLength())
		found = true
		break
	}
	assert.Nil(obs.Error())
	assert.True(found, "file must be found in list")
}

func testListRichDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	assert.Nil(op.CreateDir(parent))

	var expected []string
	for range 10 {
		path, content, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))
		expected = append(expected, path)
		_, err := op.Write(path, content)
		assert.Nil(err)
	}

	obs, err := op.List(parent)
	assert.Nil(err)
	defer obs.Close()
	var actual []string
	for obs.Next() {
		entry := obs.Entry()
		actual = append(actual, entry.Path())
	}
	assert.Nil(obs.Error())

	expected = append(expected, parent)
	slices.Sort(expected)
	slices.Sort(actual)

	assert.Equal(expected, actual)
}

func testListEmptyDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	dir := fixture.NewDirPath()

	assert.Nil(op.CreateDir(dir), "create must succeed")

	obs, err := op.List(dir)
	assert.Nil(err)
	defer obs.Close()
	var paths []string
	for obs.Next() {
		entry := obs.Entry()
		paths = append(paths, entry.Path())
		assert.Equal(dir, entry.Path())
	}
	assert.Nil(obs.Error())
	assert.Equal(1, len(paths), "dir should only return itself")

	paths = nil
	obs, err = op.List(strings.TrimSuffix(dir, "/"))
	assert.Nil(err)
	defer obs.Close()
	for obs.Next() {
		entry := obs.Entry()
		path := entry.Path()
		paths = append(paths, path)
		meta, err := op.Stat(path)
		assert.Nil(err, "given dir should exist")
		assert.True(meta.IsDir(), "given dir must be dir, but found: %v", path)
	}
	assert.Nil(obs.Error())
	assert.Equal(1, len(paths), "only return the dir itself, but found: %v", paths)
}

func testListNonExistDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	dir := fixture.NewDirPath()

	obs, err := op.List(dir)
	assert.Nil(err)
	defer obs.Close()
	assert.False(obs.Next(), "dir should only return empty")
}

func testListSubDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	path := fixture.NewDirPath()

	assert.Nil(op.CreateDir(path), "create must succeed")

	obs, err := op.List("/")
	assert.Nil(err)
	defer obs.Close()

	var found bool
	for obs.Next() {
		entry := obs.Entry()
		if path != entry.Path() {
			continue
		}
		found = true
		break
	}
	assert.Nil(obs.Error())
	assert.True(found, "dir should be found in list")
}

func testListNestedDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	dir := fixture.PushPath(fmt.Sprintf("%s%s/", parent, uuid.NewString()))

	filePath := fixture.PushPath(fmt.Sprintf("%s%s", dir, uuid.NewString()))
	dirPath := fixture.PushPath(fmt.Sprintf("%s%s/", dir, uuid.NewString()))

	assert.Nil(op.CreateDir(parent), "create must succeed")
	assert.Nil(op.CreateDir(dir), "create must succeed")
	_, err := op.Write(filePath, []byte("test_list_nested_dir"))
	assert.Nil(err, "write must succeed")
	assert.Nil(op.CreateDir(dirPath), "create must succeed")

	obs, err := op.List(parent)
	assert.Nil(err)
	defer obs.Close()
	var paths []string
	var foundParent bool
	var foundDir bool
	for obs.Next() {
		entry := obs.Entry()
		paths = append(paths, entry.Path())
		if entry.Path() == parent {
			foundParent = true
		} else if entry.Path() == dir {
			foundDir = true
		}
	}
	assert.Nil(obs.Error())
	assert.Equal(2, len(paths), "parent should only got 2 entry")
	assert.Equal(foundParent, true, "parent should be found in list")
	assert.Equal(foundDir, true, "dir should be found in list")

	obs, err = op.List(dir)
	assert.Nil(err)
	defer obs.Close()
	paths = nil
	var foundFile bool
	var foundDirPath bool
	foundDir = false
	for obs.Next() {
		entry := obs.Entry()
		paths = append(paths, entry.Path())
		if entry.Path() == filePath {
			foundFile = true
		} else if entry.Path() == dirPath {
			foundDirPath = true
		} else if entry.Path() == dir {
			foundDir = true
		}
	}
	assert.Nil(obs.Error())
	assert.Equal(3, len(paths), "dir should only got 3 entries")

	assert.True(foundDir, "dir should be found in list")

	assert.True(foundFile, "file should be found in list")
	meta, err := op.Stat(filePath)
	assert.Nil(err)
	assert.True(meta.IsFile())
	assert.Equal(uint64(20), meta.ContentLength())

	assert.True(foundDirPath, "dir path should be found in list")
	meta, err = op.Stat(dirPath)
	assert.Nil(err)
	assert.True(meta.IsDir())
}

func testListEntryMetadata(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	assert.Nil(op.CreateDir(parent))

	filePath, content, size := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))
	_, err := op.Write(filePath, content)
	assert.Nil(err)

	obs, err := op.List(parent)
	assert.Nil(err)
	defer obs.Close()

	var sawFile, sawDir bool
	for obs.Next() {
		entry := obs.Entry()
		meta := entry.Metadata()
		assert.NotNil(meta, "list entry metadata must always be populated for path %s", entry.Path())

		switch entry.Path() {
		case filePath:
			sawFile = true
			assert.True(meta.IsFile(), "expected file for %s", entry.Path())
			assert.False(meta.IsDir(), "expected file for %s", entry.Path())
			assert.Equal(uint64(size), meta.ContentLength(),
				"content length from list metadata must match written file size")
		case parent:
			sawDir = true
			assert.True(meta.IsDir(), "expected dir for %s", entry.Path())
			assert.False(meta.IsFile(), "expected dir for %s", entry.Path())
		}
	}
	assert.Nil(obs.Error())
	assert.True(sawFile, "file entry %s must be returned by list", filePath)
	assert.True(sawDir, "parent dir entry %s must be returned by list", parent)
}

func testListDirWithFilePath(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	path, content, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))

	_, err := op.Write(path, content)
	assert.Nil(err)

	obs, err := op.List(strings.TrimSuffix(parent, "/"))
	assert.Nil(err)
	defer obs.Close()

	for obs.Next() {
		entry := obs.Entry()
		assert.Equal(parent, entry.Path())
	}
	assert.Nil(obs.Error())
}

func testListWithDefaultOptions(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	subDir := fixture.PushPath(fmt.Sprintf("%s%s/", parent, uuid.NewString()))
	fileInParent, content, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))
	fileInSub := fixture.PushPath(fmt.Sprintf("%s%s", subDir, uuid.NewString()))

	assert.Nil(op.CreateDir(parent))
	assert.Nil(op.CreateDir(subDir))
	_, err := op.Write(fileInParent, content)
	assert.Nil(err)
	_, err = op.Write(fileInSub, content)
	assert.Nil(err)

	obs, err := op.List(parent)
	assert.Nil(err)
	defer obs.Close()

	var paths []string
	for obs.Next() {
		paths = append(paths, obs.Entry().Path())
	}
	assert.Nil(obs.Error())

	assert.NotContains(paths, fileInSub,
		"List without options must not descend into sub-directories")
	assert.Contains(paths, fileInParent, "direct child file must appear")
	assert.Contains(paths, subDir, "direct child dir must appear")
}

func testListWithRecursive(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	subDir := fixture.PushPath(fmt.Sprintf("%s%s/", parent, uuid.NewString()))
	deepDir := fixture.PushPath(fmt.Sprintf("%s%s/", subDir, uuid.NewString()))

	fileTop := fixture.PushPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))
	fileMid := fixture.PushPath(fmt.Sprintf("%s%s", subDir, uuid.NewString()))
	fileDeep := fixture.PushPath(fmt.Sprintf("%s%s", deepDir, uuid.NewString()))

	content := []byte("recursive test content")

	assert.Nil(op.CreateDir(parent))
	assert.Nil(op.CreateDir(subDir))
	assert.Nil(op.CreateDir(deepDir))
	_, err := op.Write(fileTop, content)
	assert.Nil(err)
	_, err = op.Write(fileMid, content)
	assert.Nil(err)
	_, err = op.Write(fileDeep, content)
	assert.Nil(err)

	obs, err := op.List(parent, opendal.ListWithRecursive(true))
	assert.Nil(err)
	defer obs.Close()

	var paths []string
	for obs.Next() {
		paths = append(paths, obs.Entry().Path())
	}
	assert.Nil(obs.Error())

	assert.Contains(paths, fileTop, "recursive list must include top-level file")
	assert.Contains(paths, fileMid, "recursive list must include file in sub-directory")
	assert.Contains(paths, fileDeep, "recursive list must include file in deep directory")
}

func testListWithLimit(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	assert.Nil(op.CreateDir(parent))

	// Write 5 files.
	for range 5 {
		path, content, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))
		_, err := op.Write(path, content)
		assert.Nil(err)
	}

	// List with limit=2; the operation must succeed without error.
	obs, err := op.List(parent, opendal.ListWithLimit(2))
	assert.Nil(err)
	defer obs.Close()

	var paths []string
	for obs.Next() {
		paths = append(paths, obs.Entry().Path())
	}
	assert.Nil(obs.Error())
	// At least one entry must be returned (limit is a hint, not a hard cap for all backends).
	assert.NotEmpty(paths, "list with limit must return at least one entry")
}

func testListWithStartAfter(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	assert.Nil(op.CreateDir(parent))

	// Write files with predictable sorted names.
	var filePaths []string
	for i := range 5 {
		name := fmt.Sprintf("%sfile-%02d", parent, i)
		filePaths = append(filePaths, name)
		_, err := op.Write(name, []byte("content"))
		assert.Nil(err)
	}
	slices.Sort(filePaths)

	// Start listing from the second file (index 1).
	pivotName := strings.TrimPrefix(filePaths[1], "/")
	obs, err := op.List(parent, opendal.ListWithStartAfter(pivotName))
	assert.Nil(err)
	defer obs.Close()

	var paths []string
	for obs.Next() {
		paths = append(paths, obs.Entry().Path())
	}
	assert.Nil(obs.Error())

	// Files after the pivot must appear.
	for _, p := range filePaths[2:] {
		assert.Contains(paths, p, "start_after must include entries after pivot")
	}
}

func testListWithVersions(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	path, _, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))

	_, err := op.Write(path, []byte("version-1"))
	assert.Nil(err, "first write must succeed")
	_, err = op.Write(path, []byte("version-2"))
	assert.Nil(err, "second write must succeed")

	obs, err := op.List(path, opendal.ListWithVersions(true))
	assert.Nil(err, "list with versions must succeed")
	defer obs.Close()

	var count int
	var currentCount int
	for obs.Next() {
		entry := obs.Entry()
		if entry.Path() == path {
			count++
			meta := entry.Metadata()
			version, ok := meta.Version()
			assert.True(ok, "version metadata must be present for list with versions")
			assert.NotEmpty(version, "each version entry must have a version ID")
			if curr, ok := meta.IsCurrent(); ok && curr {
				currentCount++
			}
		}
	}
	assert.Nil(obs.Error())
	assert.GreaterOrEqual(count, 2, "list with versions must return at least 2 entries for the same path")
	assert.Equal(1, currentCount, "exactly one version entry should be current")
}

func testListWithDeleted(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	parent := fixture.NewDirPath()
	path, content, _ := fixture.NewFileWithPath(fmt.Sprintf("%s%s", parent, uuid.NewString()))

	_, err := op.Write(path, content)
	assert.Nil(err, "write must succeed")

	obs, err := op.List(path, opendal.ListWithDeleted(true))
	assert.Nil(err, "list with deleted must succeed before deletion")
	defer obs.Close()
	var beforeCount int
	for obs.Next() {
		if obs.Entry().Path() == path {
			beforeCount++
		}
	}
	assert.Nil(obs.Error())
	assert.Equal(1, beforeCount, "active file must appear exactly once before deletion")

	assert.Nil(op.Delete(path), "delete must succeed")

	obs2, err := op.List(path, opendal.ListWithDeleted(true))
	assert.Nil(err, "list with deleted must succeed after deletion")
	defer obs2.Close()
	var foundDeleteMarker bool
	for obs2.Next() {
		entry := obs2.Entry()
		if entry.Path() == path {
			meta := entry.Metadata()
			if meta != nil && meta.IsDeleted() {
				version, ok := meta.Version()
				assert.True(ok, "delete marker must have a version ID")
				assert.NotEmpty(version, "delete marker must have a version ID")
				foundDeleteMarker = true
				break
			}
		}
	}
	assert.Nil(obs2.Error())
	assert.True(foundDeleteMarker, "delete marker must be found after deletion")
}
