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

(** {1 OpenDAL OCaml Bindings}

    Apache OpenDAL™ OCaml binding provides a unified data access layer that
    allows users to easily and efficiently retrieve data from various storage
    services.

    {2 Basic Usage}

    {[
    (* Create an operator for local filesystem *)
    let op = Operator.new_operator "fs" [ ("root", "/tmp") ] |> Result.get_ok in

    (* Write data to a file *)
    let _ = Operator.write op "hello.txt" (Bytes.of_string "Hello, World!") in

    (* Read data back *)
    let content = Operator.read op "hello.txt" |> Result.get_ok in
    print_endline (String.of_bytes content)
    ]} *)

(** {2 Core Operations} *)

val new_operator :
  string ->
  (string * string) list ->
  (Opendal_core.Operator.operator, string) result
(** [new_operator scheme config_map] creates a new blocking operator from given
    scheme and configuration.

    @param scheme
      The storage service scheme. Supported services include:
      - "fs" for local filesystem
      - "s3" for Amazon S3
      - "gcs" for Google Cloud Storage
      - "azblob" for Azure Blob Storage
      - And many more. See
        {{:https://opendal.apache.org/docs/category/services/} the full list}.

    @param config_map
      Configuration key-value pairs required by the target service. For example,
      for S3: [("bucket", "my-bucket"); ("region", "us-east-1")]

    @return A blocking operator wrapped in Result.

    Example:
    {[
    (* Local filesystem *)
    let fs_op = new_operator "fs" [ ("root", "/tmp") ]

    (* S3 storage *)
    let s3_op =
      new_operator "s3"
        [
          ("bucket", "my-bucket");
          ("region", "us-east-1");
          ("access_key_id", "...");
          ("secret_access_key", "...");
        ]
    ]} *)

val list :
  Opendal_core.Operator.operator ->
  string ->
  (Opendal_core.Operator.entry array, string) result
(** [list operator path] lists all entries in the given directory path.

    @param operator The blocking operator
    @param path Directory path to list (should end with "/")
    @return Array of directory entries

    Note: This loads all entries into memory. For large directories, consider
    using {!val-lister} for streaming access.

    Example:
    {[
    match list op "data/" with
    | Ok entries ->
        Array.iter
          (fun entry -> printf "Found: %s\n" (Entry.name entry))
          entries
    | Error err -> printf "Error: %s\n" err
    ]} *)

val lister :
  Opendal_core.Operator.operator ->
  string ->
  (Opendal_core.Operator.lister, string) result
(** [lister operator path] creates a streaming lister for the given directory.

    @param operator The blocking operator
    @param path Directory path to list (should end with "/")
    @return A lister for streaming access to entries

    Use {!Lister.next} to iterate through entries one by one. This is
    memory-efficient for large directories.

    Example:
    {[
    match lister op "data/" with
    | Ok lst ->
        let rec iter () =
          match Lister.next lst with
          | Ok (Some entry) ->
              printf "Found: %s\n" (Entry.name entry);
              iter ()
          | Ok None -> () (* End of listing *)
          | Error err -> printf "Error: %s\n" err
        in
        iter ()
    | Error err -> printf "Error: %s\n" err
    ]} *)

val stat :
  Opendal_core.Operator.operator ->
  string ->
  (Opendal_core.Operator.metadata, string) result
(** [stat operator path] gets metadata for the given path.

    @param operator The blocking operator
    @param path Path to get metadata for
    @return Metadata for the path

    Example:
    {[
    match stat op "file.txt" with
    | Ok meta ->
        printf "Size: %Ld bytes\n" (Metadata.content_length meta);
        printf "Is file: %b\n" (Metadata.is_file meta)
    | Error err -> printf "Error: %s\n" err
    ]} *)

val is_exist : Opendal_core.Operator.operator -> string -> (bool, string) result
(** [is_exist operator path] checks if the given path exists.

    @param operator The blocking operator
    @param path Path to check
    @return true if path exists, false otherwise

    Example:
    {[
    match is_exist op "file.txt" with
    | Ok true -> print_endline "File exists"
    | Ok false -> print_endline "File does not exist"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val create_dir :
  Opendal_core.Operator.operator -> string -> (bool, string) result
(** [create_dir operator path] creates a directory at the given path.

    @param operator The blocking operator
    @param path Directory path to create (must end with "/")

    Notes:
    - Creating existing directories succeeds
    - Creates parent directories recursively (like "mkdir -p")
    - Path must end with "/" to indicate it's a directory

    Example:
    {[
    match create_dir op "data/subdir/" with
    | Ok () -> print_endline "Directory created"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val read :
  Opendal_core.Operator.operator -> string -> (char array, string) result
(** [read operator path] reads the entire file content into memory.

    @param operator The blocking operator
    @param path File path to read
    @return File content as a char array

    For large files or streaming access, consider using {!val-reader}.

    Example:
    {[
    match read op "file.txt" with
    | Ok content ->
        let bytes = Array.to_seq content |> Bytes.of_seq in
        print_endline (Bytes.to_string bytes)
    | Error err -> printf "Error: %s\n" err
    ]} *)

val reader :
  Opendal_core.Operator.operator ->
  string ->
  (Opendal_core.Operator.reader, string) result
(** [reader operator path] creates a reader for streaming file access.

    @param operator The blocking operator
    @param path File path to read
    @return A reader for streaming access

    Use {!Reader.pread} to read data from specific positions.

    Example:
    {[
    match reader op "file.txt" with
    | Ok r -> (
        let buf = Bytes.create 1024 in
        match Reader.pread r buf 0L with
        | Ok bytes_read -> printf "Read %d bytes\n" bytes_read
        | Error err -> printf "Error: %s\n" err)
    | Error err -> printf "Error: %s\n" err
    ]} *)

val write :
  Opendal_core.Operator.operator -> string -> bytes -> (unit, string) result
(** [write operator path data] writes data to the given path.

    @param operator The blocking operator
    @param path File path to write to
    @param data Data to write

    Notes:
    - Overwrites existing files
    - Creates parent directories if needed
    - Ensures all data is written atomically

    Example:
    {[
    let data = Bytes.of_string "Hello, World!" in
    match write op "hello.txt" data with
    | Ok () -> print_endline "File written"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val writer :
  Opendal_core.Operator.operator ->
  string ->
  (Opendal_core.Operator.writer, string) result
(** [writer operator path] creates a writer for streaming file writes.

    @param operator The blocking operator
    @param path File path to write to
    @return A writer for streaming writes

    Use {!Writer.write} to write data chunks and {!Writer.close} to finalize.

    Example:
    {[
    match writer op "large_file.txt" with
    | Ok w ->
        let _ = Writer.write w (Bytes.of_string "chunk1") in
        let _ = Writer.write w (Bytes.of_string "chunk2") in
        Writer.close w
    | Error err -> printf "Error: %s\n" err
    ]} *)

val copy :
  Opendal_core.Operator.operator -> string -> string -> (unit, string) result
(** [copy operator from to] copies a file from source to destination.

    @param operator The blocking operator
    @param from Source file path
    @param to Destination file path

    Notes:
    - Overwrites destination if it exists
    - Creates parent directories if needed
    - Both paths must be files (not directories)

    Example:
    {[
    match copy op "source.txt" "backup.txt" with
    | Ok () -> print_endline "File copied"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val rename :
  Opendal_core.Operator.operator -> string -> string -> (unit, string) result
(** [rename operator from to] renames/moves a file from source to destination.

    @param operator The blocking operator
    @param from Source file path
    @param to Destination file path

    Notes:
    - Overwrites destination if it exists
    - Creates parent directories if needed
    - Source file is removed after successful operation

    Example:
    {[
    match rename op "old_name.txt" "new_name.txt" with
    | Ok () -> print_endline "File renamed"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val delete : Opendal_core.Operator.operator -> string -> (unit, string) result
(** [delete operator path] deletes the file at the given path.

    @param operator The blocking operator
    @param path File path to delete

    Notes:
    - Succeeds even if file doesn't exist (idempotent)
    - Cannot be used to delete directories (use {!remove_all} instead)

    Example:
    {[
    match delete op "unwanted.txt" with
    | Ok () -> print_endline "File deleted"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val remove :
  Opendal_core.Operator.operator -> string array -> (unit, string) result
(** [remove operator paths] deletes multiple files in a batch operation.

    @param operator The blocking operator
    @param paths Array of file paths to delete

    This is more efficient than calling {!delete} multiple times for services
    that support batch deletion.

    Example:
    {[
    let files = [| "file1.txt"; "file2.txt"; "file3.txt" |] in
    match remove op files with
    | Ok () -> print_endline "Files deleted"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val remove_all :
  Opendal_core.Operator.operator -> string -> (unit, string) result
(** [remove_all operator path] recursively deletes the directory and all its
    contents.

    @param operator The blocking operator
    @param path Directory path to delete (should end with "/")

    ⚠️ WARNING: This operation permanently deletes all files and subdirectories.
    Use with extreme caution!

    Example:
    {[
    match remove_all op "temp_data/" with
    | Ok () -> print_endline "Directory deleted"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val check : Opendal_core.Operator.operator -> (unit, string) result
(** [check operator] performs a health check on the storage service.

    @param operator The blocking operator

    This verifies that the operator can connect to and access the configured
    storage service.

    Example:
    {[
    match check op with
    | Ok () -> print_endline "Storage service is accessible"
    | Error err -> printf "Error: %s\n" err
    ]} *)

val info : Opendal_core.Operator.operator -> Opendal_core.Operator.operator_info
(** [info operator] returns information about the operator.

    @param operator The blocking operator
    @return Operator information (name, scheme, root path, capabilities)

    Example:
    {[
    let info = info op in
    printf "Service: %s\n" (OperatorInfo.name info);
    printf "Scheme: %s\n" (OperatorInfo.scheme info);
    printf "Root: %s\n" (OperatorInfo.root info)
    ]} *)

(** {2 Reader Operations}

    Module for reading data from files with fine-grained control. *)
module Reader : sig
  val pread :
    Opendal_core.Operator.reader -> bytes -> int64 -> (int, string) result
  (** [pread reader buf offset] reads data from the reader at the given offset.

      @param reader The reader instance
      @param buf Buffer to read data into
      @param offset Byte offset in the file to start reading from
      @return Number of bytes actually read

      Example:
      {[
      let buf = Bytes.create 1024 in
      match Reader.pread reader buf 100L with
      | Ok bytes_read ->
          printf "Read %d bytes starting at offset 100\n" bytes_read
      | Error err -> printf "Error: %s\n" err
      ]} *)
end

(** {2 Writer Operations}

    Module for writing data to files in chunks. *)
module Writer : sig
  val write : Opendal_core.Operator.writer -> bytes -> (unit, string) result
  (** [write writer data] writes a chunk of data.

      @param writer The writer instance
      @param data Data chunk to write

      Example:
      {[
      match Writer.write writer (Bytes.of_string "Hello ") with
      | Ok () -> (
          match Writer.write writer (Bytes.of_string "World!") with
          | Ok () -> print_endline "Data written"
          | Error err -> printf "Error: %s\n" err)
      | Error err -> printf "Error: %s\n" err
      ]} *)

  val close :
    Opendal_core.Operator.writer ->
    (Opendal_core.Operator.metadata, string) result
  (** [close writer] finalizes the write operation and returns metadata.

      Must be called to ensure all data is properly written and committed.

      @param writer The writer instance
      @return Metadata about the written file

      Example:
      {[
      (* After writing all chunks *)
      match Writer.close writer with
      | Ok metadata ->
          printf "File closed successfully, size: %Ld bytes\n"
            (Metadata.content_length metadata)
      | Error err -> printf "Error closing file: %s\n" err
      ]} *)
end

(** {2 Lister Operations}

    Module for streaming directory listings. *)
module Lister : sig
  val next :
    Opendal_core.Operator.lister ->
    (Opendal_core.Operator.entry option, string) result
  (** [next lister] gets the next entry from the directory listing.

      @param lister The lister instance
      @return Some entry if available, None if no more entries

      Example:
      {[
      let rec process_all lister =
        match Lister.next lister with
        | Ok (Some entry) ->
            printf "Processing: %s\n" (Entry.name entry);
            process_all lister
        | Ok None -> print_endline "Finished processing all entries"
        | Error err -> printf "Error: %s\n" err
      in
      process_all my_lister
      ]} *)
end

(** {2 Metadata Operations}

    Module for accessing file and directory metadata. *)
module Metadata : sig
  val is_file : Opendal_core.Operator.metadata -> bool
  (** [is_file metadata] checks if the metadata represents a file.

      Example:
      {[
      if Metadata.is_file meta then print_endline "It's a file"
      ]} *)

  val is_dir : Opendal_core.Operator.metadata -> bool
  (** [is_dir metadata] checks if the metadata represents a directory.

      Example:
      {[
      if Metadata.is_dir meta then print_endline "It's a directory"
      ]} *)

  val content_length : Opendal_core.Operator.metadata -> int64
  (** [content_length metadata] gets the size of the content in bytes.

      Example:
      {[
      printf "File size: %Ld bytes\n" (Metadata.content_length meta)
      ]} *)

  val content_md5 : Opendal_core.Operator.metadata -> string option
  (** [content_md5 metadata] gets the MD5 hash of the content, if available. *)

  val content_type : Opendal_core.Operator.metadata -> string option
  (** [content_type metadata] gets the MIME type of the content, if available.
  *)

  val content_disposition : Opendal_core.Operator.metadata -> string option
  (** [content_disposition metadata] gets the Content-Disposition header, if
      available. *)

  val etag : Opendal_core.Operator.metadata -> string option
  (** [etag metadata] gets the ETag of the content, if available. *)

  val last_modified : Opendal_core.Operator.metadata -> int64 option
  (** [last_modified metadata] gets the last modification time as Unix
      timestamp, if available. *)
end

(** {2 Entry Operations}

    Module for accessing directory entry information. *)
module Entry : sig
  val path : Opendal_core.Operator.entry -> string
  (** [path entry] gets the full path of the directory entry.

      Example:
      {[
      printf "Full path: %s\n" (Entry.path entry)
      ]} *)

  val name : Opendal_core.Operator.entry -> string
  (** [name entry] gets the name (filename) of the directory entry.

      Example:
      {[
      printf "Name: %s\n" (Entry.name entry)
      ]} *)

  val metadata : Opendal_core.Operator.entry -> Opendal_core.Operator.metadata
  (** [metadata entry] gets the metadata for the directory entry.

      Example:
      {[
      let meta = Entry.metadata entry in
      printf "Size: %Ld\n" (Metadata.content_length meta)
      ]} *)
end

(** {2 Operator Information}

    Module for accessing operator configuration and capabilities. *)
module OperatorInfo : sig
  val name : Opendal_core.Operator.operator_info -> string
  (** [name info] gets the name of the operator instance. *)

  val scheme : Opendal_core.Operator.operator_info -> string
  (** [scheme info] gets the storage scheme (e.g., "fs", "s3", "gcs").

      Example:
      {[
      printf "Using scheme: %s\n" (OperatorInfo.scheme info)
      ]} *)

  val root : Opendal_core.Operator.operator_info -> string
  (** [root info] gets the root path configured for the operator.

      Example:
      {[
      printf "Root path: %s\n" (OperatorInfo.root info)
      ]} *)

  val capability :
    Opendal_core.Operator.operator_info -> Opendal_core.Operator.capability
  (** [capability info] gets the capability information for the operator.

      Example:
      {[
      let cap = OperatorInfo.capability info in
      if Capability.write cap then print_endline "Write operations supported"
      ]} *)
end

(** {2 Capability Checking}

    Module for checking what operations are supported by the storage backend. *)
module Capability : sig
  val stat : Opendal_core.Operator.capability -> bool
  (** [stat cap] checks if stat operations are supported. *)

  val read : Opendal_core.Operator.capability -> bool
  (** [read cap] checks if read operations are supported. *)

  val write : Opendal_core.Operator.capability -> bool
  (** [write cap] checks if write operations are supported. *)

  val create_dir : Opendal_core.Operator.capability -> bool
  (** [create_dir cap] checks if directory creation is supported. *)

  val delete : Opendal_core.Operator.capability -> bool
  (** [delete cap] checks if delete operations are supported. *)

  val copy : Opendal_core.Operator.capability -> bool
  (** [copy cap] checks if copy operations are supported. *)

  val rename : Opendal_core.Operator.capability -> bool
  (** [rename cap] checks if rename operations are supported. *)

  val list : Opendal_core.Operator.capability -> bool
  (** [list cap] checks if list operations are supported. *)

  val list_with_limit : Opendal_core.Operator.capability -> bool
  (** [list_with_limit cap] checks if list operations with limit are supported.
  *)

  val list_with_start_after : Opendal_core.Operator.capability -> bool
  (** [list_with_start_after cap] checks if list operations with start_after are
      supported. *)

  val list_with_recursive : Opendal_core.Operator.capability -> bool
  (** [list_with_recursive cap] checks if recursive list operations are
      supported. *)

  val presign : Opendal_core.Operator.capability -> bool
  (** [presign cap] checks if presigned URL generation is supported. *)

  val presign_read : Opendal_core.Operator.capability -> bool
  (** [presign_read cap] checks if presigned read URLs are supported. *)

  val presign_stat : Opendal_core.Operator.capability -> bool
  (** [presign_stat cap] checks if presigned stat URLs are supported. *)

  val presign_write : Opendal_core.Operator.capability -> bool
  (** [presign_write cap] checks if presigned write URLs are supported. *)

  val shared : Opendal_core.Operator.capability -> bool
  (** [shared cap] checks if the operator can be safely shared between threads.
  *)
end
