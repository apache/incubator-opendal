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
import { test } from 'vitest'
import { generateBytes, generateFixedBytes } from '../utils.mjs'
import { Readable, Writable } from 'node:stream'
import { finished, pipeline } from 'node:stream/promises'

export function run(op) {
  describe('async tests', () => {
    test('async stat not exist files', async () => {
      const filename = `random_file_${randomUUID()}`

      try {
        await op.stat(filename)
      } catch (error) {
        assert.include(error.message, 'NotFound')
      }
    })

    test.runIf(op.capability().read && op.capability().write && op.capability().writeCanMulti)(
      'reader/writer stream pipeline',
      async () => {
        const filename = `random_file_${randomUUID()}`
        const buf = generateFixedBytes(5 * 1024 * 1024)
        const rs = Readable.from(buf, {
          highWaterMark: 5 * 1024 * 1024, // to buffer 5MB data to read
        })
        const w = await op.writer(filename)
        const ws = w.createWriteStream()
        await pipeline(rs, ws)

        await finished(ws)

        const t = await op.stat(filename)
        assert.equal(t.contentLength, buf.length)

        const content = await op.read(filename)
        assert.equal(Buffer.compare(content, buf), 0) // 0 means equal

        await op.delete(filename)
      },
    )

    test.runIf(op.capability().read && op.capability().write)('read stream', async () => {
      let c = generateFixedBytes(3 * 1024 * 1024)
      const filename = `random_file_${randomUUID()}`

      await op.write(filename, c)

      const r = await op.reader(filename)
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
      const buf = Buffer.concat(chunks)
      assert.equal(Buffer.compare(buf, c), 0)

      op.deleteSync(filename)
    })

    test.runIf(op.capability().read && op.capability().write)('write with behavior', async () => {
      let c = Buffer.from('hello world')
      const filename = `random_file_${randomUUID()}`

      const options = { chunk: 1024n * 1024n }
      if (op.capability().writeCanAppend) {
        options.append = true
      }
      if (op.capability().writeWithContentType) {
        options.contentType = 'text/plain'
      }
      if (op.capability().writeWithContentDisposition) {
        options.contentDisposition = 'attachment;filename=test.txt'
      }
      if (op.capability().writeWithCacheControl) {
        options.cacheControl = 'public, max-age=31536000, immutable'
      }
      await op.write(filename, c, options)

      if (op.capability().writeCanMulti) {
        const writer = await op.writer(filename, options)
        await writer.write(c)
        await writer.close()
      }

      if (op.capability().delete) {
        op.deleteSync(filename)
      }
    })

    test.runIf(op.capability().list)('test check', async () => {
      await op.check()
    })
  })
}
