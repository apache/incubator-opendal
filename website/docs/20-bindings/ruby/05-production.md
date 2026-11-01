---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in Ruby — middlewares, error handling, and capability checks.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, bound its resource use, handle errors precisely, and adapt to what a
backend supports.

## Middlewares

A middleware wraps an operator to add cross-cutting behavior — these are
OpenDAL's [layers](../../03-concepts.mdx#layer), exposed in Ruby as
`OpenDal::Middleware`. Apply one with `op.middleware(...)`, which returns a new
operator with the middleware applied:

```ruby
require "opendal"

op = OpenDal::Operator.new("s3", {"bucket" => "my-bucket", "region" => "us-east-1"})

op = op.middleware(OpenDal::Middleware::ConcurrentLimit.new(5))
op = op.middleware(OpenDal::Middleware::Retry.new)
op = op.middleware(OpenDal::Middleware::Timeout.new(10, 5))
```

The Ruby binding exposes these middlewares:

| Middleware | What it does |
|------------|--------------|
| `OpenDal::Middleware::Retry` | Retries operations that fail with temporary errors. `new` takes no arguments. |
| `OpenDal::Middleware::ConcurrentLimit` | Caps the number of concurrent requests. `new(permits)`. |
| `OpenDal::Middleware::Throttle` | Limits bandwidth. `new(bandwidth, burst)`. |
| `OpenDal::Middleware::Timeout` | Aborts slow operations. `new(timeout, io_timeout)`, in seconds. |

## Error handling

Failed operations raise `RuntimeError` with a message describing what went
wrong. Rescue it around calls that may fail, for example reading a path that may
not exist:

```ruby
begin
  data = op.read("maybe-missing.txt")
rescue RuntimeError => e
  warn "read failed: #{e.message}"
  data = nil
end
```

Invalid arguments raise the standard Ruby exceptions: `op.open` with a bad mode
raises `ArgumentError`, and reading past the end with `readline` raises
`EOFError`.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do with
`capability` before calling optional operations like `copy` or `rename`:

```ruby
cap = op.capability
if cap.copy
  op.copy("a.txt", "b.txt")
end
```

`Capability` exposes a boolean (or value) per operation, including `read`,
`write`, `stat`, `delete`, `create_dir`, `copy`, `rename`, `list`, and the
`list_with_recursive` / `write_can_append` style fine-grained flags. The same
capability is available from `op.info.capability`.
