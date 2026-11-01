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

import { EntryMode, Metadata } from '../../index.mjs'
import { generateBytes, generateFixedBytes } from '../utils.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()

  describe.runIf(capability.read && capability.write && capability.stat)('async writeOptions test', () => {
    test.runIf(capability.writeCanMulti)('write with concurrent', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content, {
        concurrent: 2,
        chunk: BigInt(1024 * 1024),
      })

      const bs = await op.read(filename)
      assert.equal(Buffer.compare(bs, content), 0)

      await op.delete(filename)
    })

    test.runIf(capability.writeWithIfNotExists)('write with if not exists', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      const meta = await op.write(filename, content, {
        ifNotExists: true,
      })
      assert.instanceOf(meta, Metadata)

      await expect(op.write(filename, content, { ifNotExists: true })).rejects.toThrowError('ConditionNotMatch')

      await op.delete(filename)
    })

    test.runIf(capability.writeWithCacheControl)('write with cache control', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_cache_control = 'no-cache, no-store, max-age=300'
      await op.write(filename, content, {
        cacheControl: target_cache_control,
      })

      const meta = await op.stat(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.cacheControl).toBe(target_cache_control)

      await op.delete(filename)
    })

    test.runIf(capability.writeWithContentType)('write with content type', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_content_type = 'application/json'
      await op.write(filename, content, {
        contentType: target_content_type,
      })

      const meta = await op.stat(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(content.length))

      await op.delete(filename)
    })

    test.runIf(capability.writeWithContentDisposition)('write with content disposition', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_content_disposition = 'attachment; filename="filename.jpg"'
      await op.write(filename, content, {
        contentDisposition: target_content_disposition,
      })

      const meta = await op.stat(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentDisposition).toBe(target_content_disposition)
      expect(meta.contentLength).toBe(BigInt(content.length))

      await op.delete(filename)
    })

    test.runIf(capability.writeWithContentEncoding)('write with content encoding', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_content_encoding = 'gzip'
      await op.write(filename, content, {
        contentEncoding: target_content_encoding,
      })

      const meta = await op.stat(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentEncoding).toBe(target_content_encoding)

      await op.delete(filename)
    })

    test.runIf(capability.writeWithUserMetadata)('write with user metadata', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_user_metadata = {
        location: 'everywhere',
      }
      await op.write(filename, content, {
        userMetadata: target_user_metadata,
      })

      const meta = await op.stat(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.userMetadata).toStrictEqual(target_user_metadata)

      await op.delete(filename)
    })

    test.runIf(capability.writeWithIfMatch)('write with if match', async () => {
      const filenameA = `random_file_${randomUUID()}`
      const filenameB = `random_file_${randomUUID()}`
      const contentA = generateBytes()
      const contentB = generateBytes()

      await op.write(filenameA, contentA)
      await op.write(filenameB, contentB)

      const metaA = await op.stat(filenameA)
      const etagA = metaA.etag
      const metaB = await op.stat(filenameB)
      const etagB = metaB.etag

      const meta = await op.write(filenameA, contentA, { ifMatch: etagA })
      assert.instanceOf(meta, Metadata)

      await expect(op.write(filenameA, contentA, { ifMatch: etagB })).rejects.toThrowError('ConditionNotMatch')

      await op.delete(filenameA)
      await op.delete(filenameB)
    })

    test.runIf(capability.writeWithIfNoneMatch)('write with if none match', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      const meta = await op.stat(filename)

      await expect(op.write(filename, content, { ifNoneMatch: meta.etag })).rejects.toThrowError('ConditionNotMatch')

      await op.delete(filename)
    })

    test.runIf(capability.writeCanAppend)('write with append', async () => {
      const filename = `random_file_${randomUUID()}`
      const contentOne = generateBytes()
      const contentTwo = generateBytes()

      await op.write(filename, contentOne, { append: true })
      const meta = await op.stat(filename)

      expect(meta.contentLength).toBe(BigInt(contentOne.length))

      await op.write(filename, contentTwo, { append: true })

      const ds = await op.read(filename)
      expect(contentOne.length + contentTwo.length).toBe(ds.length)
      expect(contentOne.length).toEqual(ds.subarray(0, contentOne.length).length)
      expect(contentTwo.length).toEqual(ds.subarray(contentOne.length).length)

      await op.delete(filename)
    })

    test.runIf(capability.writeCanAppend)('write with append returns metadata', async () => {
      const filename = `random_file_${randomUUID()}`
      const contentOne = generateBytes()
      const contentTwo = generateBytes()

      await op.write(filename, contentOne, { append: true })
      const meta = await op.write(filename, contentTwo, { append: true })
      const statMeta = await op.stat(filename)
      expect(meta).toStrictEqual(statMeta)

      await op.delete(filename)
    })
  })
}
