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

  describe.runIf(capability.read && capability.write && capability.stat)('sync writeOptions test', () => {
    test.runIf(capability.writeCanMulti)('write with concurrent', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content, {
        concurrent: 2,
        chunk: BigInt(1024 * 1024),
      })

      const bs = op.readSync(filename)
      assert.equal(Buffer.compare(bs, content), 0)

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithIfNotExists)('write with if not exists', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      const meta = op.writeSync(filename, content, {
        ifNotExists: true,
      })
      assert.instanceOf(meta, Metadata)

      expect(() => op.writeSync(filename, content, { ifNotExists: true })).toThrowError('ConditionNotMatch')

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithCacheControl)('write with cache control', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_cache_control = 'no-cache, no-store, max-age=300'
      op.writeSync(filename, content, {
        cacheControl: target_cache_control,
      })

      const meta = op.statSync(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.cacheControl).toBe(target_cache_control)

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithContentType)('write with content type', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_content_type = 'application/json'
      op.writeSync(filename, content, {
        contentType: target_content_type,
      })

      const meta = op.statSync(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentLength).toBe(BigInt(content.length))

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithContentDisposition)('write with content disposition', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_content_disposition = 'attachment; filename="filename.jpg"'
      op.writeSync(filename, content, {
        contentDisposition: target_content_disposition,
      })

      const meta = op.statSync(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentDisposition).toBe(target_content_disposition)
      expect(meta.contentLength).toBe(BigInt(content.length))

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithContentEncoding)('write with content encoding', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_content_encoding = 'gzip'
      op.writeSync(filename, content, {
        contentEncoding: target_content_encoding,
      })

      const meta = op.statSync(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.contentEncoding).toBe(target_content_encoding)

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithUserMetadata)('write with user metadata', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      const target_user_metadata = {
        location: 'everywhere',
      }
      op.writeSync(filename, content, {
        userMetadata: target_user_metadata,
      })

      const meta = op.statSync(filename)
      expect(meta.mode).toBe(EntryMode.FILE)
      expect(meta.userMetadata).toStrictEqual(target_user_metadata)

      op.deleteSync(filename)
    })

    test.runIf(capability.writeWithIfMatch)('write with if match', () => {
      const filenameA = `random_file_${randomUUID()}`
      const filenameB = `random_file_${randomUUID()}`
      const contentA = generateBytes()
      const contentB = generateBytes()

      op.writeSync(filenameA, contentA)
      op.writeSync(filenameB, contentB)

      const metaA = op.statSync(filenameA)
      const etagA = metaA.etag
      const metaB = op.statSync(filenameB)
      const etagB = metaB.etag

      const meta = op.writeSync(filenameA, contentA, { ifMatch: etagA })
      assert.instanceOf(meta, Metadata)

      expect(() => op.writeSync(filenameA, contentA, { ifMatch: etagB })).toThrowError('ConditionNotMatch')

      op.deleteSync(filenameA)
      op.deleteSync(filenameB)
    })

    test.runIf(capability.writeWithIfNoneMatch)('write with if none match', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      expect(() => op.writeSync(filename, content, { ifNoneMatch: meta.etag })).toThrowError('ConditionNotMatch')

      op.deleteSync(filename)
    })

    test.runIf(capability.writeCanAppend)('write with append', () => {
      const filename = `random_file_${randomUUID()}`
      const contentOne = generateBytes()
      const contentTwo = generateBytes()

      op.writeSync(filename, contentOne, { append: true })
      const meta = op.statSync(filename)

      expect(meta.contentLength).toBe(BigInt(contentOne.length))

      op.writeSync(filename, contentTwo, { append: true })

      const ds = op.readSync(filename)
      expect(contentOne.length + contentTwo.length).toBe(ds.length)
      expect(contentOne.length).toEqual(ds.subarray(0, contentOne.length).length)
      expect(contentTwo.length).toEqual(ds.subarray(contentOne.length).length)

      op.deleteSync(filename)
    })

    test.runIf(capability.writeCanAppend)('write with append returns metadata', () => {
      const filename = `random_file_${randomUUID()}`
      const contentOne = generateBytes()
      const contentTwo = generateBytes()

      op.writeSync(filename, contentOne, { append: true })
      const meta = op.writeSync(filename, contentTwo, { append: true })
      const statMeta = op.statSync(filename)
      expect(meta).toStrictEqual(statMeta)

      op.deleteSync(filename)
    })
  })
}
