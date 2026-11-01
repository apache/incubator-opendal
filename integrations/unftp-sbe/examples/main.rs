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

use std::error::Error;

use opendal::Operator;
use unftp_sbe_opendal::OpendalStorage;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Create any service desired
    let service = opendal::services::Memory::default();

    // Init an operator with the service created
    let op = Operator::new(service)?;

    // Wrap the operator with `OpendalStorage`
    let backend = OpendalStorage::new(op);

    // Build the actual unftp server
    let server = libunftp::ServerBuilder::new(Box::new(move || backend.clone())).build()?;

    // Start the server
    server.listen("0.0.0.0:0").await?;

    Ok(())
}
