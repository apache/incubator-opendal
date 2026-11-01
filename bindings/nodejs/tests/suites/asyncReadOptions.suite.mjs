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
import { Writable } from 'node:stream'
import { finished, pipeline } from 'node:stream/promises'
import { setTimeout } from 'node:timers/promises'

import { generateBytes, generateFixedBytes } from '../utils.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()
  const readerWithIfMatch = capability.readWithIfMatch && capability.statWithIfMatch
  const readerWithIfNoneMatch = capability.readWithIfNoneMatch && capability.statWithIfNoneMatch
  const readerWithIfModifiedSince = capability.readWithIfModifiedSince && capability.statWithIfModifiedSince
  const readerWithIfUnmodifiedSince = capability.readWithIfUnmodifiedSince && capability.statWithIfUnmodifiedSince

  describe.runIf(capability.read && capability.write)('async read options', () => {
    test('read with range', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)
      const offset = Math.floor(Math.random() * (size - 1))
      const maxLen = size - offset
      const length = Math.floor(Math.random() * (maxLen - 1)) + 1

      await op.write(filename, content)

      const bs = await op.read(filename, {
        offset: BigInt(offset),
        size: BigInt(length),
      })
      expect(bs.length).toBe(length)
      assert.equal(Buffer.compare(bs, content.subarray(offset, offset + length)), 0)

      await op.delete(filename)
    })

    test.runIf(capability.readWithIfMatch)('read with if match', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const invalidOptions = {
        ifMatch: '"invalid_etag"',
      }

      await expect(op.read(filename, invalidOptions)).rejects.toThrowError('ConditionNotMatch')

      const bs = await op.read(filename, {
        ifMatch: meta.etag,
      })

      assert.equal(Buffer.compare(bs, content), 0)

      await op.delete(filename)
    })

    test.runIf(capability.readWithIfNoneMatch)('read with if none match', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const invalidOptions = {
        ifNoneMatch: meta.etag,
      }

      await expect(op.read(filename, invalidOptions)).rejects.toThrowError('ConditionNotMatch')

      const bs = await op.read(filename, {
        ifNoneMatch: '"invalid_etag"',
      })

      assert.equal(Buffer.compare(bs, content), 0)

      await op.delete(filename)
    })

    test.runIf(capability.readWithIfModifiedSince)('read with if modified since', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)
      const bs = await op.read(filename, { ifModifiedSince: sinceMinus.toISOString() })
      assert.equal(Buffer.compare(bs, content), 0)

      await setTimeout(1000)

      const sinceAdd = new Date(meta.lastModified)
      sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

      await expect(op.read(filename, { ifModifiedSince: sinceAdd.toISOString() })).rejects.toThrowError(
        'ConditionNotMatch',
      )
      await op.delete(filename)
    })

    test.runIf(capability.readWithIfUnmodifiedSince)('read with if unmodified since', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 3600)
      await expect(op.read(filename, { ifUnmodifiedSince: sinceMinus.toISOString() })).rejects.toThrowError(
        'ConditionNotMatch',
      )

      await setTimeout(1000)

      const sinceAdd = new Date(meta.lastModified)
      sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

      const bs = await op.read(filename, { ifUnmodifiedSince: sinceAdd.toISOString() })
      assert.equal(Buffer.compare(bs, content), 0)

      await op.delete(filename)
    })

    test.runIf(capability.readWithVersion)('read with version', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      const meta = await op.stat(filename)
      const data = await op.read(filename, { version: meta.version })
      assert.equal(Buffer.compare(data, content), 0)

      await op.write(filename, Buffer.from('1'))
      // After writing new data, we can still read the first version data
      const secondData = await op.read(filename, { version: meta.version })
      assert.equal(Buffer.compare(secondData, content), 0)

      await op.delete(filename)
    })

    test.runIf(capability.readWithVersion)('read with not existing version', async () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const filename2 = `random_file_${randomUUID()}`
      const content2 = generateBytes()
      await op.write(filename2, content2)

      await expect(op.read(filename2, { version: meta.version })).rejects.toThrowError('NotFound')

      await op.delete(filename)
      await op.delete(filename2)
    })
  })

  describe.runIf(capability.read && capability.write)('async reader options', () => {
    test.runIf(readerWithIfMatch)('reader with if match', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const invalidOptions = {
        ifMatch: '"invalid_etag"',
      }

      const invalidReader = await op.reader(filename, invalidOptions)
      await expect(invalidReader.read(Buffer.alloc(1))).rejects.toThrowError('ConditionNotMatch')

      const r = await op.reader(filename, { ifMatch: meta.etag })
      const rs = r.createReadStream()
      let chunks = []
      await pipeline(
        rs,
        new Writable({
          write(chunk, encoding, callback) {
            chunks.push(chunk)
            callback()
          },
        }),
      )

      await finished(rs)
      const bs = Buffer.concat(chunks)

      assert.equal(Buffer.compare(bs, content), 0)
      await op.delete(filename)
    })

    test.runIf(readerWithIfNoneMatch)('reader with if none match', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const invalidReader = await op.reader(filename, { ifNoneMatch: meta.etag })
      await expect(invalidReader.read(Buffer.alloc(1))).rejects.toThrowError('ConditionNotMatch')

      const r = await op.reader(filename, { ifNoneMatch: '"invalid_etag"' })
      const rs = r.createReadStream()
      let chunks = []
      await pipeline(
        rs,
        new Writable({
          write(chunk, encoding, callback) {
            chunks.push(chunk)
            callback()
          },
        }),
      )

      await finished(rs)
      const bs = Buffer.concat(chunks)

      assert.equal(Buffer.compare(bs, content), 0)
      await op.delete(filename)
    })

    test.runIf(readerWithIfModifiedSince)('reader with if modified since', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)
      const reader = await op.reader(filename, { ifModifiedSince: sinceMinus.toISOString() })
      const rs = reader.createReadStream()
      let chunks = []
      await pipeline(
        rs,
        new Writable({
          write(chunk, encoding, callback) {
            chunks.push(chunk)
            callback()
          },
        }),
      )

      await finished(rs)
      const buf = Buffer.concat(chunks)
      assert.equal(Buffer.compare(buf, content), 0)

      await setTimeout(1000)

      const sinceAdd = new Date(meta.lastModified)
      sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)
      const invalidReader = await op.reader(filename, { ifModifiedSince: sinceAdd.toISOString() })
      await expect(invalidReader.read(Buffer.alloc(1))).rejects.toThrowError('ConditionNotMatch')

      await op.delete(filename)
    })

    test.runIf(readerWithIfUnmodifiedSince)('reader with if unmodified since', async () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      await op.write(filename, content)
      const meta = await op.stat(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)

      const invalidReader = await op.reader(filename, { ifUnmodifiedSince: sinceMinus.toISOString() })
      await expect(invalidReader.read(Buffer.alloc(1))).rejects.toThrowError('ConditionNotMatch')

      await setTimeout(1000)

      const sinceAdd = new Date(meta.lastModified)
      sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

      const reader = await op.reader(filename, { ifUnmodifiedSince: sinceAdd.toISOString() })
      const rs = reader.createReadStream()
      let chunks = []
      await pipeline(
        rs,
        new Writable({
          write(chunk, encoding, callback) {
            chunks.push(chunk)
            callback()
          },
        }),
      )

      await finished(rs)
      const bs2 = Buffer.concat(chunks)
      assert.equal(Buffer.compare(bs2, content), 0)

      await op.delete(filename)
    })
  })
}
