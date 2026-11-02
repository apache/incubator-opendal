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

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func testsCopy(cap *opendal.Capability) []behaviorTest {
	if !cap.Read() || !cap.Write() || !cap.Copy() {
		return nil
	}
	tests := []behaviorTest{
		testCopyFileWithASCIIName,
		testCopyFileWithNonASCIIName,
		testCopyNonExistingSource,
		testCopySelf,
		testCopyNested,
		testCopyOverwrite,
		testCopierFile,
		testCopierCompletion,
		testCopierAbort,
		testCopierClosed,
	}
	if cap.CreateDir() {
		tests = append(tests, testCopySourceDir, testCopyTargetDir)
	}
	if isCapEnabled(cap.CopyWithIfNotExists, "copy_with_if_not_exists") {
		tests = append(tests, testCopyWithIfNotExistsToNewFile, testCopyWithIfNotExistsToExistingFile,
			testCopierWithIfNotExistsToNewFile, testCopierWithIfNotExistsToExistingFile)
	}
	if isCapEnabled(cap.CopyWithIfMatch, "copy_with_if_match") {
		tests = append(tests, testCopyWithIfMatchMatch, testCopyWithIfMatchMismatch)
	}
	if isCapEnabled(cap.CopyWithSourceVersion, "copy_with_source_version") {
		tests = append(tests, testCopyWithSourceVersionToNewFile, testCopyWithSourceVersionToSameFile)
	}
	if isCapEnabled(cap.CopyCanMulti, "copy_can_multi") {
		tests = append(tests, testCopyWithChunk, testCopyWithChunkAndConcurrent, testCopierMultipart)
	}
	return tests
}

func testCopyFileWithASCIIName(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()

	assert.Nil(op.Copy(sourcePath, targetPath))

	targetContent, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}

func testCopyFileWithNonASCIIName(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFileWithPath("🐂🍺中文.docx")
	targetPath := fixture.PushPath("😈🐅Français.docx")

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)
	assert.Nil(op.Copy(sourcePath, targetPath))

	targetContent, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}

func testCopyNonExistingSource(assert *require.Assertions, op *opendal.Operator, _ *fixture) {
	sourcePath := uuid.NewString()
	targetPath := uuid.NewString()

	err := op.Copy(sourcePath, targetPath)
	assert.NotNil(err, "copy must fail")
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))
}

func testCopySourceDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath := fixture.NewDirPath()
	targetPath := uuid.NewString()

	assert.Nil(op.CreateDir(sourcePath))

	err := op.Copy(sourcePath, targetPath)
	assert.NotNil(err, "copy must fail")
	assert.Equal(opendal.CodeIsADirectory, assertErrorCode(err))
}

func testCopyTargetDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewDirPath()

	assert.Nil(op.CreateDir(targetPath))

	err = op.Copy(sourcePath, targetPath)
	assert.NotNil(err, "copy must fail")
	assert.Equal(opendal.CodeIsADirectory, assertErrorCode(err))
}

func testCopySelf(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	err = op.Copy(sourcePath, sourcePath)
	assert.NotNil(err, "copy must fail")
	assert.Equal(opendal.CodeIsSameFile, assertErrorCode(err))
}

func testCopyNested(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.PushPath(fmt.Sprintf(
		"%s/%s/%s",
		uuid.NewString(),
		uuid.NewString(),
		uuid.NewString(),
	))

	assert.Nil(op.Copy(sourcePath, targetPath))

	targetContent, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}

func testCopyOverwrite(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath, targetContent, _ := fixture.NewFile()
	assert.NotEqual(sourceContent, targetContent)

	_, err = op.Write(targetPath, targetContent)
	assert.Nil(err)

	assert.Nil(op.Copy(sourcePath, targetPath))

	targetContent, err = op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}

func driveCopier(copier *opendal.Copier) error {
	for {
		_, ok, err := copier.Next()
		if err != nil {
			return err
		}
		if !ok {
			return nil
		}
	}
}

func testCopierFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	copier, err := op.Copier(sourcePath, targetPath)
	assert.Nil(err, "create copier must succeed")
	defer copier.Close()

	assert.Nil(driveCopier(copier), "copier must drive to completion")

	targetContent, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}

func testCopierCompletion(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	copier, err := op.Copier(sourcePath, targetPath)
	assert.Nil(err, "create copier must succeed")
	defer copier.Close()

	assert.Nil(driveCopier(copier), "copier must drive to completion")

	_, ok, err := copier.Next()
	assert.Nil(err, "next on completed copier must not error")
	assert.False(ok, "completed copier must keep reporting no progress")
}

func testCopierAbort(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	copier, err := op.Copier(sourcePath, targetPath)
	assert.Nil(err, "create copier must succeed")
	defer copier.Close()

	assert.Nil(copier.Abort(), "abort must succeed")
}

func testCopierClosed(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	copier, err := op.Copier(sourcePath, targetPath)
	assert.Nil(err, "create copier must succeed")

	assert.Nil(copier.Close(), "close must succeed")
	assert.Nil(copier.Close(), "close must be idempotent")

	_, ok, err := copier.Next()
	assert.Nil(err, "next on a closed copier must not error")
	assert.False(ok, "closed copier must report no progress")
	assert.Nil(copier.Abort(), "abort on a closed copier must be a no-op")
}

func testCopierWithIfNotExistsToNewFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	copier, err := op.Copier(sourcePath, targetPath, opendal.CopyWithIfNotExists(true))
	assert.Nil(err, "create copier must succeed")
	defer copier.Close()

	assert.Nil(driveCopier(copier), "copier must drive to completion")

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func testCopierWithIfNotExistsToExistingFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath, targetContent, _ := fixture.NewFile()
	_, err = op.Write(targetPath, targetContent)
	assert.Nil(err)

	copier, err := op.Copier(sourcePath, targetPath, opendal.CopyWithIfNotExists(true))
	assert.Nil(err, "create copier must succeed")
	defer copier.Close()

	err = driveCopier(copier)
	assert.NotNil(err, "copier must fail when target exists")
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(targetContent, bs, "target must not be overwritten")
}

func testCopyWithIfNotExistsToNewFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	assert.Nil(op.Copy(sourcePath, targetPath, opendal.CopyWithIfNotExists(true)))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func testCopyWithIfNotExistsToExistingFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath, targetContent, _ := fixture.NewFile()
	_, err = op.Write(targetPath, targetContent)
	assert.Nil(err)

	err = op.Copy(sourcePath, targetPath, opendal.CopyWithIfNotExists(true))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(targetContent, bs, "target must not be overwritten")
}

func testCopyWithIfMatchMatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath, targetContent, _ := fixture.NewFile()
	assert.NotEqual(sourceContent, targetContent)
	_, err = op.Write(targetPath, targetContent)
	assert.Nil(err)

	meta, err := op.Stat(targetPath)
	assert.Nil(err, "stat must succeed")
	etag, ok := meta.ETag()
	assert.True(ok, "etag must exist")

	assert.Nil(op.Copy(sourcePath, targetPath, opendal.CopyWithIfMatch(etag)))
	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func testCopyWithIfMatchMismatch(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath, targetContent, _ := fixture.NewFile()
	assert.NotEqual(sourceContent, targetContent)
	_, err = op.Write(targetPath, targetContent)
	assert.Nil(err)

	err = op.Copy(sourcePath, targetPath, opendal.CopyWithIfMatch("wrong-etag"))
	assert.NotNil(err)
	assert.Equal(opendal.CodeConditionNotMatch, assertErrorCode(err))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(targetContent, bs, "target must not be overwritten")
}

func testCopyWithSourceVersionToNewFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	meta, err := op.Stat(sourcePath)
	assert.Nil(err, "stat must succeed")
	version, ok := meta.Version()
	if !ok {
		return
	}

	newContent := genFixedBytes(uint(len(sourceContent)) + 1)
	_, err = op.Write(sourcePath, newContent)
	assert.Nil(err, "overwrite must succeed")

	targetPath := fixture.NewFilePath()
	assert.Nil(op.Copy(sourcePath, targetPath, opendal.CopyWithSourceVersion(version)))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func testCopyWithSourceVersionToSameFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	meta, err := op.Stat(sourcePath)
	assert.Nil(err, "stat must succeed")
	version, ok := meta.Version()
	if !ok {
		return
	}

	newContent := genFixedBytes(uint(len(sourceContent)) + 1)
	_, err = op.Write(sourcePath, newContent)
	assert.Nil(err, "overwrite must succeed")

	assert.Nil(op.Copy(sourcePath, sourcePath, opendal.CopyWithSourceVersion(version)))

	bs, err := op.Read(sourcePath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func copyMultiChunkSize(cap *opendal.Capability) (uint, uint) {
	chunk := cap.CopyMultiMinSize()
	if chunk == 0 {
		return 0, 0
	}
	maxChunk := cap.CopyMultiMaxSize()
	if maxChunk != 0 && chunk > maxChunk {
		return 0, 0
	}
	return chunk, chunk + 1
}

func testCopyWithChunk(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	chunk, sourceSize := copyMultiChunkSize(op.Info().GetCapability())
	if sourceSize == 0 {
		return
	}

	sourcePath := fixture.NewFilePath()
	sourceContent := genFixedBytes(sourceSize)
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	assert.Nil(op.Copy(sourcePath, targetPath, opendal.CopyWithChunk(uint(chunk))))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func testCopyWithChunkAndConcurrent(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	chunk, sourceSize := copyMultiChunkSize(op.Info().GetCapability())
	if sourceSize == 0 {
		return
	}

	sourcePath := fixture.NewFilePath()
	sourceContent := genFixedBytes(sourceSize)
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	assert.Nil(op.Copy(sourcePath, targetPath, opendal.CopyWithChunk(uint(chunk)), opendal.CopyWithConcurrent(4)))

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}

func testCopierMultipart(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	chunk, sourceSize := copyMultiChunkSize(op.Info().GetCapability())
	if sourceSize == 0 {
		return
	}

	sourcePath := fixture.NewFilePath()
	sourceContent := genFixedBytes(sourceSize)
	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err)

	targetPath := fixture.NewFilePath()
	copier, err := op.Copier(sourcePath, targetPath, opendal.CopyWithChunk(uint(chunk)))
	assert.Nil(err, "create copier must succeed")
	defer copier.Close()

	var steps int
	var total uint
	for {
		n, ok, err := copier.Next()
		assert.Nil(err, "copier next must succeed")
		if !ok {
			break
		}
		steps++
		total += n
	}

	assert.Greater(steps, 0, "multipart copier must report incremental progress")
	assert.Equal(uint(sourceSize), total, "reported progress must sum to source size")

	bs, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, bs)
}
