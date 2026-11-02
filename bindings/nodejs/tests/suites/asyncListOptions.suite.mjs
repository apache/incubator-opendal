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
import path from 'node:path'
import { test, describe, expect, assert } from 'vitest'
import { EntryMode } from '../../index.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()

  describe.runIf(capability.write && capability.read && capability.list && capability.createDir)(
    'async listOptions tests',
    () => {
      test('test remove all', async () => {
        const parent = `random_remove_${randomUUID()}/`
        await op.createDir(parent)

        const expected = ['x/', 'x/y', 'x/x/', 'x/x/y', 'x/x/x/', 'x/x/x/y', 'x/x/x/x/']

        for await (const entry of expected) {
          if (entry.endsWith('/')) {
            await op.createDir(path.join(parent, entry))
          } else {
            await op.write(path.join(parent, entry), 'test_scan')
          }
        }

        await op.removeAll(path.join(parent, 'x/'))

        for await (const entry of expected) {
          if (entry.endsWith('/')) {
            continue
          }

          assert.isNotTrue(await op.exists(path.join(parent, entry)))
        }

        await op.removeAll(parent)
      })

      test.runIf(capability.listWithRecursive)('list with recursive', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const expected = ['x/', 'x/y', 'x/x/', 'x/x/y', 'x/x/x/', 'x/x/x/y', 'x/x/x/x/']
        for await (const entry of expected) {
          if (entry.endsWith('/')) {
            await op.createDir(path.join(dirname, entry))
          } else {
            await op.write(path.join(dirname, entry), 'test_scan')
          }
        }

        const lists = await op.list(dirname, { recursive: true })
        const actual = lists.filter((item) => item.metadata().isFile()).map((item) => item.path().slice(dirname.length))
        expect(actual.length).toEqual(3)

        expect(actual.sort()).toEqual(['x/x/x/y', 'x/x/y', 'x/y'])

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithStartAfter)('list with start after', async () => {
        const dirname = `random_dir_${randomUUID()}/`

        await op.createDir(dirname)

        const given = Array.from({ length: 6 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for await (const entry of given) {
          await op.write(entry, 'content')
        }

        const lists = await op.list(dirname, { startAfter: given[2] })
        const actual = lists.filter((item) => item.path() !== dirname).map((item) => item.path())
        const expected = given.slice(3)
        expect(actual).toEqual(expected)

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithVersions)('list with versions', async () => {
        const dirname = `random_dir_${randomUUID()}/`

        await op.createDir(dirname)

        const filePath = path.join(dirname, `random_file_${randomUUID()}`)
        await op.write(filePath, '1')
        await op.write(filePath, '2')
        const lists = await op.list(dirname, { versions: true })
        const actual = lists.filter((item) => item.path() !== dirname)
        expect(actual.length).toBe(2)
        expect(actual[0].metadata().version).not.toBe(actual[1].metadata().version)

        for await (const entry of actual) {
          expect(entry.path()).toBe(filePath)
          const meta = entry.metadata()
          expect(meta.mode).toBe(EntryMode.FILE)
        }

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithLimit)('list with limit', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const given = Array.from({ length: 5 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for await (const entry of given) {
          await op.write(entry, 'data')
        }

        const lists = await op.list(dirname, { limit: 3 })
        expect(lists.length).toBe(6)

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithVersions && capability.listWithStartAfter)(
        'list with versions and start after',
        async () => {
          const dirname = `random_dir_${randomUUID()}/`
          await op.createDir(dirname)
          const given = Array.from({ length: 6 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
          for await (const entry of given) {
            await op.write(entry, '1')
            await op.write(entry, '2')
          }
          const lists = await op.list(dirname, { versions: true, startAfter: given[2] })
          const actual = lists
            .filter((item) => item.path() !== dirname)
            .map((item) => item.path())
            .sort()
          const expected = given
            .slice(3)
            .flatMap((item) => [item, item])
            .sort()
          expect(actual).toEqual(expected)

          await op.removeAll(dirname)
        },
      )

      test.runIf(capability.listWithDeleted)('list with deleted', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)
        const filePath = path.join(dirname, `random_file_${randomUUID()}`)
        await op.write(filePath, '1')

        const lists = await op.list(filePath, { deleted: true })
        expect(lists.length).toBe(1)

        await op.write(filePath, '2')
        await op.delete(filePath)

        const ds = await op.list(filePath, { deleted: true })
        const actual = ds.filter((item) => item.path() === filePath && item.metadata().isDeleted())
        expect(actual.length).toBe(1)

        await op.removeAll(dirname)
      })
    },
  )
}
