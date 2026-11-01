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

// ANCHOR: quickstart
import { Operator } from 'opendal'

async function main() {
  // Configure a service, then build an operator from it.
  const op = new Operator('memory')

  // The same verbs work on every service.
  await op.write('hello.txt', 'Hello, World!')

  const bs = await op.read('hello.txt') // returns a Buffer
  console.log(new TextDecoder().decode(bs))

  const meta = await op.stat('hello.txt')
  console.log(`size = ${meta.contentLength} bytes`)

  await op.delete('hello.txt')
}

main()
// ANCHOR_END: quickstart
