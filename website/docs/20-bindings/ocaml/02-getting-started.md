---
title: Getting started
sidebar_label: Getting started
description: Create an operator, read and write files, and handle errors in the OCaml binding.
---

# Getting started

:::warning Experimental / Work in Progress
This binding is unreleased and experimental. Build from source before following
this guide. See [Overview](./01-overview.md) for the build steps.
:::

## Your first program

The example below uses the `memory` service, which requires no credentials and
runs entirely in-process. Open the project in utop or add it to a dune
executable that depends on the `opendal` library.

```ocaml file=bindings/ocaml/examples/getting_started.ml region=quickstart
```

`Operator.read` returns `(char array, string) result`. Convert to `Bytes` with
`Array.to_seq |> Bytes.of_seq` and then to `string` with `Bytes.to_string`.

## Point it at a real backend

Only the scheme and config map change; every operation stays the same:

```ocaml
open Opendal

let () =
  let config =
    [ ("bucket", "my-bucket")
    ; ("region", "us-east-1")
    ; ("access_key_id", "...")
    ; ("secret_access_key", "...")
    ]
  in
  match Operator.new_operator "s3" config with
  | Error err -> Printf.eprintf "Failed: %s\n" err
  | Ok op ->
      ignore (Operator.write op "hello.txt" (Bytes.of_string "Hello from S3!"))
```

See [Services](/services) for every backend and its configuration keys.

## Error handling

Every fallible function returns `('a, string) result`. The idiomatic pattern is
`match`:

```ocaml
match Operator.read op "maybe-missing.txt" with
| Ok content ->
    (* use content *)
    let text = content |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string in
    print_endline text
| Error msg ->
    (* msg is a string description of the error *)
    Printf.eprintf "Error: %s\n" msg
```

You can also use `Result.get_ok` to raise an exception on error (useful in
tests and scripts):

```ocaml
let content = Operator.read op "file.txt" |> Result.get_ok in
```

## Streaming reads and writes

For large files, avoid loading everything into memory:

```ocaml
(* Streaming read — read 1 KiB starting at byte offset 0 *)
match Operator.reader op "big.bin" with
| Error err -> Printf.eprintf "Reader failed: %s\n" err
| Ok r ->
    let buf = Bytes.create 1024 in
    (match Operator.Reader.pread r buf 0L with
    | Ok n -> Printf.printf "Read %d bytes\n" n
    | Error err -> Printf.eprintf "pread failed: %s\n" err)
```

```ocaml
(* Streaming write — write in chunks, then close to commit *)
match Operator.writer op "big.bin" with
| Error err -> Printf.eprintf "Writer failed: %s\n" err
| Ok w ->
    ignore (Operator.Writer.write w (Bytes.of_string "first chunk\n"));
    ignore (Operator.Writer.write w (Bytes.of_string "second chunk\n"));
    (match Operator.Writer.close w with
    | Ok _meta -> print_endline "Upload complete"
    | Error err -> Printf.eprintf "Close failed: %s\n" err)
```

`Writer.close` must be called to commit the upload; it returns metadata for the
written object.

## Listing a directory

`list` loads all entries into an array; `lister` streams them one at a time:

```ocaml
(* Batch listing *)
match Operator.list op "data/" with
| Error err -> Printf.eprintf "List failed: %s\n" err
| Ok entries ->
    Array.iter
      (fun entry -> Printf.printf "%s\n" (Operator.Entry.name entry))
      entries
```

```ocaml
(* Streaming listing — memory-efficient for large directories *)
match Operator.lister op "data/" with
| Error err -> Printf.eprintf "Lister failed: %s\n" err
| Ok lst ->
    let rec loop () =
      match Operator.Lister.next lst with
      | Ok (Some entry) ->
          Printf.printf "%s\n" (Operator.Entry.name entry);
          loop ()
      | Ok None -> ()   (* end of listing *)
      | Error err -> Printf.eprintf "Next failed: %s\n" err
    in
    loop ()
```

## Checking capabilities

Not every service supports every operation. Query capabilities before calling
optional operations like `copy` or `rename`:

```ocaml
let info = Operator.info op in
let cap  = Operator.OperatorInfo.capability info in
if Operator.Capability.copy cap then
  ignore (Operator.copy op "source.txt" "backup.txt")
else
  print_endline "copy not supported by this backend"
```
