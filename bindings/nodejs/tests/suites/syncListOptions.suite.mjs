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
    'sync listOptions tests',
    () => {
      test('test remove all', () => {
        const parent = `random_remove_${randomUUID()}/`
        op.createDirSync(parent)

        const expected = ['x/', 'x/y', 'x/x/', 'x/x/y', 'x/x/x/', 'x/x/x/y', 'x/x/x/x/']

        for (const entry of expected) {
          if (entry.endsWith('/')) {
            op.createDirSync(path.join(parent, entry))
          } else {
            op.writeSync(path.join(parent, entry), 'test_scan')
          }
        }

        op.removeAllSync(path.join(parent, 'x/'))

        for (const entry of expected) {
          if (entry.endsWith('/')) {
            continue
          }

          assert.isNotTrue(op.existsSync(path.join(parent, entry)))
        }

        op.removeAllSync(parent)
      })

      test.runIf(capability.listWithRecursive)('list with recursive', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const expected = ['x/', 'x/y', 'x/x/', 'x/x/y', 'x/x/x/', 'x/x/x/y', 'x/x/x/x/']
        for (const entry of expected) {
          if (entry.endsWith('/')) {
            op.createDirSync(path.join(dirname, entry))
          } else {
            op.writeSync(path.join(dirname, entry), 'test_scan')
          }
        }

        const lists = op.listSync(dirname, { recursive: true })
        const actual = lists.filter((item) => item.metadata().isFile()).map((item) => item.path().slice(dirname.length))
        expect(actual.length).toEqual(3)

        expect(actual.sort()).toEqual(['x/x/x/y', 'x/x/y', 'x/y'])

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithStartAfter)('list with start after', () => {
        const dirname = `random_dir_${randomUUID()}/`

        op.createDirSync(dirname)

        const given = Array.from({ length: 6 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for (const entry of given) {
          op.writeSync(entry, 'content')
        }

        const lists = op.listSync(dirname, { startAfter: given[2] })
        const actual = lists.filter((item) => item.path() !== dirname).map((item) => item.path())
        const expected = given.slice(3)
        expect(actual).toEqual(expected)

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithVersions)('list with versions', () => {
        const dirname = `random_dir_${randomUUID()}/`

        op.createDirSync(dirname)

        const filePath = path.join(dirname, `random_file_${randomUUID()}`)
        op.writeSync(filePath, '1')
        op.writeSync(filePath, '2')
        const lists = op.listSync(dirname, { versions: true })
        const actual = lists.filter((item) => item.path() !== dirname)
        expect(actual.length).toBe(2)
        expect(actual[0].metadata().version).not.toBe(actual[1].metadata().version)

        for (const entry of actual) {
          expect(entry.path()).toBe(filePath)
          const meta = entry.metadata()
          expect(meta.mode).toBe(EntryMode.FILE)
        }

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithLimit)('list with limit', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const given = Array.from({ length: 5 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for (const entry of given) {
          op.writeSync(entry, 'data')
        }

        const lists = op.listSync(dirname, { limit: 3 })
        expect(lists.length).toBe(6)

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithVersions && capability.listWithStartAfter)(
        'list with versions and start after',
        () => {
          const dirname = `random_dir_${randomUUID()}/`
          op.createDirSync(dirname)
          const given = Array.from({ length: 6 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
          for (const entry of given) {
            op.writeSync(entry, '1')
            op.writeSync(entry, '2')
          }
          const lists = op.listSync(dirname, { versions: true, startAfter: given[2] })
          const actual = lists
            .filter((item) => item.path() !== dirname)
            .map((item) => item.path())
            .sort()
          const expected = given
            .slice(3)
            .flatMap((item) => [item, item])
            .sort()
          expect(actual).toEqual(expected)

          op.removeAllSync(dirname)
        },
      )

      test.runIf(capability.listWithDeleted)('list with deleted', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)
        const filePath = path.join(dirname, `random_file_${randomUUID()}`)
        op.writeSync(filePath, '1')

        const lists = op.listSync(filePath, { deleted: true })
        expect(lists.length).toBe(1)

        op.writeSync(filePath, '2')
        op.deleteSync(filePath)

        const ds = op.listSync(filePath, { deleted: true })
        const actual = ds.filter((item) => item.path() === filePath && item.metadata().isDeleted())
        expect(actual.length).toBe(1)

        op.removeAllSync(dirname)
      })
    },
  )
}
