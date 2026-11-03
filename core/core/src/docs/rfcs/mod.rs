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

#![doc = include_str!("README.md")]

/// RFC example
#[doc = include_str!("0000_example.md")]
pub mod rfc_0000_example {}

/// Compact metadata and operation arguments
#[doc = include_str!("8194_compact_metadata_and_operation_args.md")]
pub mod rfc_8194_compact_metadata_and_operation_args {}

/// Write if not changed
#[doc = include_str!("8147_write_if_not_changed.md")]
pub mod rfc_8147_write_if_not_changed {}

/// Object native API
#[doc = include_str!("0041_object_native_api.md")]
pub mod rfc_0041_object_native_api {}

/// Error handle
#[doc = include_str!("0044_error_handle.md")]
pub mod rfc_0044_error_handle {}

/// Auto region
#[doc = include_str!("0057_auto_region.md")]
pub mod rfc_0057_auto_region {}

/// Object stream
#[doc = include_str!("0069_object_stream.md")]
pub mod rfc_0069_object_stream {}

/// Limited reader
#[doc = include_str!("0090_limited_reader.md")]
pub mod rfc_0090_limited_reader {}

/// Path normalization
#[doc = include_str!("0112_path_normalization.md")]
pub mod rfc_0112_path_normalization {}

/// Async streaming IO
#[doc = include_str!("0191_async_streaming_io.md")]
pub mod rfc_0191_async_streaming_io {}

/// Remove credential
#[doc = include_str!("0203_remove_credential.md")]
pub mod rfc_0203_remove_credential {}

/// Create dir
#[doc = include_str!("0221_create_dir.md")]
pub mod rfc_0221_create_dir {}

/// Retryable error
#[doc = include_str!("0247_retryable_error.md")]
pub mod rfc_0247_retryable_error {}

/// Object ID
#[doc = include_str!("0293_object_id.md")]
pub mod rfc_0293_object_id {}

/// Dir entry
#[doc = include_str!("0337_dir_entry.md")]
pub mod rfc_0337_dir_entry {}

/// Accessor capabilities
#[doc = include_str!("0409_accessor_capabilities.md")]
pub mod rfc_0409_accessor_capabilities {}

/// Presign
#[doc = include_str!("0413_presign.md")]
pub mod rfc_0413_presign {}

/// Command line interface
#[doc = include_str!("0423_command_line_interface.md")]
pub mod rfc_0423_command_line_interface {}

/// Init from iter
#[doc = include_str!("0429_init_from_iter.md")]
pub mod rfc_0429_init_from_iter {}

/// Multipart
#[doc = include_str!("0438_multipart.md")]
pub mod rfc_0438_multipart {}

/// Gateway
#[doc = include_str!("0443_gateway.md")]
pub mod rfc_0443_gateway {}

/// New builder
#[doc = include_str!("0501_new_builder.md")]
pub mod rfc_0501_new_builder {}

/// Write refactor
#[doc = include_str!("0554_write_refactor.md")]
pub mod rfc_0554_write_refactor {}

/// List metadata reuse
#[doc = include_str!("0561_list_metadata_reuse.md")]
pub mod rfc_0561_list_metadata_reuse {}

/// Blocking API
#[doc = include_str!("0599_blocking_api.md")]
pub mod rfc_0599_blocking_api {}

/// Redis service
#[doc = include_str!("0623_redis_service.md")]
pub mod rfc_0623_redis_service {}

/// Split capabilities
#[doc = include_str!("0627_split_capabilities.md")]
pub mod rfc_0627_split_capabilities {}

/// Path in accessor
#[doc = include_str!("0661_path_in_accessor.md")]
pub mod rfc_0661_path_in_accessor {}

/// Generic KV services
#[doc = include_str!("0793_generic_kv_services.md")]
pub mod rfc_0793_generic_kv_services {}

/// Object reader
#[doc = include_str!("0926_object_reader.md")]
pub mod rfc_0926_object_reader {}

/// Refactor error
#[doc = include_str!("0977_refactor_error.md")]
pub mod rfc_0977_refactor_error {}

/// Object handler
#[doc = include_str!("1085_object_handler.md")]
pub mod rfc_1085_object_handler {}

/// Object metadataer
#[doc = include_str!("1391_object_metadataer.md")]
pub mod rfc_1391_object_metadataer {}

/// Query based metadata
#[doc = include_str!("1398_query_based_metadata.md")]
pub mod rfc_1398_query_based_metadata {}

/// Object writer
#[doc = include_str!("1420_object_writer.md")]
pub mod rfc_1420_object_writer {}

/// Remove object concept
#[doc = include_str!("1477_remove_object_concept.md")]
pub mod rfc_1477_remove_object_concept {}

/// Operation extension
#[doc = include_str!("1735_operation_extension.md")]
pub mod rfc_1735_operation_extension {}

/// Writer sink API
#[doc = include_str!("2083_writer_sink_api.md")]
pub mod rfc_2083_writer_sink_api {}

/// Append API
#[doc = include_str!("2133_append_api.md")]
pub mod rfc_2133_append_api {}

/// Chain based operator API
#[doc = include_str!("2299_chain_based_operator_api.md")]
pub mod rfc_2299_chain_based_operator_api {}

/// Object versioning
#[doc = include_str!("2602_object_versioning.md")]
pub mod rfc_2602_object_versioning {}

/// Merge append into write
#[doc = include_str!("2758_merge_append_into_write.md")]
pub mod rfc_2758_merge_append_into_write {}

