(*
   Licensed to the Apache Software Foundation (ASF) under one
   or more contributor license agreements.  See the NOTICE file
   distributed with this work for additional information
   regarding copyright ownership.  The ASF licenses this file
   to you under the Apache License, Version 2.0 (the
   "License"); you may not use this file except in compliance
   with the License.  You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing,
   software distributed under the License is distributed on an
   "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
   KIND, either express or implied.  See the License for the
   specific language governing permissions and limitations
   under the License.
*)

(* ANCHOR: quickstart *)
open Opendal

(* Return the value, or print the error and exit. Every OpenDAL call returns a
   [(_, string) result], so this keeps the example flat. *)
let or_fail = function
  | Ok v -> v
  | Error err ->
      prerr_endline err;
      exit 1

let () =
  (* Create an operator for the in-memory service — no credentials needed. *)
  let op = or_fail (Operator.new_operator "memory" []) in

  (* Write a file, then read it back (read returns a char array). *)
  or_fail (Operator.write op "hello.txt" (Bytes.of_string "Hello, World!"));
  let content = or_fail (Operator.read op "hello.txt") in
  let text = content |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string in
  Printf.printf "read: %s\n" text;

  (* Inspect metadata, then delete. *)
  let meta = or_fail (Operator.stat op "hello.txt") in
  Printf.printf "size = %Ld bytes\n" (Operator.Metadata.content_length meta);
  or_fail (Operator.delete op "hello.txt")
(* ANCHOR_END: quickstart *)
