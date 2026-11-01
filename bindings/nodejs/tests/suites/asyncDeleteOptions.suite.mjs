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
    test('test delete file', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      await op.delete(filename)

      assert.isFalse(await op.exists(filename))
    })

    test.runIf(capability.createDir)('test delete empty dir', async () => {
      const dirname = `random_dir_${randomUUID()}/`
      await op.createDir(dirname)
      await op.delete(dirname)

      assert.isFalse(await op.exists(dirname))
    })

    test.runIf(capability.createDir)('test delete not existing', async () => {
      const filename = `random_file_${randomUUID()}`

      await op.delete(filename)
      assert.isFalse(await op.exists(filename))
    })

    test.runIf(capability.deleteWithVersion)('test delete with version', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      const meta = await op.stat(filename)
      const version = meta.version

      await op.delete(filename)
      assert.isFalse(await op.exists(filename))

      const metadata = await op.stat(filename, { version })
      expect(metadata.version).toBe(version)

      await op.delete(filename, { version })

      await expect(op.stat(filename, { version })).rejects.toThrowError('NotFound')
    })

    test.runIf(capability.deleteWithVersion)('test delete with not existing version', async () => {
      const filename1 = `random_file_${randomUUID()}`
      const content1 = generateBytes()

      await op.write(filename1, content1)
      const meta = await op.stat(filename1)
      const version = meta.version

      const filename2 = `random_file_${randomUUID()}`
      const content2 = generateBytes()
      await op.write(filename2, content2)

      await op.delete(filename2, { version })
      await op.delete(filename1)
    })
  })
}
