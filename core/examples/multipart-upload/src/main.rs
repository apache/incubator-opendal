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
    let mut w = op.writer("test.txt").await?;

    // Write a 10MiB chunk.
    w.write(vec![0; 10 * 1024 * 1024]).await?;
    println!("vec![0; 10 * 1024 * 1024] uploaded");
    // Write another 10MiB chunk.
    w.write(vec![1; 10 * 1024 * 1024]).await?;
    println!("vec![1; 10 * 1024 * 1024] uploaded");
    // Finish the upload.
    w.close().await?;
    println!("upload finished");

    let bs = op.read("test.txt").await?;
    println!("read: {} bytes", bs.len());
    assert_eq!(
        bs.to_vec(),
        vec![0; 10 * 1024 * 1024]
            .into_iter()
            .chain(vec![1; 10 * 1024 * 1024])
            .collect::<Vec<_>>()
    );

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
