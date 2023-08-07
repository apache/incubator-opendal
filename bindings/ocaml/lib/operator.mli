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

val new_operator :
  string ->
  (string * string) list ->
  (Opendal_core.Operator.operator, string) result
(** [new_operator scheme config_map] Create a new block operator from given scheme and config_map.
    
    @param scheme Supported services, for details, refer to https://opendal.apache.org/docs/category/services/
    @param config_map Configuration information required by the target service
    @return The block operator
*)

val is_exist : Opendal_core.Operator.operator -> string -> (bool, string) result
(** [is_exist operator path] Check if this path exists or not.
    
    @param operator The operator
    @param path want to check
    @return is exists
*)

val create_dir :
  Opendal_core.Operator.operator -> string -> (bool, string) result
(** [create_dir operator path] Create a dir at given path.
    
    # Notes
    
    To indicate that a path is a directory, it is compulsory to include
    a trailing / in the path. Failure to do so may result in
    `NotADirectory` error being returned by OpenDAL.
    
    # Behavior
    
    - Create on existing dir will succeed.
    - Create dir is always recursive, works like `mkdir -p`
    @param operator The operator
    @param path want to create dir
*)

val read :
  Opendal_core.Operator.operator -> string -> (char array, string) result
(** [read operator path] Read the whole path into a bytes.
    
    @param operator The operator
    @param path want to read
    @return data of path
*)

val write :
  Opendal_core.Operator.operator -> string -> bytes -> (unit, string) result
(** [write operator path data] Write bytes into given path.
    - Write will make sure all bytes has been written, or an error will be returned.
    @param operator The operator
    @param path want to write
    @param data want to write
*)

val copy :
  Opendal_core.Operator.operator -> string -> string -> (unit, string) result
(** [copy operator from to] Copy a file from [from] to [to].
    - [from] and [to] must be a file.
    - [to] will be overwritten if it exists.
    - If [from] and [to] are the same, nothing will happen.
    - copy is idempotent. For same [from] and [to] input, the result will be the same.
    @param operator The operator
    @param from file path
    @param to file path
*)

val rename :
  Opendal_core.Operator.operator -> string -> string -> (unit, string) result
(** [rename operator from to] Rename a file from [from] to [to].
    - [from] and [to] must be a file.
    - [to] will be overwritten if it exists.
    - If [from] and [to] are the same, a `IsSameFile` error will occur.
    @param operator The operator
    @param from file path
    @param to file path
*)

val delete : Opendal_core.Operator.operator -> string -> (unit, string) result
(** [delete operator path] Delete given path.
    - Delete not existing error won't return errors.
    @param operator The block operator
    @param path file path
*)

val remove :
  Opendal_core.Operator.operator -> string array -> (unit, string) result
(** [remove operator paths] Remove path array.
    - We don't support batch delete now, will call delete on each object in turn
    @param operator The block operator
    @param paths file path array
*)

val remove_all :
  Opendal_core.Operator.operator -> string -> (unit, string) result
(** [remove_all operator path] Remove the path and all nested dirs and files recursively.
    - We don't support batch delete now, will call delete on each object in turn
    @param operator The block operator
    @param path file path
*)
