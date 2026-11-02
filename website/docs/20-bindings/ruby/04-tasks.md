---
title: Common tasks
sidebar_label: Common tasks
description: Task-oriented recipes for OpenDAL in Ruby — read, write, stream, list, delete, copy, and rename.
---

# Common tasks

Recipes for the things you actually do with storage. Each works on every
service. They assume an `op = OpenDal::Operator.new(...)` built as in
[Getting started](./02-getting-started.md). For full signatures, see the
[API reference](https://opendal.apache.org/docs/ruby/).

A few conventions used throughout:

- Paths are relative to the operator's root; a trailing `/` means a directory
  (`logs/app/` is a directory, `logs/app` is a file).
- `read` returns a `String`; `write` takes a `String`.
- For large data, open an `OpenDal::IO` with `op.open` and stream in chunks.

## Read a whole file

```ruby
data = op.read("path/to/file")     # String
```

## Stream a large file

Don't load gigabytes into memory — open the file and read in chunks:

```ruby
io = op.open("big.bin", "rb")
while (chunk = io.read(8192))
  # process chunk (String)
end
io.close
```

`OpenDal::IO` also supports `readline`, `readlines`, `seek`, `tell`, `rewind`,
and `eof?` for line- and position-oriented reads.

## Write a whole file

```ruby
op.write("path/to/file", "Hello, World!")
```

## Stream a large upload

Open the file in write mode and write incrementally, then `close` to commit:

```ruby
io = op.open("big.bin", "wb")
io.write(first_chunk)
io.write(second_chunk)
io.close
```

Writing does not support `seek`; an `OpenDal::IO` is either a reader or a writer.

## Check existence and metadata

```ruby
if op.exist?("path/to/file")
  meta = op.stat("path/to/file")
  puts "#{meta.content_length} bytes, dir = #{meta.dir?}"
end
```

`Metadata` also exposes `content_type`, `content_md5`, `etag`,
`content_disposition`, and the `file?` / `dir?` helpers.

## List a directory

`list` returns a `Lister`, which is `Enumerable` and yields `Entry` objects:

```ruby
op.list("dir/").each do |entry|
  puts "#{entry.path} (#{entry.metadata.mode})"
end
```

Because it is `Enumerable`, the usual methods work too:

```ruby
paths = op.list("dir/").map(&:path)
```

## Walk a tree recursively

```ruby
op.list("dir/", recursive: true).each do |entry|
  puts entry.path
end
```

`list` also accepts `limit:` (per-request max results) and `start_after:` (the
key to start listing from).

## Delete a file or a whole tree

```ruby
op.delete("path/to/file")     # single path
op.remove_all("dir/")         # a path and everything under it
```

## Create a directory

```ruby
op.create_dir("path/to/dir/")   # the trailing slash is required
```

## Copy and rename

```ruby
op.copy("from.txt", "to.txt")
op.rename("old.txt", "new.txt")
```

Both require a service that supports them; see
[capability checks](./05-production.md#capability-checks).

## Inspect the operator

```ruby
info = op.info
puts info.scheme    # e.g. "s3"
puts info.root      # e.g. "/"
puts info.name      # e.g. the bucket or container name
```
