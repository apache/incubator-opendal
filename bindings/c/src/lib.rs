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

#![warn(missing_docs)]
// This crate is the C binding for the OpenDAL project.
// So it's type node can't meet camel case.
#![allow(non_camel_case_types)]
// This crate is the C binding for the OpenDAL project.
// Nearly all the functions exposed to C FFI are unsafe.
#![allow(clippy::missing_safety_doc)]
// Allow unknown cfgs like `cbindgen` used in cfg_attr for header generation.
#![allow(unexpected_cfgs)]

//! The Apache OpenDAL C binding.
//!
//! The OpenDAL C binding allows users to utilize the OpenDAL's amazing storage accessing capability
//! in the C programming language.
//!
//! For examples, you may see the examples subdirectory

mod error;
pub use error::opendal_code;
pub use error::opendal_error;

mod lister;
pub use lister::opendal_lister;

mod layer;
pub use layer::opendal_operator_layers;

mod metadata;
pub use metadata::opendal_metadata;
pub use metadata::opendal_metadata_user_metadata;
pub use metadata::opendal_metadata_user_metadata_pair;

mod operator;
pub use operator::opendal_operator;

mod operator_info;

mod presign;
pub use presign::opendal_http_header_pair;
pub use presign::opendal_presigned_request;
pub use presign::opendal_result_presign;

mod result;
pub use result::opendal_result_copier_next;
pub use result::opendal_result_exists;
pub use result::opendal_result_is_exist;
pub use result::opendal_result_list;
pub use result::opendal_result_lister_next;
pub use result::opendal_result_operator_copier;
pub use result::opendal_result_operator_new;
pub use result::opendal_result_operator_reader;
pub use result::opendal_result_operator_writer;
pub use result::opendal_result_read;
pub use result::opendal_result_reader_read;
pub use result::opendal_result_stat;
pub use result::opendal_result_write;
pub use result::opendal_result_writer_write;

mod types;
pub use types::opendal_bytes;
pub use types::opendal_copy_options;
pub use types::opendal_delete_options;
pub use types::opendal_list_options;
pub use types::opendal_operator_options;
pub use types::opendal_read_options;
pub use types::opendal_reader_options;
pub use types::opendal_stat_options;
pub use types::opendal_write_options;
pub use types::opendal_write_user_metadata_pair;

mod entry;
pub use entry::opendal_entry;

mod reader;
pub use reader::opendal_reader;

mod writer;
pub use writer::opendal_writer;

mod copier;
pub use copier::opendal_copier;
