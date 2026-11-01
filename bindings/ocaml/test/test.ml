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

open Opendal
open OUnit2

let test_check_result = function
  | Ok data -> data
  | Error err -> assert_failure err

let new_test_block_operator test_ctxt : operator =
  let cfgs = [ ("root", bracket_tmpdir test_ctxt) ] in
  test_check_result (Operator.new_operator "fs" cfgs)

(** Basic operator creation tests *)
let test_new_block_operator _ = ignore new_test_block_operator

(** Test directory creation and recursive removal *)
let test_create_dir_and_remove_all test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  ignore (test_check_result (Operator.create_dir bo "/testdir/"));
  ignore
    (test_check_result
       (Operator.write bo "/testdir/foo" (Bytes.of_string "bar")));
  ignore
    (test_check_result
       (Operator.write bo "/testdir/bar" (Bytes.of_string "foo")));
  ignore (test_check_result (Operator.remove_all bo "/testdir/"))

(** Test basic write and read operations *)
let test_block_write_and_read test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  ignore
    (test_check_result
       (Operator.write bo "tempfile" (Bytes.of_string "helloworld")));
  let data = test_check_result (Operator.read bo "tempfile") in
  assert_equal "helloworld"
    (data |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string)

(** Test reader functionality *)
let test_operator_reader test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  ignore
    (test_check_result
       (Operator.write bo "tempfile" (Bytes.of_string "helloworld")));
  let reader = Operator.reader bo "tempfile" |> test_check_result in
  let data = Bytes.create 5 in
  let i = Operator.Reader.pread reader data 5L |> test_check_result in
  assert_equal 5 i;
  assert_equal "world" (Bytes.to_string data)

(** Test writer functionality *)
let test_operator_writer test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  let writer = Operator.writer bo "test_writer_file" |> test_check_result in
  ignore
    (test_check_result
       (Operator.Writer.write writer (Bytes.of_string "Hello ")));
  ignore
    (test_check_result
       (Operator.Writer.write writer (Bytes.of_string "World!")));
  ignore (test_check_result (Operator.Writer.close writer));

  (* Verify the content was written correctly *)
  let data = test_check_result (Operator.read bo "test_writer_file") in
  assert_equal "Hello World!"
    (data |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string)

(** Test metadata functionality *)
let test_operator_stat test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  ignore
    (test_check_result
       (Operator.write bo "tempfile" (Bytes.of_string "helloworld")));
  let metadata = Operator.stat bo "tempfile" |> test_check_result in
  assert_equal false (Operator.Metadata.is_dir metadata);
  assert_equal true (Operator.Metadata.is_file metadata);
  assert_equal 10L (Operator.Metadata.content_length metadata);
  ()

(** Test list functionality *)
let test_list test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  ignore (test_check_result (Operator.create_dir bo "/testdir/"));
  ignore
    (test_check_result
       (Operator.write bo "/testdir/foo" (Bytes.of_string "bar")));
  ignore
    (test_check_result
       (Operator.write bo "/testdir/bar" (Bytes.of_string "foo")));
  let array = Operator.list bo "testdir/" |> test_check_result in
  let actual = Array.map Operator.Entry.name array in
  let expected = [| "testdir/"; "foo"; "bar" |] in
  List.iter (Array.sort compare) [ expected; actual ];
  assert_equal expected actual;
  assert_equal 3 (Array.length array);
  ()

(** Test streaming lister functionality *)
let test_lister test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  ignore (test_check_result (Operator.create_dir bo "/testdir/"));
  ignore
    (test_check_result
       (Operator.write bo "/testdir/file1" (Bytes.of_string "content1")));
  ignore
    (test_check_result
       (Operator.write bo "/testdir/file2" (Bytes.of_string "content2")));

  let lister = Operator.lister bo "testdir/" |> test_check_result in
  let entries = ref [] in

  (* Collect all entries using the lister *)
  let rec collect_entries () =
    match Operator.Lister.next lister with
    | Ok (Some entry) ->
        entries := Operator.Entry.name entry :: !entries;
        collect_entries ()
    | Ok None -> ()
    | Error err -> assert_failure err
  in
  collect_entries ();

  let actual = Array.of_list (List.rev !entries) in
  let expected = [| "testdir/"; "file1"; "file2" |] in
  List.iter (Array.sort compare) [ expected; actual ];
  assert_equal expected actual;
  assert_equal 3 (Array.length actual)

