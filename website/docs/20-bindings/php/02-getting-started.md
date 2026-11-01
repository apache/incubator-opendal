---
title: Getting started
sidebar_label: Getting started
description: Construct an OpenDAL operator in PHP, then write, read, inspect, and delete a file.
---

# Getting started

This page assumes the `opendal-php` extension is installed and enabled. If it
is not, follow the [build and install steps](./01-overview.md#building-and-installing-the-extension) first.

## Your first program

The example below builds an operator against the in-memory service — no
credentials or configuration needed — then writes a file, reads it back,
inspects its metadata, and deletes it.

```php file=bindings/php/examples/getting-started.php region=quickstart
```

## Point it at a real backend

Only the first two arguments to `Operator` change; the operations are identical
on every service.

```php
use OpenDAL\Operator;

// Local filesystem rooted at /tmp
$op = new Operator("fs", ["root" => "/tmp"]);

$op->write("hello.txt", "Hello from the filesystem!");
echo $op->read("hello.txt");
```

The first argument is the service scheme (`"fs"`, `"s3"`, `"memory"`, etc.).
The second argument is an associative array of service-specific configuration
keys. See [Services](/services) for the full list of schemes and their keys.

## Check existence before reading

`is_exist` returns `1` if the path exists and `0` otherwise — both are PHP **int**,
not bool. The same applies to `mode->is_file` and `mode->is_dir`. Using them
in an `if` or ternary works because PHP treats `1` as truthy and `0` as falsy,
but `=== true` will always be `false`. Prefer `=== 1` for strict checks.

```php
if ($op->is_exist("hello.txt")) {
    $meta = $op->stat("hello.txt");
    echo $meta->content_length . " bytes\n";
    // is_file returns 1 (int) for files, 0 for directories/other
    echo $meta->mode->is_file ? "file\n" : "directory\n";
}

// Strict comparison — use === 1, not === true
if ($op->is_exist("hello.txt") === 1) {
    // ...
}
```

## Write binary data

`write` requires valid UTF-8. For arbitrary bytes, convert with `unpack` first
and use `write_binary`:

```php
$bytes = unpack("C*", $binaryContent); // array of unsigned byte values
$op->write_binary("data.bin", $bytes);

$content = $op->read("data.bin");      // returns a binary-safe string
```

## Create a directory

The trailing slash is required; omitting it produces a `NotADirectory` error.
Creation is always recursive (equivalent to `mkdir -p`).

```php
$op->create_dir("logs/app/");
```

## Error handling

All methods throw a standard `Exception` on failure. The message comes from
the OpenDAL core and describes what went wrong (service error, path not found,
invalid configuration, and so on).

```php
use OpenDAL\Operator;

try {
    $op = new Operator("fs", ["root" => "/tmp"]);
    $content = $op->read("no-such-file.txt");
} catch (\Exception $e) {
    echo "Error: " . $e->getMessage() . "\n";
}
```

Common error causes:
- Passing an unknown scheme to the constructor.
- Missing required configuration keys (e.g. `"root"` for the `fs` service).
- Reading a path that does not exist.
- Passing non-UTF-8 content to `write` (use `write_binary` instead).
