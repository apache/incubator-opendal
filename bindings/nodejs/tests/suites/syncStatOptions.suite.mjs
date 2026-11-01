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
import { test, describe, expect } from 'vitest'
import { EntryMode } from '../../index.mjs'
import { generateFixedBytes } from '../utils.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()

  describe.skipIf(!capability.write || !capability.stat)('sync statOptions tests', () => {
    test.skipIf(!capability.statWithIfMatch)('stat with ifMatch', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(5 * 1024 * 1024)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const invalidOptions = {
        ifMatch: "'invalid_etag'",
      }

      expect(() => op.statSync(filename, invalidOptions)).toThrowError('ConditionNotMatch')

      const res = op.statSync(filename, {
        ifMatch: meta.etag,
      })

      expect(res.etag).toBe(meta.etag)
      op.deleteSync(filename)
    })

    test.skipIf(!capability.statWithIfNoneMatch)('stat with ifNoneMatch', () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(size))

      expect(() => op.statSync(filename, { ifNoneMatch: meta.etag })).toThrowError('ConditionNotMatch')

      const res = op.statSync(filename, {
        ifNoneMatch: '"invalid_etag"',
      })

      expect(res.mode).toBe(meta.mode)
      expect(res.contentLength).toBe(BigInt(size))
      op.deleteSync(filename)
    })

    test.skipIf(!capability.statWithIfModifiedSince)('stat with ifModifiedSince', () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(size))

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)
      const metaMinus = op.statSync(filename, { ifModifiedSince: sinceMinus.toISOString() })
      expect(metaMinus.lastModified).toBe(meta.lastModified)

      setTimeout(() => {
        const sinceAdd = new Date(meta.lastModified)
        sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)
        expect(() => op.statSync(filename, { ifModifiedSince: sinceAdd.toISOString() })).toThrowError(
          'ConditionNotMatch',
        )

        op.deleteSync(filename)
      }, 1000)
    })

    test.skipIf(!capability.statWithIfUnmodifiedSince)('stat with ifUnmodifiedSince', () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(size))

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)

      expect(() =>
        op.statSync(filename, { ifUnmodifiedSince: sinceMinus.toISOString() }).toThrowError('ConditionNotMatch'),
      )

      setTimeout(() => {
        const sinceAdd = new Date(meta.lastModified)
        sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

        const metaAdd = op.statSync(filename, { ifUnmodifiedSince: sinceAdd.toISOString() })
        expect(metaAdd.lastModified).toBe(meta.lastModified)

        op.deleteSync(filename)
      }, 1000)
    })

    test.skipIf(!capability.statWithVersion)('stat with version', () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const first_meta = op.statSync(filename)
      const first_version = first_meta.version

      const first_versioning_meta = op.statSync(filename, { version: first_version })
      expect(first_versioning_meta).toStrictEqual(first_meta)

      op.writeSync(filename, content)
      const second_meta = op.statSync(filename)
      const second_version = second_meta.version
      expect(second_version).not.toBe(first_version)

      const meta = op.statSync(filename, { version: first_version })
      expect(meta).toStrictEqual(first_meta)

      op.deleteSync(filename)
    })

    test.skipIf(!capability.statWithVersion)('stat with not existing version', () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)
      const version = meta.version

      const filename2 = `random_file_${randomUUID()}`
      op.writeSync(filename2, content)
      expect(() => op.statSync(filename2, { version })).toThrowError('NotFound')

      op.deleteSync(filename)
      op.deleteSync(filename2)
    })
  })
}
