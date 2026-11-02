// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use opendal::Operator;
use opendal::Result;

async fn example(op: Operator) -> Result<()> {
    // Write data to S3.
    op.write("test.txt", "Hello, World!").await?;
    println!("write succeeded");

    // Read data from s3.
    let bs = op.read("test.txt").await?;

    println!("read: {}", String::from_utf8(bs.to_vec()).unwrap());

    // Fetch metadata of s3.
    let meta = op.stat("test.txt").await?;
    println!("stat: {meta:?}");

    // Delete data from s3.
    op.delete("test.txt").await?;
    println!("delete succeeded");

    Ok(())
}

// --- The following data is not part of this example ---

#[tokio::main]
async fn main() -> Result<()> {
    use opendal::tests::init_test_service;
    let op = init_test_service()?.expect("OPENDAL_TEST must be set");

    println!("service {:?} has been initialized", op.info());
    example(op).await
}
