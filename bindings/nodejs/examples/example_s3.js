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

import { Operator } from 'opendal'

async function main() {
  const op = new Operator('s3', {
    root: '/test_opendal',
    bucket: 'your bucket name',
    region: 'your bucket region',
    endpoint: 'your endpoint',
    access_key_id: 'your access key id',
    secret_access_key: 'your secret access key',
  })

  await op.write('test', 'Hello, World!')
  const bs = await op.read('test')
  console.log(new TextDecoder().decode(bs))
  const meta = await op.stat('test')
  console.log(`contentLength: ${meta.contentLength}`)
}

main()
