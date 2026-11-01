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

import { randomUUID } from 'node:crypto'
import { test, describe, expect, assert } from 'vitest'

import { generateBytes } from '../utils.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()

  describe.runIf(capability.write && capability.delete)('async delete options', () => {
    test('test delete file', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      op.deleteSync(filename)

      assert.isFalse(op.existsSync(filename))
    })

    test.runIf(capability.createDir)('test delete empty dir', () => {
      const dirname = `random_dir_${randomUUID()}/`
      op.createDirSync(dirname)
      op.deleteSync(dirname)

      assert.isFalse(op.existsSync(dirname))
    })

    test.runIf(capability.createDir)('test delete not existing', () => {
      const filename = `random_file_${randomUUID()}`

      op.deleteSync(filename)
      assert.isFalse(op.existsSync(filename))
    })

    test.runIf(capability.deleteWithVersion)('test delete with version', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      const meta = op.statSync(filename)
      const version = meta.version

      op.deleteSync(filename)
      assert.isFalse(op.existsSync(filename))

      const metadata = op.statSync(filename, { version })
      expect(metadata.version).toBe(version)

      op.deleteSync(filename, { version })

      expect(() => op.statSync(filename, { version })).toThrowError('NotFound')
    })

    test.runIf(capability.deleteWithVersion)('test delete with not existing version', () => {
      const filename1 = `random_file_${randomUUID()}`
      const content1 = generateBytes()

      op.writeSync(filename1, content1)
      const meta = op.statSync(filename1)
      const version = meta.version

      const filename2 = `random_file_${randomUUID()}`
      const content2 = generateBytes()
      op.writeSync(filename2, content2)

      op.deleteSync(filename2, { version })
      op.deleteSync(filename1)
    })
  })
}
