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

const { Operator } = require('../index.cjs')
const { S3Client, PutObjectCommand, GetObjectCommand } = require('@aws-sdk/client-s3')
const { suite, add, cycle, complete } = require('benny')
const crypto = require('node:crypto')

const endpoint = process.env.AWS_S3_ENDPOINT
const region = process.env.AWS_S3_REGION
const accessKeyId = process.env.AWS_ACCESS_KEY_ID
const secretAccessKey = process.env.AWS_SECRET_ACCESS_KEY
const bucket = process.env.AWS_BUCKET

const opendalClient = new Operator('s3', {
  root: '/',
  bucket,
  endpoint,
  region,
})

const s3Client = new S3Client({
  endpoint,
  region,
  credentials: {
    accessKeyId,
    secretAccessKey,
  },
})

const testCases = [
  { name: '4kb', content: Buffer.alloc(4 * 1024, 'opendal', 'utf8') },
  { name: '256kb', content: Buffer.alloc(256 * 1024, 'opendal', 'utf8') },
  { name: '4mb', content: Buffer.alloc(4 * 1024 * 1024, 'opendal', 'utf8') },
  { name: '16mb', content: Buffer.alloc(16 * 1024 * 1024, 'opendal', 'utf8') },
]

async function benchRead() {
  const uuid = crypto.randomUUID()
  await testCases
    .map((v) => async () => {
      const filename = `${uuid}_${v.name}_read_bench.txt`
      await opendalClient.write(filename, v.content)

      return suite(
        `read (${v.name})`,
        add(`opendal read (${v.name})`, async () => {
          await opendalClient.read(filename).then((v) => v.toString('utf-8'))
        }),
        add(`s3 read (${v.name})`, async () => {
          const command = new GetObjectCommand({
            Key: filename,
            Bucket: bucket,
          })
          await s3Client.send(command).then((v) => v.Body.transformToString('utf-8'))
        }),
        cycle(),
        complete(),
      )
    })
    .reduce((p, v) => p.then(() => v()), Promise.resolve())
}

async function benchWrite() {
  const uuid = crypto.randomUUID()
  await testCases
    .map(
      (v) => () =>
        suite(
          `write (${v.name})`,
          add(`opendal write (${v.name})`, async () => {
            let count = 0
            return async () => opendalClient.write(`${uuid}_${count++}_${v.name}_opendal.txt`, v.content)
          }),
          add(`s3 write (${v.name})`, async () => {
            let count = 0

            return async () => {
              const command = new PutObjectCommand({
                Bucket: bucket,
                Key: `${uuid}_${count++}_${v.name}_s3.txt`,
                Body: v.content,
              })
              await s3Client.send(command)
            }
          }),
          cycle(),
          complete(),
        ),
    )
    .reduce((p, v) => p.then(() => v()), Promise.resolve())
}

async function bench() {
  await benchRead()
  await benchWrite()
}

bench()
