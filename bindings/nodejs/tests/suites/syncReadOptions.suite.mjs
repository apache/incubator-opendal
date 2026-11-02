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

import { generateFixedBytes, generateBytes } from '../utils.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  const capability = op.capability()
  const readerWithIfMatch = capability.readWithIfMatch && capability.statWithIfMatch
  const readerWithIfNoneMatch = capability.readWithIfNoneMatch && capability.statWithIfNoneMatch
  const readerWithIfModifiedSince = capability.readWithIfModifiedSince && capability.statWithIfModifiedSince
  const readerWithIfUnmodifiedSince = capability.readWithIfUnmodifiedSince && capability.statWithIfUnmodifiedSince

  describe.runIf(capability.read && capability.write)('sync read options', () => {
    test('read with range', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)
      const offset = Math.floor(Math.random() * (size - 1))
      const maxLen = size - offset
      const length = Math.floor(Math.random() * (maxLen - 1)) + 1

      op.writeSync(filename, content)

      const bs = op.readSync(filename, {
        offset: BigInt(offset),
        size: BigInt(length),
      })
      expect(bs.length).toBe(length)
      assert.equal(Buffer.compare(bs, content.subarray(offset, offset + length)), 0)

      op.deleteSync(filename)
    })

    test.runIf(capability.readWithIfMatch)('read with if match', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const invalidOptions = {
        ifMatch: '"invalid_etag"',
      }

      expect(() => op.readSync(filename, invalidOptions)).toThrowError('ConditionNotMatch')

      const bs = op.readSync(filename, {
        ifMatch: meta.etag,
      })

      assert.equal(Buffer.compare(bs, content), 0)

      op.deleteSync(filename)
    })

    test.runIf(capability.readWithIfNoneMatch)('read with if none match', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const invalidOptions = {
        ifNoneMatch: meta.etag,
      }

      expect(() => op.readSync(filename, invalidOptions)).toThrowError('ConditionNotMatch')

      const bs = op.readSync(filename, {
        ifNoneMatch: '"invalid_etag"',
      })

      assert.equal(Buffer.compare(bs, content), 0)

      op.deleteSync(filename)
    })

    test.runIf(capability.readWithIfModifiedSince)('read with if modified since', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)
      const bs = op.readSync(filename, { ifModifiedSince: sinceMinus.toISOString() })
      assert.equal(Buffer.compare(bs, content), 0)

      setTimeout(() => {
        const sinceAdd = new Date(meta.lastModified)
        sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

        expect(() => op.readSync(filename, { ifModifiedSince: sinceAdd.toISOString() })).toThrowError(
          'ConditionNotMatch',
        )
        op.deleteSync(filename)
      }, 1000)
    })

    test.runIf(capability.readWithIfUnmodifiedSince)('read with if unmodified since', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 3600)
      expect(() => op.readSync(filename, { ifUnmodifiedSince: sinceMinus.toISOString() })).toThrowError(
        'ConditionNotMatch',
      )

      setTimeout(() => {
        const sinceAdd = new Date(meta.lastModified)
        sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

        const bs = op.readSync(filename, { ifUnmodifiedSince: sinceAdd.toISOString() })
        assert.equal(Buffer.compare(bs, content), 0)

        op.deleteSync(filename)
      }, 1000)
    })

    test.runIf(capability.readWithVersion)('read with version', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      const meta = op.statSync(filename)
      const data = op.readSync(filename, { version: meta.version })
      assert.equal(Buffer.compare(data, content), 0)

      op.writeSync(filename, Buffer.from('1'))
      // After writing new data, we can still read the first version data
      const secondData = op.readSync(filename, { version: meta.version })
      assert.equal(Buffer.compare(secondData, content), 0)

      op.deleteSync(filename)
    })

    test.runIf(capability.readWithVersion)('read with not existing version', () => {
      const filename = `random_file_${randomUUID()}`
      const content = generateBytes()

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const filename2 = `random_file_${randomUUID()}`
      const content2 = generateBytes()
      op.writeSync(filename2, content2)

      expect(() => op.readSync(filename2, { version: meta.version })).toThrowError('NotFound')

      op.deleteSync(filename)
      op.deleteSync(filename2)
    })
  })

  describe.runIf(capability.read && capability.write)('sync reader options', () => {
    test.runIf(readerWithIfMatch)('reader with if match', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const invalidOptions = {
        ifMatch: '"invalid_etag"',
      }

      const invalidReader = op.readerSync(filename, invalidOptions)
      expect(() => invalidReader.read(Buffer.alloc(1))).toThrowError('ConditionNotMatch')

      const r = op.readerSync(filename, { ifMatch: meta.etag })
      const rs = r.createReadStream()

      let chunks = []
      rs.on('data', (chunk) => {
        chunks.push(chunk)
      })

      rs.on('end', () => {
        const buf = Buffer.concat(chunks)
        assert.equal(Buffer.compare(buf, content), 0)

        op.deleteSync(filename)
      })
    })

    test.runIf(readerWithIfNoneMatch)('reader with if none match', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const invalidReader = op.readerSync(filename, { ifNoneMatch: meta.etag })
      expect(() => invalidReader.read(Buffer.alloc(1))).toThrowError('ConditionNotMatch')

      const r = op.readerSync(filename, { ifNoneMatch: '"invalid_etag"' })
      const rs = r.createReadStream()

      let chunks = []
      rs.on('data', (chunk) => {
        chunks.push(chunk)
      })

      rs.on('end', () => {
        const buf = Buffer.concat(chunks)
        assert.equal(Buffer.compare(buf, content), 0)

        op.deleteSync(filename)
      })
    })

    test.runIf(readerWithIfModifiedSince)('reader with if modified since', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)
      const reader = op.readerSync(filename, { ifModifiedSince: sinceMinus.toISOString() })
      const rs = reader.createReadStream()

      let chunks = []
      rs.on('data', (chunk) => {
        chunks.push(chunk)
      })

      rs.on('end', () => {
        const buf = Buffer.concat(chunks)
        assert.equal(Buffer.compare(buf, content), 0)
      })

      setTimeout(() => {
        const sinceAdd = new Date(meta.lastModified)
        sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)
        const invalidReader = op.readerSync(filename, { ifModifiedSince: sinceAdd.toISOString() })
        expect(() => invalidReader.read(Buffer.alloc(1))).toThrowError('ConditionNotMatch')

        op.deleteSync(filename)
      }, 1000)
    })

    test.runIf(readerWithIfUnmodifiedSince)('reader with if unmodified since', () => {
      const size = 3 * 1024 * 1024
      const filename = `random_file_${randomUUID()}`
      const content = generateFixedBytes(size)

      op.writeSync(filename, content)
      const meta = op.statSync(filename)

      const sinceMinus = new Date(meta.lastModified)
      sinceMinus.setSeconds(sinceMinus.getSeconds() - 1)

      const invalidReader = op.readerSync(filename, { ifUnmodifiedSince: sinceMinus.toISOString() })
      expect(() => invalidReader.read(Buffer.alloc(1))).toThrowError('ConditionNotMatch')

      setTimeout(() => {
        const sinceAdd = new Date(meta.lastModified)
        sinceAdd.setSeconds(sinceAdd.getSeconds() + 1)

        const reader = op.readerSync(filename, { ifUnmodifiedSince: sinceAdd.toISOString() })
        const rs = reader.createReadStream()

        let chunks = []
        rs.on('data', (chunk) => {
          chunks.push(chunk)
        })

        rs.on('end', () => {
          const buf = Buffer.concat(chunks)
          assert.equal(Buffer.compare(buf, content), 0)
          op.deleteSync(filename)
        })
      }, 1000)
    })
  })
}
