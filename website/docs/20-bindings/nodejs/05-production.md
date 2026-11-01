---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in Node.js — retries, timeouts, throttling, concurrency limits, error handling, and observability.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, bound its resource use, observe what happens, and react to errors.
OpenDAL handles the first three with **layers** and gives you **capability
checks** for the rest.

## Layers

A layer wraps an operator to add cross-cutting behavior without touching your
storage code. Construct a layer, configure it, then apply it with
`op.layer(layer.build())`, which returns an operator with the layer applied:

```javascript
import { Operator, RetryLayer } from "opendal";

const retry = new RetryLayer();
retry.maxTimes = 3;
retry.jitter = true;

// layer() returns a NEW operator; capture it. It does not mutate in place.
const op = new Operator("s3", { bucket: "my-bucket", region: "us-east-1" })
  .layer(retry.build());
```

The Node.js binding exposes these layers:

| Layer | What it does |
|-------|--------------|
| `RetryLayer` | Retries temporary failures with exponential backoff (`maxTimes`, `jitter`, `factor`, `minDelay`, `maxDelay`). |
| `TimeoutLayer` | Bounds slow operations (`timeout` for non-IO ops, `ioTimeout` for IO ops; both in ms). |
| `ConcurrentLimitLayer` | Caps concurrent operations (`new ConcurrentLimitLayer(permits)`, plus `httpPermits`). |
| `ThrottleLayer` | Rate-limits bandwidth (`new ThrottleLayer(bandwidth, burst)`). |
| `LoggingLayer` | Logs every operation through Rust's `log` facade. |

## Combining layers

Apply layers one after another. A robust setup wraps logging on the outside and
retry on the inside, so each retry attempt is logged:

```javascript
import { Operator, LoggingLayer, TimeoutLayer, RetryLayer } from "opendal";

const logging = new LoggingLayer();

const timeout = new TimeoutLayer();
timeout.timeout = 30000;    // 30s for non-IO ops (ms)
timeout.ioTimeout = 10000;  // 10s for IO ops (ms)

const retry = new RetryLayer();
retry.maxTimes = 3;
retry.jitter = true;

// Each layer() returns a new operator; chain them. The last applied is the
// outermost, so logging wraps timeout wraps retry — every retry is logged.
const op = new Operator("s3", { bucket: "my-bucket", region: "us-east-1" })
  .layer(retry.build())
  .layer(timeout.build())
  .layer(logging.build());
```

## Observability

`LoggingLayer` emits structured logs through Rust's `log` facade. Enable output
with the `RUST_LOG` environment variable:

```bash
# Show debug logs
RUST_LOG=debug node app.js

# Show only OpenDAL service logs
RUST_LOG=opendal::services=debug node app.js
```

## Error handling

Operations reject (async) or throw (sync) with a standard `Error`. The OpenDAL
error kind appears in `error.message` — branch on it instead of assuming a call
succeeded:

```javascript
try {
  const bs = await op.read("maybe-missing.txt");
  // ...
} catch (err) {
  if (err.message.includes("NotFound")) {
    // handle absence
  } else {
    throw err;
  }
}
```

Common kinds that appear in messages include `NotFound`, `PermissionDenied`,
`AlreadyExists`, `ConditionNotMatch`, `RateLimited`, and `Unsupported`. Note that
`delete` is idempotent — deleting a missing path succeeds rather than failing.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do with
`capability()` before calling optional operations like `copy`, `rename`, or
presign:

```javascript
const cap = op.capability();
if (cap.copy) {
  await op.copy("a.txt", "b.txt");
}
```

`Capability` exposes boolean getters such as `read`, `write`, `copy`, `rename`,
`list`, and `presign`. Calling an unsupported operation throws an error, so
capability checks are an optimization, not a requirement for safety.
</content>
