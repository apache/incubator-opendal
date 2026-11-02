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
import { test, describe, expect } from 'vitest'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()

  describe.runIf(capability.write && capability.read && capability.list && capability.createDir)(
    'async lister tests',
    () => {
      test('test basic lister', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const expected = ['file1', 'file2', 'file3']
        for (const entry of expected) {
          await op.write(path.join(dirname, entry), 'test_content')
        }

        const lister = await op.lister(dirname)
        const actual = []
        let entry
        while ((entry = await lister.next()) !== null) {
          if (entry.path() !== dirname) {
            actual.push(entry.path().slice(dirname.length))
          }
        }

        expect(actual.sort()).toEqual(expected.sort())

        await op.removeAll(dirname)
      })

      test('test lister returns same results as list', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const expected = Array.from({ length: 5 }, (_, i) => `file_${i}_${randomUUID()}`)
        for (const entry of expected) {
          await op.write(path.join(dirname, entry), 'content')
        }

        // Get results using list()
        const listResults = await op.list(dirname)
        const listPaths = listResults
          .filter((item) => item.path() !== dirname)
          .map((item) => item.path())
          .sort()

        // Get results using lister()
        const lister = await op.lister(dirname)
        const listerPaths = []
        let entry
        while ((entry = await lister.next()) !== null) {
          if (entry.path() !== dirname) {
            listerPaths.push(entry.path())
          }
        }
        listerPaths.sort()

        expect(listerPaths).toEqual(listPaths)

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithRecursive)('lister with recursive', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const expected = ['x/', 'x/y', 'x/x/', 'x/x/y', 'x/x/x/', 'x/x/x/y', 'x/x/x/x/']
        for (const entry of expected) {
          if (entry.endsWith('/')) {
            await op.createDir(path.join(dirname, entry))
          } else {
            await op.write(path.join(dirname, entry), 'test_scan')
          }
        }

        const lister = await op.lister(dirname, { recursive: true })
        const actual = []
        let entry
        while ((entry = await lister.next()) !== null) {
          if (entry.metadata().isFile()) {
            actual.push(entry.path().slice(dirname.length))
          }
        }

        expect(actual.length).toEqual(3)
        expect(actual.sort()).toEqual(['x/x/x/y', 'x/x/y', 'x/y'])

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithStartAfter)('lister with start after', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const given = Array.from({ length: 6 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for (const entry of given) {
          await op.write(entry, 'content')
        }

        const lister = await op.lister(dirname, { startAfter: given[2] })
        const actual = []
        let entry
        while ((entry = await lister.next()) !== null) {
          if (entry.path() !== dirname) {
            actual.push(entry.path())
          }
        }

        const expected = given.slice(3)
        expect(actual).toEqual(expected)

        await op.removeAll(dirname)
      })

      test.runIf(capability.listWithLimit)('lister with limit', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const given = Array.from({ length: 10 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for (const entry of given) {
          await op.write(entry, 'data')
        }

        const lister = await op.lister(dirname, { limit: 5 })
        const actual = []
        let entry
        while ((entry = await lister.next()) !== null) {
          actual.push(entry.path())
        }

        // With limit, we should get the paginated results
        expect(actual.length).toBeGreaterThan(0)

        await op.removeAll(dirname)
      })

      test('test empty directory lister', async () => {
        const dirname = `empty_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const lister = await op.lister(dirname)
        let entry
        let count = 0
        while ((entry = await lister.next()) !== null) {
          if (entry.path() !== dirname) {
            count++
          }
        }

        expect(count).toEqual(0)

        await op.removeAll(dirname)
      })

      test('test lister metadata', async () => {
        const dirname = `random_dir_${randomUUID()}/`
        await op.createDir(dirname)

        const filename = `file_${randomUUID()}`
        const content = 'test content for metadata'
        await op.write(path.join(dirname, filename), content)

        const lister = await op.lister(dirname)
        let entry
        let found = false
        while ((entry = await lister.next()) !== null) {
          if (entry.path() === path.join(dirname, filename)) {
            const meta = entry.metadata()
            expect(meta.isFile()).toBe(true)
            expect(meta.isDirectory()).toBe(false)
            found = true
          }
        }

        expect(found).toBe(true)

        await op.removeAll(dirname)
      })
    },
  )
}
