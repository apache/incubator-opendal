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

mod mode;
pub use mode::EntryMode;

mod buffer;
pub use buffer::Buffer;
pub use buffer::BufferCursor;

pub(crate) mod compact;

mod http_transport;
pub use http_transport::HttpBody;
pub use http_transport::HttpRedirect;
pub use http_transport::HttpTransport;
pub use http_transport::HttpTransporter;
pub use http_transport::HttpUri;

mod entry;
pub use entry::Entry;

pub(crate) mod metadata;
pub use metadata::Metadata;
pub use metadata::MetadataBuilder;
pub use metadata::UserMetadata;
pub use metadata::UserMetadataIntoIter;

mod read;
pub use read::*;

mod write;
pub use write::*;

mod list;
pub use list::Lister;

mod copy;
pub use copy::Copier;

mod compose;
pub use compose::*;

mod delete;
pub use delete::*;

mod execute;
pub use execute::*;

mod operator;
pub use operator::IntoOperatorUri;
pub use operator::Operator;
pub use operator::OperatorFactory;
pub use operator::OperatorInfo;
pub use operator::OperatorRegistry;
pub use operator::OperatorUri;
pub use operator::operator_futures;

mod builder;
pub use builder::Builder;
pub use builder::Configurator;

mod error;
pub use error::Error;
pub use error::ErrorKind;
pub use error::Result;

mod capability;
pub use capability::Capability;

mod bytes_range;
pub use bytes_range::BytesRange;

pub mod options;

mod context;
pub use context::OperationContext;
pub(crate) use context::*;
