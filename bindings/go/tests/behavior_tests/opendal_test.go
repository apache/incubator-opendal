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
	"crypto/rand"
	"fmt"
	"math/big"
	"os"
	"reflect"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	opendal "github.com/apache/opendal/bindings/go"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

var op *opendal.Operator

const defaultContentMaxSize uint = 4 * 1024 * 1024

func TestMain(m *testing.M) {
	var (
		closeFunc func()
		err       error
	)
	op, closeFunc, err = newOperator(
		opendal.WithTimeout(time.Minute, 10*time.Second),
		opendal.WithRetry(),
	)
	if err != nil {
		panic(err)
	}
	defer closeFunc()

	m.Run()
}

type behaviorTest = func(assert *require.Assertions, op *opendal.Operator, fixture *fixture)

func TestBehavior(t *testing.T) {
	assert := require.New(t)

	cap := op.Info().GetCapability()

	var tests []behaviorTest

	tests = append(tests, testsCopy(cap)...)
	tests = append(tests, testsCreateDir(cap)...)
	tests = append(tests, testsDelete(cap)...)
	tests = append(tests, testsList(cap)...)
	tests = append(tests, testsRead(cap)...)
	tests = append(tests, testsPresign(cap)...)
	tests = append(tests, testsRename(cap)...)
	tests = append(tests, testsStat(cap)...)
	tests = append(tests, testsWrite(cap)...)

	fixture := newFixture(op)

	t.Cleanup(func() {
		fixture.Cleanup(assert)
	})

	for i := range tests {
		test := tests[i]

		fullName := runtime.FuncForPC(reflect.ValueOf(test).Pointer()).Name()
		parts := strings.Split(fullName, ".")
		testName := strings.TrimPrefix(parts[len((parts))-1], "test")

		t.Run(testName, func(t *testing.T) {
			// Run all tests in parallel by default.
			// To run synchronously for specific services, set GOMAXPROCS=1.
			t.Parallel()
			assert := require.New(t)

			test(assert, op, fixture)
		})
	}
}

func newOperator(operatorOptions ...opendal.OperatorOption) (op *opendal.Operator, closeFunc func(), err error) {
	test := os.Getenv("OPENDAL_TEST")
	var scheme opendal.Scheme
	for _, s := range schemes {
		normalizedSchemeName := strings.ReplaceAll(test, "_", "-")
		if s.Name() != test && s.Name() != normalizedSchemeName {
			continue
		}
		err = s.LoadOnce()
		if err != nil {
			return
		}
		scheme = s
		break
	}
	if scheme == nil {
		err = fmt.Errorf("unsupported scheme: %s", test)
		return
	}

	prefix := fmt.Sprintf("OPENDAL_%s_", strings.ToUpper(scheme.Name()))

	opts := opendal.OperatorOptions{}
	for _, env := range os.Environ() {
		pair := strings.SplitN(env, "=", 2)
		if len(pair) != 2 {
			continue
		}
		key := pair[0]
		value := pair[1]
		if !strings.HasPrefix(key, prefix) {
			continue
		}
		opts[strings.ToLower(strings.TrimPrefix(key, prefix))] = value
	}

	op, err = opendal.NewOperator(scheme, opts, operatorOptions...)
	if err != nil {
		err = fmt.Errorf("create operator must succeed: %s", err)
	}

	closeFunc = func() {
		op.Close()

		os.Remove(scheme.Path())
	}

	return
}

func assertErrorCode(err error) opendal.ErrorCode {
	return err.(*opendal.Error).Code()
}

// parsedOverrides is lazily parsed from OPENDAL_TEST_CAPABILITY_OVERRIDES.
var parsedOverrides map[string]bool

func getCapOverrides() map[string]bool {
	if parsedOverrides != nil {
		return parsedOverrides
	}
	raw := os.Getenv("OPENDAL_TEST_CAPABILITY_OVERRIDES")
	parsedOverrides = make(map[string]bool)
	if raw == "" {
		return parsedOverrides
	}
	for _, pair := range strings.Split(raw, ",") {
		pair = strings.TrimSpace(pair)
		if pair == "" {
			continue
		}
		kv := strings.SplitN(pair, "=", 2)
		if len(kv) != 2 {
			continue
		}
		v, err := strconv.ParseBool(strings.TrimSpace(kv[1]))
		if err != nil {
			continue
		}
		parsedOverrides[strings.TrimSpace(kv[0])] = v
	}
	return parsedOverrides
}

// isCapEnabled checks both the capability reported by the operator and any
// test-level overrides set via OPENDAL_TEST_CAPABILITY_OVERRIDES.
func isCapEnabled(check func() bool, name string) bool {
	if !check() {
		return false
	}
	overrides := getCapOverrides()
	if v, ok := overrides[name]; ok {
		return v
	}
	return true
}

func genBytesWithRange(min, max uint) ([]byte, uint) {
	diff := max - min
	n, _ := rand.Int(rand.Reader, big.NewInt(int64(diff+1)))
	size := uint(n.Int64()) + min

	content := make([]byte, size)

	_, _ = rand.Read(content)

	return content, size
}

func genFixedBytes(size uint) []byte {
	content, _ := genBytesWithRange(size, size)
	return content
}

func genOffsetLength(size uint) (int64, int64) {
	// Generate a random offset and length within the given size
	offset, _ := rand.Int(rand.Reader, big.NewInt(int64(size-1)))
	length, _ := rand.Int(rand.Reader, big.NewInt(int64(size)-offset.Int64()))

	return offset.Int64(), length.Int64()
}

type fixture struct {
	op   *opendal.Operator
	lock *sync.Mutex

	paths []string
}

func newFixture(op *opendal.Operator) *fixture {
	return &fixture{
		op:   op,
		lock: &sync.Mutex{},
	}
}

func (f *fixture) NewDirPath() string {
	path := fmt.Sprintf("%s/", uuid.NewString())
	f.PushPath(path)

	return path
}

func (f *fixture) NewFilePath() string {
	path := uuid.NewString()
	f.PushPath(path)

	return path
}

func (f *fixture) NewFile() (string, []byte, uint) {
	return f.NewFileWithPath(uuid.NewString())
}

func (f *fixture) NewFileWithPath(path string) (string, []byte, uint) {
	maxSize := contentMaxSize(f.op.Info().GetCapability())
	return f.NewFileWithRange(path, 1, maxSize)
}

func (f *fixture) NewFileWithRange(path string, min, max uint) (string, []byte, uint) {
	f.PushPath(path)

	content, size := genBytesWithRange(min, max)
	return path, content, size
}

func contentMaxSize(cap *opendal.Capability) uint {
	maxSize := cap.WriteTotalMaxSize()
	if maxSize == 0 || maxSize > defaultContentMaxSize {
		return defaultContentMaxSize
	}
	return maxSize
}

func (f *fixture) Cleanup(assert *require.Assertions) {
	if !f.op.Info().GetCapability().Delete() {
		return
	}
	f.lock.Lock()
	defer f.lock.Unlock()

	for i := len(f.paths) - 1; i >= 0; i-- {
		f.op.Delete(f.paths[i])
	}
}

func (f *fixture) PushPath(path string) string {
	f.lock.Lock()
	defer f.lock.Unlock()

	f.paths = append(f.paths, path)

	return path
}
