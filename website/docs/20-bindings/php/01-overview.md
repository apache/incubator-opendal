---
title: PHP
sidebar_label: Overview
slug: /bindings/php
description: The OpenDAL PHP binding — experimental extension built from Rust source, how to install it, and where to go next.
---

# Apache OpenDAL™ PHP Binding

A PHP extension for OpenDAL: access S3, GCS, Azure Blob, the local
filesystem, and [50+ more services](/services) through one synchronous API,
with the performance of the Rust core underneath.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

## Status

**Experimental / work in progress.** The binding is unreleased and has no
published package. It is built from source as a native PHP extension using
[ext-php-rs](https://github.com/davidcole1340/ext-php-rs). The API may change
without notice. It is not ready for production use.

## Capabilities

- **Synchronous API** — all calls block; there is no async variant.
- **String and binary writes** — `write` for UTF-8 strings, `write_binary` for
  arbitrary byte arrays.
- **Metadata** — `stat` returns content length, content type, ETag, MD5, and
  entry mode (file or directory).
- **Directory operations** — `create_dir` creates directories recursively.
- **All services** that OpenDAL supports are available as long as they are
  enabled at compile time; see [Services](/services).

## Building and installing the extension

The extension cannot be installed with `pecl` or through Composer. You must
build it from the repository source.

**Requirements**

- PHP 8.1+
- Rust and Cargo — see [rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

**Step 1 — Clone the repository**

```bash
git clone https://github.com/apache/opendal.git
```

**Step 2 — Build the extension**

```bash
cd opendal/bindings/php
cargo build
```

Add `--release` for production builds.

**Step 3 — Enable the extension**

Copy the compiled library to PHP's extension directory and add the `extension=`
directive to `php.ini`:

```bash
# Linux
cp target/debug/libopendal_php.so $(php -r "echo ini_get('extension_dir');")/libopendal_php.so
echo "extension=libopendal_php.so" >> $(php -r "echo php_ini_loaded_file();")

# macOS
cp target/debug/libopendal_php.dylib $(php -r "echo ini_get('extension_dir');")/libopendal_php.dylib
echo "extension=libopendal_php.dylib" >> $(php -r "echo php_ini_loaded_file();")

# Windows
cp target/debug/libopendal_php.dll $(php -r "echo ini_get('extension_dir');")/libopendal_php.dll
echo "extension=libopendal_php.dll" >> $(php -r "echo php_ini_loaded_file();")
```

Alternatively, use `cargo-php` to build and install in one step:

```bash
cargo install cargo-php
cd opendal/bindings/php
cargo php install
```

**Verify the installation**

```bash
php -m | grep opendal-php
```

## Useful Links

- **Source & examples**: [`bindings/php/`](https://github.com/apache/opendal/tree/main/bindings/php)
- **Services & configuration**: [/services](/services)
- **ext-php-rs**: [github.com/davidcole1340/ext-php-rs](https://github.com/davidcole1340/ext-php-rs)

## Next steps

1. [Getting started](./02-getting-started.md) — construct an operator, then write,
   read, inspect, and delete a file.
