(*
   Licensed to the Apache Software Foundation (ASF) under one
   or more contributor license agreements.  See the NOTICE file
   distributed with this work for additional information
   regarding copyright ownership.  The ASF licenses this file
   to you under the Apache License, Version 2.0 (the
   "License")you may not use this file except in compliance
   with the License.  You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing,
   software distributed under the License is distributed on an
   "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
   KIND, either express or implied.  See the License for the
   specific language governing permissions and limitations
   under the License.
*)

(** Core operator functions *)
let new_operator = Opendal_core.Operator.operator

let list = Opendal_core.Operator.blocking_list
let lister = Opendal_core.Operator.blocking_lister
let stat = Opendal_core.Operator.blocking_stat
let is_exist = Opendal_core.Operator.blocking_is_exist
let create_dir = Opendal_core.Operator.blocking_create_dir
let read = Opendal_core.Operator.blocking_read
let reader = Opendal_core.Operator.blocking_reader
let write = Opendal_core.Operator.blocking_write
let writer = Opendal_core.Operator.blocking_writer
let copy = Opendal_core.Operator.blocking_copy
let rename = Opendal_core.Operator.blocking_rename
let delete = Opendal_core.Operator.blocking_delete
let remove = Opendal_core.Operator.blocking_remove
let remove_all = Opendal_core.Operator.blocking_remove_all
let check = Opendal_core.Operator.blocking_check
let info = Opendal_core.Operator.operator_info

(** Reader module for reading data *)
module Reader = struct
  let pread = Opendal_core.Operator.reader_pread
end

(** Writer module for writing data *)
module Writer = struct
  let write = Opendal_core.Operator.writer_write
  let close = Opendal_core.Operator.writer_close
end

(** Lister module for streaming directory listings *)
module Lister = struct
  let next = Opendal_core.Operator.lister_next
end

(** Metadata module for file/directory information *)
module Metadata = struct
  let is_file = Opendal_core.Operator.metadata_is_file
  let is_dir = Opendal_core.Operator.metadata_is_dir
  let content_length = Opendal_core.Operator.metadata_content_length
  let content_md5 = Opendal_core.Operator.metadata_content_md5
  let content_type = Opendal_core.Operator.metadata_content_type
  let content_disposition = Opendal_core.Operator.metadata_content_disposition
  let etag = Opendal_core.Operator.metadata_etag
  let last_modified = Opendal_core.Operator.metadata_last_modified
end

(** Entry module for directory entry information *)
module Entry = struct
  let path = Opendal_core.Operator.entry_path
  let name = Opendal_core.Operator.entry_name
  let metadata = Opendal_core.Operator.entry_metadata
end

(** OperatorInfo module for operator information *)
module OperatorInfo = struct
  let name = Opendal_core.Operator.operator_info_name
  let scheme = Opendal_core.Operator.operator_info_scheme
  let root = Opendal_core.Operator.operator_info_root
  let capability = Opendal_core.Operator.operator_info_capability
end

(** Capability module for checking supported operations *)
module Capability = struct
  let stat = Opendal_core.Operator.capability_stat
  let read = Opendal_core.Operator.capability_read
  let write = Opendal_core.Operator.capability_write
  let create_dir = Opendal_core.Operator.capability_create_dir
  let delete = Opendal_core.Operator.capability_delete
  let copy = Opendal_core.Operator.capability_copy
  let rename = Opendal_core.Operator.capability_rename
  let list = Opendal_core.Operator.capability_list
  let list_with_limit = Opendal_core.Operator.capability_list_with_limit

  let list_with_start_after =
    Opendal_core.Operator.capability_list_with_start_after

  let list_with_recursive = Opendal_core.Operator.capability_list_with_recursive
  let presign = Opendal_core.Operator.capability_presign
  let presign_read = Opendal_core.Operator.capability_presign_read
  let presign_stat = Opendal_core.Operator.capability_presign_stat
  let presign_write = Opendal_core.Operator.capability_presign_write
  let shared = Opendal_core.Operator.capability_shared
end
