---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in Go — retries, timeouts, error handling, and capability checks.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, bound its resource use, handle errors precisely, and adapt to what a
backend supports.

## Layers

In the Go binding, layers are configured when you build the operator by passing
options to `NewOperator` after the `OperatorOptions` map. Two layers are
exposed: retry and timeout.

```go
import "time"

op, err := opendal.NewOperator(
	s3.Scheme,
	opendal.OperatorOptions{"bucket": "my-bucket", "region": "us-east-1"},
	opendal.WithTimeout(10*time.Second, 60*time.Second),
	opendal.WithRetry(opendal.RetryMaxTimes(3)),
)
if err != nil {
	log.Fatal(err)
}
defer op.Close()
```

### Retry

`WithRetry` retries operations that fail with temporary errors, using
exponential backoff. Tune it with these options:

| Option | What it does |
|--------|--------------|
| `RetryMaxTimes(n)` | Maximum number of retry attempts. |
| `RetryFactor(f)` | Backoff multiplier between attempts (must be `>= 1`). |
| `RetryMinDelay(d)` | Minimum delay before the first retry. |
| `RetryMaxDelay(d)` | Cap on the delay between retries. |
| `RetryJitter()` | Adds randomness to the backoff to avoid thundering herds. |

```go
opendal.WithRetry(
	opendal.RetryMaxTimes(5),
	opendal.RetryMinDelay(200*time.Millisecond),
	opendal.RetryJitter(),
)
```

### Timeout

`WithTimeout(timeout, ioTimeout)` bounds slow calls. The first argument applies
to non-IO operations; the second applies to read, write, list, and streaming IO.
Pass `WithTimeout` before `WithRetry` so each retry attempt has its own timeout.

## Error handling

Every fallible call returns a Go `error`. When the failure comes from the
storage layer, the concrete type is `*opendal.Error`. Type-assert it and branch
on `Code()` instead of parsing messages:

```go
data, err := op.Read("maybe-missing.txt")
if err != nil {
	var oerr *opendal.Error
	if errors.As(err, &oerr) && oerr.Code() == opendal.CodeNotFound {
		// handle absence
	} else {
		log.Fatal(err)
	}
}
```

Common codes include `CodeNotFound`, `CodePermissioDenied`, `CodeAlreadyExists`,
`CodeRateLimited`, `CodeConditionNotMatch`, and `CodeUnsupported`. Note that
`Delete` is idempotent — deleting a missing path succeeds rather than returning
`CodeNotFound`.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do through
`Info().GetCapability()` before calling optional operations like `Copy`,
`Rename`, or presign:

```go
cap := op.Info().GetCapability()
if cap.Copy() {
	if err := op.Copy("a.txt", "b.txt"); err != nil {
		log.Fatal(err)
	}
}
```

Calling an unsupported operation returns an error with `CodeUnsupported`, so
capability checks are an optimization, not a requirement for safety.

`OperatorInfo` also reports the backend's scheme, root, and name via
`GetScheme()`, `GetRoot()`, and `GetName()`.