/// Lister API
#[doc = include_str!("2774_lister_api.md")]
pub mod rfc_2774_lister_api {}

/// List with metakey
#[doc = include_str!("2779_list_with_metakey.md")]
pub mod rfc_2779_list_with_metakey {}

/// Native capability
#[doc = include_str!("2852_native_capability.md")]
pub mod rfc_2852_native_capability {}

/// Remove write copy from
#[doc = include_str!("3017_remove_write_copy_from.md")]
pub mod rfc_3017_remove_write_copy_from {}

/// Config
#[doc = include_str!("3197_config.md")]
pub mod rfc_3197_config {}

/// Align list API
#[doc = include_str!("3232_align_list_api.md")]
pub mod rfc_3232_align_list_api {}

/// List prefix
#[doc = include_str!("3243_list_prefix.md")]
pub mod rfc_3243_list_prefix {}

/// Lazy reader
#[doc = include_str!("3356_lazy_reader.md")]
pub mod rfc_3356_lazy_reader {}

/// List recursive
#[doc = include_str!("3526_list_recursive.md")]
pub mod rfc_3526_list_recursive {}

/// Concurrent stat in list
#[doc = include_str!("3574_concurrent_stat_in_list.md")]
pub mod rfc_3574_concurrent_stat_in_list {}

/// Buffered Reader
#[doc = include_str!("3734_buffered_reader.md")]
pub mod rfc_3734_buffered_reader {}

/// Concurrent Writer
#[doc = include_str!("3898_concurrent_writer.md")]
pub mod rfc_3898_concurrent_writer {}

/// Deleter API
#[doc = include_str!("3911_deleter_api.md")]
pub mod rfc_3911_deleter_api {}

/// Range Based Read API
#[doc = include_str!("4382_range_based_read.md")]
pub mod rfc_4382_range_based_read {}

/// Executor API
#[doc = include_str!("4638_executor.md")]
pub mod rfc_4638_executor {}

/// Remove metakey
#[doc = include_str!("5314_remove_metakey.md")]
pub mod rfc_5314_remove_metakey {}

/// Operator from uri
#[doc = include_str!("5444_operator_from_uri.md")]
pub mod rfc_5444_operator_from_uri {}

/// Context
#[doc = include_str!("5479_context.md")]
pub mod rfc_5479_context {}

/// Conditional Reader
#[doc = include_str!("5485_conditional_reader.md")]
pub mod rfc_5485_conditional_reader {}

/// List With Deleted
#[doc = include_str!("5495_list_with_deleted.md")]
pub mod rfc_5495_list_with_deleted {}

/// Write Returns Metadata
#[doc = include_str!("5556_write_returns_metadata.md")]
pub mod rfc_5556_write_returns_metadata {}

/// Read Returns Metadata
#[doc = include_str!("5871_read_returns_metadata.md")]
pub mod rfc_5871_read_returns_metadata {}

/// Remove Native Blocking
#[doc = include_str!("6189_remove_native_blocking.md")]
pub mod rfc_6189_remove_native_blocking {}

/// Glob support
#[doc = include_str!("6209_glob_support.md")]
pub mod rfc_6209_glob_support {}

/// Options API
#[doc = include_str!("6213_options_api.md")]
pub mod rfc_6213_options_api {}

/// Cache Layer
#[doc = include_str!("6297_cache_layer.md")]
pub mod rfc_6297_cache_layer {}

/// Simulate Layer
#[doc = include_str!("6678_simulate_layer.md")]
pub mod rfc_6678_simulate_layer {}

/// Capability Override Layer
#[doc = include_str!("6707_capability_override_layer.md")]
pub mod rfc_6707_capability_override_layer {}

/// Checksum
#[doc = include_str!("6817_checksum.md")]
pub mod rfc_6817_checksum {}

/// Core
#[doc = include_str!("6828_core.md")]
pub mod rfc_6828_core {}

/// Route Layer
#[doc = include_str!("7130_route_layer.md")]
pub mod rfc_7130_route_layer {}

/// Copier API
#[doc = include_str!("7519_copier_api.md")]
pub mod rfc_7519_copier_api {}

/// Move Read Range To Reader
#[doc = include_str!("7660_move_read_range_to_reader.md")]
pub mod rfc_7660_move_read_range_to_reader {}

/// Capability API
#[doc = include_str!("7700_capability_api.md")]
pub mod rfc_7700_capability_api {}

/// Operator Composition
#[doc = include_str!("7740_operator_composition.md")]
pub mod rfc_7740_operator_composition {}

/// Simplify Stateful Operation Factory
#[doc = include_str!("7744_simplify_stateful_operation_factory.md")]
pub mod rfc_7744_simplify_stateful_operation_factory {}

/// HTTP Transporter
#[doc = include_str!("7749_http_transporter.md")]
pub mod rfc_7749_http_transporter {}

/// Rename If Not Exists
#[doc = include_str!("7818_rename_if_not_exists.md")]
pub mod rfc_7818_rename_if_not_exists {}

/// Limited Read
#[doc = include_str!("7945_limited_read.md")]
pub mod rfc_7945_limited_read {}

/// S3 Provider Presets
#[doc = include_str!("7948_s3_provider_presets.md")]
pub mod rfc_7948_s3_provider_presets {}

/// Writer Copy From
#[doc = include_str!("8109_writer_copy_from.md")]
pub mod rfc_8109_writer_copy_from {}

/// Conflict Error Kind
#[doc = include_str!("8145_conflict_error_kind.md")]
pub mod rfc_8145_conflict_error_kind {}
