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

//! The Getting Started example for the Rust core, embedded into the website
//! guide. It runs against the in-memory service with no credentials, so CI can
//! execute it directly. The region between the ANCHOR markers is what the docs
//! show — keep it copy-pasteable.

// ANCHOR: quickstart
use opendal::Operator;
use opendal::Result;
use opendal::services;

#[tokio::main]
async fn main() -> Result<()> {
    // Configure a service, then build an operator from it.
    let op = Operator::new(services::Memory::default())?;

    // The same verbs work on every service.
    op.write("hello.txt", "Hello, World!").await?;

    let bytes = op.read("hello.txt").await?;
    println!("read {} bytes", bytes.len());

    let meta = op.stat("hello.txt").await?;
    println!("size = {} bytes", meta.content_length());

    op.delete("hello.txt").await?;
    Ok(())
}
// ANCHOR_END: quickstart
