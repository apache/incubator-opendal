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

import { describe } from 'vitest'
import { Operator, layers } from '../../index.mjs'
import { checkRandomRootEnabled, generateRandomRoot, loadConfigFromEnv } from '../utils.mjs'

import { run as AsyncIOTestRun } from './async.suite.mjs'
import { run as ServicesTestRun } from './services.suite.mjs'
import { run as SyncIOTestRun } from './sync.suite.mjs'
import { run as LayerTestRun } from './layer.suite.mjs'
import { run as AsyncStatOptionsTestRun } from './asyncStatOptions.suite.mjs'
import { run as SyncStatOptionsTestRun } from './syncStatOptions.suite.mjs'
import { run as AsyncReadOptionsTestRun } from './asyncReadOptions.suite.mjs'
import { run as SyncReadOptionsTestRun } from './syncReadOptions.suite.mjs'
import { run as AsyncListOptionsTestRun } from './asyncListOptions.suite.mjs'
import { run as SyncListOptionsTestRun } from './syncListOptions.suite.mjs'
import { run as AsyncListerTestRun } from './asyncLister.suite.mjs'
import { run as SyncListerTestRun } from './syncLister.suite.mjs'
import { run as AsyncDeleteOptionsTestRun } from './asyncDeleteOptions.suite.mjs'
import { run as SyncDeleteOptionsTestRun } from './syncDeleteOptions.suite.mjs'
import { run as AsyncWriteOptionsTestRun } from './asyncWriteOptions.suite.mjs'
import { run as SyncWriteOptionsTestRun } from './syncWriteOptions.suite.mjs'

export function runner(testName, scheme) {
  if (!scheme) {
    console.warn('The scheme is empty. Test will be skipped.')
    return
  }

  const config = loadConfigFromEnv(scheme)

  if (checkRandomRootEnabled()) {
    if (config.root) {
      config.root = generateRandomRoot(config.root)
    } else {
      console.warn("The root is not set. Won't generate random root.")
    }
  }

  let operator = scheme ? new Operator(scheme, config) : null

  let retryLayer = new layers.RetryLayer()
  retryLayer.jitter = true
  retryLayer.maxTimes = 4

  operator = operator.layer(retryLayer.build())
  if (process.env.OPENDAL_TEST_CAPABILITY_OVERRIDES) {
    operator = operator.layer(new layers.CapabilityOverrideLayer(process.env.OPENDAL_TEST_CAPABILITY_OVERRIDES).build())
  }

  describe.skipIf(!operator)(testName, () => {
    AsyncIOTestRun(operator)
    ServicesTestRun(operator)
    SyncIOTestRun(operator)
    LayerTestRun(operator)
    AsyncStatOptionsTestRun(operator)
    SyncStatOptionsTestRun(operator)
    AsyncReadOptionsTestRun(operator)
    SyncReadOptionsTestRun(operator)
    AsyncListOptionsTestRun(operator)
    SyncListOptionsTestRun(operator)
    AsyncListerTestRun(operator)
    SyncListerTestRun(operator)
    AsyncDeleteOptionsTestRun(operator)
    SyncDeleteOptionsTestRun(operator)
    AsyncWriteOptionsTestRun(operator)
    SyncWriteOptionsTestRun(operator)
  })
}
