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

const path = require('path')

export function generateBytes() {
    const size = Math.floor(Math.random() * 1024) + 1
    const content = []

    for (let i = 0; i < size; i++) {
        content.push(Math.floor(Math.random() * 256))
    }

    return Buffer.from(content)
}

export function loadTestSchemeFromEnv() {
    require('dotenv').config({ path: path.resolve(__dirname, '../../../.env'), debug: true })
    return process.env.OPENDAL_TEST
}

export function loadConfigFromEnv(scheme) {
    if (!scheme) return {}

    const prefix = `opendal_${scheme}_`

    return Object.fromEntries(
        Object.entries(process.env)
            .map(([key, value]) => [key.toLowerCase(), value])
            .filter(([key]) => key.startsWith(prefix))
            .map(([key, value]) => [key.replace(prefix, ''), value]),
    )
}
