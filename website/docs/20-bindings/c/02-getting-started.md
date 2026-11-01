---
title: Getting started
sidebar_label: Getting started
description: Build an OpenDAL operator in C, write and read data, check errors, and free resources.
---

# Getting started

## Build the library

From `bindings/c/`:

```shell
mkdir -p build && cd build
cmake ..
make
```

This produces `libopendal_c` under `../../target/debug/`. Link against it and
include `bindings/c/include/opendal.h`.

## Your first program

This program builds a memory-backed operator, writes data, reads it back,
inspects the metadata, and frees every heap-allocated value. It requires no
credentials and no filesystem access.

```c file=bindings/c/examples/getting-started.c region=quickstart
```

Every operation returns either an `opendal_error *` (NULL on success) or a
result struct whose `.error` field is NULL on success. See
[Error handling](#error-handling) below.

## Point it at a real backend

The only change is the scheme and the options you pass to `opendal_operator_new`.
Operations stay identical across all services.

```c
/* Filesystem operator rooted at /tmp/mydata */
opendal_operator_options *opts = opendal_operator_options_new();
opendal_operator_options_set(opts, "root", "/tmp/mydata");

opendal_result_operator_new result = opendal_operator_new("fs", opts);
opendal_operator_options_free(opts);   /* options are copied; free immediately */

assert(result.op != NULL);
opendal_operator *op = result.op;
/* ... same write/read/stat/delete calls ... */
opendal_operator_free(op);
```

For S3 or any other service, set the configuration keys documented on
[/services](/services) as key-value pairs via `opendal_operator_options_set`.

## Error handling

C has no exceptions. Every function that can fail returns either a bare
`opendal_error *` or a result struct with an `.error` field:

- `NULL` → success.
- non-NULL → failure; inspect `.code` (an `opendal_code` enum) and `.message`
  (an `opendal_bytes` containing the error text, **not** NUL-terminated).
  Call `opendal_error_free` when done.

```c
opendal_result_read r = opendal_operator_read(op, "/no-such-file");
if (r.error != NULL) {
    /* .message is NOT NUL-terminated; use the length. */
    fprintf(stderr, "error %d: %.*s\n",
            r.error->code,
            (int)r.error->message.len,
            r.error->message.data);
    opendal_error_free(r.error);
    /* r.data is invalid when error != NULL */
}
```

Common error codes (from `opendal_code`):

| Code | Meaning |
|------|---------|
| `OPENDAL_NOT_FOUND` | Path does not exist |
| `OPENDAL_PERMISSION_DENIED` | Insufficient permissions |
| `OPENDAL_ALREADY_EXISTS` | Path already exists |
| `OPENDAL_UNSUPPORTED` | Operation not supported by this service |
| `OPENDAL_CONFIG_INVALID` | Operator configuration is invalid |

## Memory ownership rules

Every heap-allocated value returned by OpenDAL must be freed by the caller:

| Type | Free with |
|------|-----------|
| `opendal_operator *` | `opendal_operator_free` |
| `opendal_bytes` (field, not pointer) | `opendal_bytes_free` (pass by address) |
| `opendal_error *` | `opendal_error_free` |
| `opendal_metadata *` | `opendal_metadata_free` |
| `opendal_lister *` | `opendal_lister_free` |
| `opendal_entry *` | `opendal_entry_free` |
| `opendal_reader *` | `opendal_reader_free` |
| `opendal_writer *` | `opendal_writer_free` |
| `char *` returned by metadata / entry / info accessors | `opendal_string_free` |
| `opendal_operator_options *` | `opendal_operator_options_free` |

Do not use `free()` directly on any of these; they are allocated inside the Rust
runtime and must be released through the provided functions.
