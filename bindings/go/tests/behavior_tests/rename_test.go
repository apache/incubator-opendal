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

	"github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func testsRename(cap *opendal.Capability) []behaviorTest {
	if !cap.Read() || !cap.Write() || !cap.Rename() {
		return nil
	}
	return []behaviorTest{
		testRenameFile,
		testRenameNonExistingSource,
		testRenameSourceDir,
		testRenameTargetDir,
		testRenameSelf,
		testRenameNested,
		testRenameOverwrite,
	}
}

func testRenameFile(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err, "write must succeed")

	targetPath := fixture.NewFilePath()

	assert.Nil(op.Rename(sourcePath, targetPath))

	_, err = op.Stat(sourcePath)
	assert.NotNil(err, "stat must fail")
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))

	targetContent, err := op.Read(targetPath)
	assert.Nil(err)
	assert.Equal(sourceContent, targetContent)
}

func testRenameNonExistingSource(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath := fixture.NewFilePath()
	targetPath := fixture.NewFilePath()

	err := op.Rename(sourcePath, targetPath)
	assert.NotNil(err, "rename must fail")
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))
}

func testRenameSourceDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().CreateDir() {
		return
	}

	sourcePath := fixture.NewDirPath()
	targetPth := fixture.NewFilePath()

	assert.Nil(op.CreateDir(sourcePath), "create must succeed")

	err := op.Rename(sourcePath, targetPth)
	assert.NotNil(err, "rename must fail")
	assert.Equal(opendal.CodeIsADirectory, assertErrorCode(err))
}

func testRenameTargetDir(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	if !op.Info().GetCapability().CreateDir() {
		return
	}

	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err, "write must succeed")

	targetPath := fixture.NewDirPath()
	assert.Nil(op.CreateDir(targetPath))

	err = op.Rename(sourcePath, targetPath)
	assert.NotNil(err)
	assert.Equal(opendal.CodeIsADirectory, assertErrorCode(err))
}

func testRenameSelf(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err, "write must succeed")

	err = op.Rename(sourcePath, sourcePath)
	assert.NotNil(err)
	assert.Equal(opendal.CodeIsSameFile, assertErrorCode(err))
}

func testRenameNested(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err, "write must succeed")

	targetPath := fixture.PushPath(fmt.Sprintf(
		"%s/%s/%s",
		uuid.NewString(),
		uuid.NewString(),
		uuid.NewString(),
	))

	assert.Nil(op.Rename(sourcePath, targetPath))

	_, err = op.Stat(sourcePath)
	assert.NotNil(err, "stat must fail")
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))

	targetContent, err := op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}

func testRenameOverwrite(assert *require.Assertions, op *opendal.Operator, fixture *fixture) {
	sourcePath, sourceContent, _ := fixture.NewFile()

	_, err := op.Write(sourcePath, sourceContent)
	assert.Nil(err, "write must succeed")

	targetPath, targetContent, _ := fixture.NewFile()
	assert.NotEqual(sourceContent, targetContent)

	_, err = op.Write(targetPath, targetContent)
	assert.Nil(err, "write must succeed")

	assert.Nil(op.Rename(sourcePath, targetPath))

	_, err = op.Stat(sourcePath)
	assert.NotNil(err, "stat must fail")
	assert.Equal(opendal.CodeNotFound, assertErrorCode(err))

	targetContent, err = op.Read(targetPath)
	assert.Nil(err, "read must succeed")
	assert.Equal(sourceContent, targetContent)
}
