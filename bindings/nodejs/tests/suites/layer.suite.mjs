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

import { test, assert } from 'vitest'

import { RetryLayer, ConcurrentLimitLayer, TimeoutLayer, LoggingLayer, ThrottleLayer } from '../../index.mjs'

/**
 * @param {import("../../index").Operator} op
 */
export function run(op) {
  test('test operator with retry layer', () => {
    const retryLayer = new RetryLayer()
    retryLayer.maxTimes = 3
    retryLayer.jitter = true

    const layerOp = op.layer(retryLayer.build())
    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with concurrent limit layer', () => {
    const concurrentLimitLayer = new ConcurrentLimitLayer(1024)
    const layerOp = op.layer(concurrentLimitLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with concurrent limit layer and http permits', () => {
    const concurrentLimitLayer = new ConcurrentLimitLayer(1024)
    concurrentLimitLayer.httpPermits = 512

    const layerOp = op.layer(concurrentLimitLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with timeout layer', () => {
    const timeoutLayer = new TimeoutLayer()
    timeoutLayer.timeout = 10000
    timeoutLayer.ioTimeout = 5000

    const layerOp = op.layer(timeoutLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with timeout layer using default values', () => {
    const timeoutLayer = new TimeoutLayer()

    const layerOp = op.layer(timeoutLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with logging layer', () => {
    const loggingLayer = new LoggingLayer()

    const layerOp = op.layer(loggingLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with throttle layer', () => {
    const throttleLayer = new ThrottleLayer(10 * 1024, 1024 * 1024)

    const layerOp = op.layer(throttleLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })

  test('test operator with multiple layers', () => {
    const loggingLayer = new LoggingLayer()
    const timeoutLayer = new TimeoutLayer()
    timeoutLayer.timeout = 30000
    timeoutLayer.ioTimeout = 10000
    const retryLayer = new RetryLayer()
    retryLayer.maxTimes = 3
    const throttleLayer = new ThrottleLayer(100 * 1024, 10 * 1024 * 1024)

    const layerOp = op
      .layer(loggingLayer.build())
      .layer(timeoutLayer.build())
      .layer(retryLayer.build())
      .layer(throttleLayer.build())

    assert.ok(layerOp)
    assert.ok(layerOp.capability())
  })
}