(** Test operator info functionality *)
let test_operator_info test_ctxt =
  let bo = new_test_block_operator test_ctxt in
  let info = Operator.info bo in

  (* Test basic info properties *)
  let scheme = Operator.OperatorInfo.scheme info in
  assert_equal "fs" scheme;

  let root = Operator.OperatorInfo.root info in
  assert_bool "Root should not be empty" (String.length root > 0);

  (* Test capability information *)
  let cap = Operator.OperatorInfo.capability info in
  assert_equal true (Operator.Capability.read cap);
  assert_equal true (Operator.Capability.write cap);
  assert_equal true (Operator.Capability.stat cap);
  assert_equal true (Operator.Capability.list cap);
  assert_equal true (Operator.Capability.create_dir cap);
  assert_equal true (Operator.Capability.delete cap);

  (* Filesystem should support these operations *)
  assert_equal true (Operator.Capability.copy cap);
  assert_equal true (Operator.Capability.rename cap)

(** Test file existence checking *)
let test_is_exist test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Test non-existent file *)
  let exists = Operator.is_exist bo "nonexistent.txt" |> test_check_result in
  assert_equal false exists;

  (* Create a file and test it exists *)
  ignore
    (test_check_result
       (Operator.write bo "testfile.txt" (Bytes.of_string "test")));
  let exists = Operator.is_exist bo "testfile.txt" |> test_check_result in
  assert_equal true exists

(** Test file copy operation *)
let test_copy test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Create source file *)
  let content = "Hello, copy test!" in
  ignore
    (test_check_result
       (Operator.write bo "source.txt" (Bytes.of_string content)));

  (* Copy the file *)
  ignore (test_check_result (Operator.copy bo "source.txt" "destination.txt"));

  (* Verify both files exist and have same content *)
  let source_data = test_check_result (Operator.read bo "source.txt") in
  let dest_data = test_check_result (Operator.read bo "destination.txt") in

  let source_str =
    source_data |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string
  in
  let dest_str = dest_data |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string in

  assert_equal content source_str;
  assert_equal content dest_str

(** Test file rename operation *)
let test_rename test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Create source file *)
  let content = "Hello, rename test!" in
  ignore
    (test_check_result
       (Operator.write bo "old_name.txt" (Bytes.of_string content)));

  (* Rename the file *)
  ignore (test_check_result (Operator.rename bo "old_name.txt" "new_name.txt"));

  (* Verify old file doesn't exist and new file has correct content *)
  let old_exists = Operator.is_exist bo "old_name.txt" |> test_check_result in
  let new_exists = Operator.is_exist bo "new_name.txt" |> test_check_result in

  assert_equal false old_exists;
  assert_equal true new_exists;

  let new_data = test_check_result (Operator.read bo "new_name.txt") in
  let new_str = new_data |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string in
  assert_equal content new_str

(** Test delete operation *)
let test_delete test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Create a file *)
  ignore
    (test_check_result
       (Operator.write bo "to_delete.txt" (Bytes.of_string "delete me")));

  (* Verify it exists *)
  let exists_before =
    Operator.is_exist bo "to_delete.txt" |> test_check_result
  in
  assert_equal true exists_before;

  (* Delete it *)
  ignore (test_check_result (Operator.delete bo "to_delete.txt"));

  (* Verify it's gone *)
  let exists_after =
    Operator.is_exist bo "to_delete.txt" |> test_check_result
  in
  assert_equal false exists_after

(** Test remove (batch delete) operation *)
let test_remove test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Create multiple files *)
  let files = [| "file1.txt"; "file2.txt"; "file3.txt" |] in
  Array.iter
    (fun file ->
      ignore
        (test_check_result
           (Operator.write bo file (Bytes.of_string ("content of " ^ file)))))
    files;

  (* Verify all files exist *)
  Array.iter
    (fun file ->
      let exists = Operator.is_exist bo file |> test_check_result in
      assert_equal true exists)
    files;

  (* Remove all files *)
  ignore (test_check_result (Operator.remove bo files));

  (* Verify all files are gone *)
  Array.iter
    (fun file ->
      let exists = Operator.is_exist bo file |> test_check_result in
      assert_equal false exists)
    files

