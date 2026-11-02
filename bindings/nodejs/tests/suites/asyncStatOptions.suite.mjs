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
import { setTimeout } from 'node:timers/promises'

import { EntryMode } from '../../index.mjs'
import { generateFixedBytes } from '../utils.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()

  describe.skipIf(!capability.write || !capability.stat)('async statOptions tests', () => {
    test.skipIf(!capability.statWithIfMatch)('stat with ifMatch', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(5 * 1024 * 1024)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const invalidOptions = {
        ifMatch: '"invalid_etag"',
      }

      await expect(op.stat(filename, invalidOptions)).rejects.toThrowError('ConditionNotMatch')

      const res = await op.stat(filename, {
        ifMatch: meta.etag,
      })

      expect(res.etag).toBe(meta.etag)

      await op.delete(filename)
    })

    test.skipIf(!capability.statWithIfNoneMatch)('stat with ifNoneMatch', async () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(size))

      await expect(op.stat(filename, { ifNoneMatch: meta.etag })).rejects.toThrowError('ConditionNotMatch')

      const res = await op.stat(filename, {
        ifNoneMatch: '"invalid_etag"',
      })

      expect(res.mode).toBe(meta.mode)
      expect(res.contentLength).toBe(BigInt(meta.contentLength))

      await op.delete(filename)
    })

    test.skipIf(!capability.statWithIfModifiedSince)('stat with ifModifiedSince', async () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(size))

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)
      const metaMinus = await op.stat(filename, { ifModifiedSince: sinceMinus.toISOString() })
      expect(metaMinus.lastModified).toBe(meta.lastModified)

      await setTimeout(1000)

      const sinceAdd = new Date(meta.lastModified)
      sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

      await expect(op.stat(filename, { ifModifiedSince: sinceAdd.toISOString() })).rejects.toThrowError(
        'ConditionNotMatch',
      )

      await op.delete(filename)
    })

    test.skipIf(!capability.statWithIfUnmodifiedSince)('stat with ifUnmodifiedSince', async () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(size))

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)

      await expect(op.stat(filename, { ifUnmodifiedSince: sinceMinus.toISOString() })).rejects.toThrowError(
        'ConditionNotMatch',
      )

      await setTimeout(1000)

      const sinceAdd = new Date(meta.lastModified)
      sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

      const metaAdd = await op.stat(filename, { ifUnmodifiedSince: sinceAdd.toISOString() })
      expect(metaAdd.lastModified).toBe(meta.lastModified)

      await op.delete(filename)
    })

    test.skipIf(!capability.statWithVersion)('stat with version', async () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const first_meta = await op.stat(filename)
      const first_version = first_meta.version

      const first_versioning_meta = await op.stat(filename, { version: first_version })
      expect(first_versioning_meta).toStrictEqual(first_meta)

      await op.write(filename, content)
      const second_meta = await op.stat(filename)
      const second_version = second_meta.version
      expect(second_version).not.toBe(first_version)

      const meta = await op.stat(filename, { version: first_version })
      expect(meta).toStrictEqual(first_meta)

      await op.delete(filename)
    })

    test.skipIf(!capability.statWithVersion)('stat with not existing version', async () => {
      const size = 5 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)
      const version = meta.version

      const filename2 = `random_file_${randomUUID()}`
      await op.write(filename2, content)
      await expect(op.stat(filename2, { version })).rejects.toThrowError('NotFound')

      await op.delete(filename)
      await op.delete(filename2)
    })
  })
}
