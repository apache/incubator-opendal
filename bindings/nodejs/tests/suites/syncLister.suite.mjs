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
    'sync lister tests',
    () => {
      test('test basic lister', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const expected = ['file1', 'file2', 'file3']
        for (const entry of expected) {
          op.writeSync(path.join(dirname, entry), 'test_content')
        }

        const lister = op.listerSync(dirname)
        const actual = []
        let entry
        while ((entry = lister.next()) !== null) {
          if (entry.path() !== dirname) {
            actual.push(entry.path().slice(dirname.length))
          }
        }

        expect(actual.sort()).toEqual(expected.sort())

        op.removeAllSync(dirname)
      })

      test('test lister returns same results as list', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const expected = Array.from({ length: 5 }, (_, i) => `file_${i}_${randomUUID()}`)
        for (const entry of expected) {
          op.writeSync(path.join(dirname, entry), 'content')
        }

        // Get results using listSync()
        const listResults = op.listSync(dirname)
        const listPaths = listResults
          .filter((item) => item.path() !== dirname)
          .map((item) => item.path())
          .sort()

        // Get results using listerSync()
        const lister = op.listerSync(dirname)
        const listerPaths = []
        let entry
        while ((entry = lister.next()) !== null) {
          if (entry.path() !== dirname) {
            listerPaths.push(entry.path())
          }
        }
        listerPaths.sort()

        expect(listerPaths).toEqual(listPaths)

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithRecursive)('lister with recursive', () => {
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

        const lister = op.listerSync(dirname, { recursive: true })
        const actual = []
        let entry
        while ((entry = lister.next()) !== null) {
          if (entry.metadata().isFile()) {
            actual.push(entry.path().slice(dirname.length))
          }
        }

        expect(actual.length).toEqual(3)
        expect(actual.sort()).toEqual(['x/x/x/y', 'x/x/y', 'x/y'])

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithStartAfter)('lister with start after', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const given = Array.from({ length: 6 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for (const entry of given) {
          op.writeSync(entry, 'content')
        }

        const lister = op.listerSync(dirname, { startAfter: given[2] })
        const actual = []
        let entry
        while ((entry = lister.next()) !== null) {
          if (entry.path() !== dirname) {
            actual.push(entry.path())
          }
        }

        const expected = given.slice(3)
        expect(actual).toEqual(expected)

        op.removeAllSync(dirname)
      })

      test.runIf(capability.listWithLimit)('lister with limit', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const given = Array.from({ length: 10 }, (_, i) => path.join(dirname, `file_${i}_${randomUUID()}`))
        for (const entry of given) {
          op.writeSync(entry, 'data')
        }

        const lister = op.listerSync(dirname, { limit: 5 })
        const actual = []
        let entry
        while ((entry = lister.next()) !== null) {
          actual.push(entry.path())
        }

        // With limit, we should get the paginated results
        expect(actual.length).toBeGreaterThan(0)

        op.removeAllSync(dirname)
      })

      test('test empty directory lister', () => {
        const dirname = `empty_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const lister = op.listerSync(dirname)
        let entry
        let count = 0
        while ((entry = lister.next()) !== null) {
          if (entry.path() !== dirname) {
            count++
          }
        }

        expect(count).toEqual(0)

        op.removeAllSync(dirname)
      })

      test('test lister metadata', () => {
        const dirname = `random_dir_${randomUUID()}/`
        op.createDirSync(dirname)

        const filename = `file_${randomUUID()}`
        const content = 'test content for metadata'
        op.writeSync(path.join(dirname, filename), content)

        const lister = op.listerSync(dirname)
        let entry
        let found = false
        while ((entry = lister.next()) !== null) {
          if (entry.path() === path.join(dirname, filename)) {
            const meta = entry.metadata()
            expect(meta.isFile()).toBe(true)
            expect(meta.isDirectory()).toBe(false)
            found = true
          }
        }

        expect(found).toBe(true)

        op.removeAllSync(dirname)
      })
    },
  )
}
