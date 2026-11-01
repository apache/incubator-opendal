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

// The four-step mental model — service, operator, layer, operation — written
// out in bindings whose public API exposes the operator and layers directly,
// so the concepts page never drifts from the libraries it describes. Bindings
// with a different surface (for example Dart) are covered by the page prose.
export const conceptSamples = [
  {
    id: "rust",
    label: "Rust",
    language: "rust",
    code: `use opendal::layers::RetryLayer;
use opendal::services::S3;
use opendal::Operator;

// 1. Describe the service.
let builder = S3::default().bucket("data");

// 2. Build the operator and 3. wrap it with layers.
let op = Operator::new(builder)?
    .layer(RetryLayer::new())
    .finish();

// 4. Call operations.
op.write("hello.txt", "Hello, World!").await?;
let bytes = op.read("hello.txt").await?;`,
  },
  {
    id: "python",
    label: "Python",
    language: "python",
    code: `import opendal

# 1. Describe the service and 2. build the operator.
op = opendal.Operator("s3", bucket="data")

# 3. Wrap it with layers.
op = op.layer(opendal.layers.RetryLayer())

# 4. Call operations.
op.write("hello.txt", b"Hello, World!")
data = op.read("hello.txt")`,
  },
  {
    id: "java",
    label: "Java",
    language: "java",
    code: `import org.apache.opendal.AsyncOperator;
import org.apache.opendal.layer.RetryLayer;
import java.util.Map;

// 1. Describe the service and 2. build the operator,
//    3. wrapped with layers.
var conf = Map.of("bucket", "data");
try (var op = AsyncOperator.of("s3", conf)
        .layer(RetryLayer.builder().build())) {
    // 4. Call operations.
    op.write("hello.txt", "Hello, World!").join();
    byte[] data = op.read("hello.txt").join();
}`,
  },
  {
    id: "node",
    label: "Node.js",
    language: "javascript",
    code: `import { Operator, RetryLayer } from "opendal";

// 1. Describe the service and 2. build the operator.
let op = new Operator("s3", { bucket: "data" });

// 3. Wrap it with layers.
const retry = new RetryLayer();
retry.maxTimes = 3;
op = op.layer(retry.build());

// 4. Call operations.
await op.write("hello.txt", "Hello, World!");
const data = await op.read("hello.txt");`,
  },
];
