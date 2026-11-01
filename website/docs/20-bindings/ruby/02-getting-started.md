---
title: Getting started
sidebar_label: Getting started
description: Run your first OpenDAL program in Ruby, then point it at a real storage backend.
---

# Getting started

## Your first program

This program builds an operator, writes a file, reads it back, inspects its
metadata, and deletes it. It runs against the in-memory service with no
credentials, so you can run it right after installing.

```shell
gem install opendal
```

```ruby file=bindings/ruby/examples/getting_started.rb region=quickstart
```

`write` takes a `String`, and `read` returns a `String`.

## Point it at a real backend

Only the service changes; the operations stay identical. The scheme is the first
argument, and configuration is a `Hash` of string keys and values:

```ruby
require "opendal"

op = OpenDal::Operator.new("s3", {
  "bucket" => "my-bucket",
  "region" => "us-east-1",
})

op.write("hello.txt", "Hello from S3!")
puts op.read("hello.txt")
```

The gem bundles every service, so there is nothing extra to install. The next
page, [Connecting to your storage](./03-connecting.md), covers configuration and
credentials in depth; [Services](/services) lists every backend and its keys.

## Synchronous by design

The Ruby binding is synchronous: every operation runs on the calling thread and
returns when it completes. There is no async operator and nothing to `await` —
call the methods directly as shown above.
