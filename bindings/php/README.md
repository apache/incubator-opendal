# Apache OpenDAL™ PHP Binding

[![status: unreleased](https://img.shields.io/badge/status-unreleased-red)](https://opendal.apache.org/docs/bindings/php)

A PHP extension for OpenDAL: access S3, GCS, Azure Blob, the local filesystem,
and 50+ more services through one synchronous API, built on the Rust core.

**Status**: experimental / work in progress. Not published to any package
registry. Expect breaking changes.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/php](https://opendal.apache.org/docs/bindings/php)
- **Services & configuration**: [opendal.apache.org/services](https://opendal.apache.org/services)
- **Source**: [`bindings/php/`](https://github.com/apache/opendal/tree/main/bindings/php)
- **ext-php-rs**: [github.com/davidcole1340/ext-php-rs](https://github.com/davidcole1340/ext-php-rs)

## Requirements

- PHP 8.1+
- Rust and Cargo — [rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

## Build and install the extension

The extension cannot be installed via `pecl` or Composer. Build from source:

```bash
git clone https://github.com/apache/opendal.git
cd opendal/bindings/php
cargo build          # add --release for production
```

Copy the library to PHP's extension directory, then register it in `php.ini`:

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

Or use `cargo-php` to build and install in one step:

```bash
cargo install cargo-php
cargo php install
```

Verify with:

```bash
php -m | grep opendal-php
```

## Quickstart

```php
use OpenDAL\Operator;

$op = new Operator("memory", []);

$op->write("hello.txt", "Hello, World!");

echo $op->read("hello.txt"); // Hello, World!

$meta = $op->stat("hello.txt");
echo $meta->content_length;  // 13

$op->delete("hello.txt");
```

Replace `"memory"` with any supported scheme and pass its configuration as the
second argument — for example, `new Operator("fs", ["root" => "/tmp"])`.

For the full guide including error handling and binary writes, see the
[user guide](https://opendal.apache.org/docs/bindings/php).

## Running the tests

```bash
cd opendal/bindings/php
composer install
composer test
```

## Contributing

All contributions go through the main OpenDAL repository at
[github.com/apache/opendal](https://github.com/apache/opendal). Open an issue
or pull request there.

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