(** Test operator check functionality *)
let test_check test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Check should succeed for a valid operator *)
  match Operator.check bo with
  | Ok () -> () (* Success is expected *)
  | Error err -> assert_failure ("Check failed: " ^ err)

(** Test multiple operations together *)
let test_complex_workflow test_ctxt =
  let bo = new_test_block_operator test_ctxt in

  (* Create directory structure *)
  ignore (test_check_result (Operator.create_dir bo "project/"));
  ignore (test_check_result (Operator.create_dir bo "project/src/"));
  ignore (test_check_result (Operator.create_dir bo "project/docs/"));

  (* Create some files using writer *)
  let readme_writer =
    Operator.writer bo "project/README.md" |> test_check_result
  in
  ignore
    (test_check_result
       (Operator.Writer.write readme_writer (Bytes.of_string "# My Project\n")));
  ignore
    (test_check_result
       (Operator.Writer.write readme_writer
          (Bytes.of_string "This is a test project.\n")));
  ignore (test_check_result (Operator.Writer.close readme_writer));

  (* Create a source file *)
  ignore
    (test_check_result
       (Operator.write bo "project/src/main.ml"
          (Bytes.of_string "let () = print_endline \"Hello, World!\"")));

  (* List the project directory *)
  let entries = Operator.list bo "project/" |> test_check_result in
  assert_equal 4 (Array.length entries);

  (* project/, src/, docs/, README.md *)

  (* Verify file contents *)
  let readme_data = test_check_result (Operator.read bo "project/README.md") in
  let readme_str =
    readme_data |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string
  in
  let contains_substring s sub =
    let len_s = String.length s in
    let len_sub = String.length sub in
    let rec check i =
      if i > len_s - len_sub then false
      else if String.sub s i len_sub = sub then true
      else check (i + 1)
    in
    if len_sub = 0 then true else check 0
  in
  assert_bool "README should contain project info"
    (contains_substring readme_str "My Project");

  (* Copy the main file *)
  ignore
    (test_check_result
       (Operator.copy bo "project/src/main.ml" "project/src/backup.ml"));

  (* Check capabilities *)
  let info = Operator.info bo in
  let cap = Operator.OperatorInfo.capability info in
  assert_equal true (Operator.Capability.write cap);
  assert_equal true (Operator.Capability.copy cap);

  (* Clean up *)
  ignore (test_check_result (Operator.remove_all bo "project/"))

let suite =
  "OCaml OpenDAL Test Suite"
  >::: [
         "test_new_block_operator" >:: test_new_block_operator;
         "test_create_dir_and_remove_all" >:: test_create_dir_and_remove_all;
         "test_block_write_and_read" >:: test_block_write_and_read;
         "test_operator_reader" >:: test_operator_reader;
         "test_operator_writer" >:: test_operator_writer;
         "test_operator_stat" >:: test_operator_stat;
         "test_list" >:: test_list;
         "test_lister" >:: test_lister;
         "test_operator_info" >:: test_operator_info;
         "test_is_exist" >:: test_is_exist;
         "test_copy" >:: test_copy;
         "test_rename" >:: test_rename;
         "test_delete" >:: test_delete;
         "test_remove" >:: test_remove;
         "test_check" >:: test_check;
         "test_complex_workflow" >:: test_complex_workflow;
       ]

let () = run_test_tt_main suite
