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

import crypto from 'node:crypto'

// Generate random bytes between 1 and 1024 KB
export function generateBytes() {
  return crypto.randomBytes((Math.floor(Math.random() * 1024) + 1) * 1024)
}

// Generate random bytes with given size
export function generateFixedBytes(size) {
  return crypto.randomBytes(size)
}

export function loadTestSchemeFromEnv() {
  const scheme = process.env.OPENDAL_TEST
  // Normalize scheme by replacing underscores with hyphens
  return scheme ? scheme.replace(/_/g, '-') : scheme
}

export function checkRandomRootEnabled() {
  return process.env.OPENDAL_DISABLE_RANDOM_ROOT !== 'true'
}

export function generateRandomRoot(baseRoot) {
  return `${baseRoot}/opendal_${crypto.randomUUID()}`
}

export function loadConfigFromEnv(scheme) {
  if (!scheme) return {}

  // Convert scheme to underscore format for environment variable prefix
  const schemeKey = scheme.replace(/-/g, '_')
  const prefix = `opendal_${schemeKey}_`

  return Object.fromEntries(
    Object.entries(process.env)
      .map(([key, value]) => [key.toLowerCase(), value])
      .filter(([key]) => key.startsWith(prefix))
      .map(([key, value]) => [key.replace(prefix, ''), value]),
  )
}
