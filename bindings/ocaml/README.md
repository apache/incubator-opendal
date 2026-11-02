# Apache OpenDAL™ OCaml Binding (WIP)

[![status: unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/ocaml)

An OCaml binding for [Apache OpenDAL](https://opendal.apache.org/) — access
S3, GCS, Azure Blob, local filesystem, and 50+ more storage services through
one API, backed by the Rust core.

> **Experimental / Work in Progress.** APIs may change without notice. Not
> published to opam. Build from source only.

## Useful Links

- **User guide**: https://opendal.apache.org/docs/bindings/ocaml
- **Services & configuration**: https://opendal.apache.org/services
- **Source**: [`bindings/ocaml/`](https://github.com/apache/opendal/tree/main/bindings/ocaml)

## Requirements

- OCaml >= 4.10 and < 5.0 (OCaml 5 is not yet supported)
- Rust stable toolchain (for the `build.rs` native compilation step)
- dune >= 2.1

## Build & Install

Install opam and create a 4.x switch:

```bash
# Install opam (if needed)
bash -c "sh <(curl -fsSL https://raw.githubusercontent.com/ocaml/opam/master/shell/install.sh)"

# macOS
brew install opam

# Debian/Ubuntu
apt-get install opam

# Arch
pacman -S opam
```

Initialize opam and create a switch:

```bash
opam init --bare -a -y          # use --disable-sandboxing on WSL1
opam switch create opendal-ocaml4.14 ocaml-base-compiler.4.14.0
eval $(opam env)
```

Install optional dev tools (recommended for IDE support):

```bash
opam install -y utop odoc ounit2 ocaml-lsp-server ocamlformat ocamlformat-rpc
```

Build and test:

```bash
cd bindings/ocaml
dune build
dune test
```

## Quickstart

```ocaml
open Opendal

(* Create an operator for the local filesystem *)
let () =
  match Operator.new_operator "fs" [ ("root", "/tmp") ] with
  | Error err -> Printf.eprintf "Error: %s\n" err
  | Ok op ->

  (* Write a file *)
  ignore (Operator.write op "hello.txt" (Bytes.of_string "Hello, World!"));

  (* Read it back *)
  (match Operator.read op "hello.txt" with
  | Ok content ->
      let text = content |> Array.to_seq |> Bytes.of_seq |> Bytes.to_string in
      print_endline text    (* Hello, World! *)
  | Error err -> Printf.eprintf "Error: %s\n" err)
```

Every fallible function returns `('a, string) result`. For S3 or other
backends, change the scheme and pass the appropriate config keys:

```ocaml
match Operator.new_operator "s3"
  [ ("bucket", "my-bucket"); ("region", "us-east-1")
  ; ("access_key_id", "..."); ("secret_access_key", "...") ]
with
| Error err -> Printf.eprintf "Error: %s\n" err
| Ok _op -> ()   (* use _op here *)
```

See the [user guide](https://opendal.apache.org/docs/bindings/ocaml) for
examples covering streaming reads/writes, directory listing, and capability
checks.

## Contributing

Contributions are welcome. Standard OCaml style; format with `ocamlformat`
before submitting. See the repository
[CONTRIBUTING.md](https://github.com/apache/opendal/blob/main/CONTRIBUTING.md)
for the general process.

## License and Trademarks

Licensed under the Apache License, Version 2.0:
http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or
trademarks of the Apache Software Foundation.
