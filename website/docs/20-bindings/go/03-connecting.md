---
title: Connecting to your storage
sidebar_label: Connecting to your storage
description: Build an OpenDAL operator in Go for any backend — scheme packages, configuration, and credentials.
---

# Connecting to your storage

Switching backends is a configuration change, not a code change. For the
configuration keys of a specific service, see [Services](/services).

## Add the scheme package

Each service is a separate companion module under
`github.com/apache/opendal-go-services/<scheme>`. Add the ones you use, then
import the package to get its `Scheme` value:

```shell
go get github.com/apache/opendal-go-services/s3
```

```go
import (
	"github.com/apache/opendal-go-services/s3"
	opendal "github.com/apache/opendal/bindings/go"
)
```

Forgetting to import the scheme package is the most common reason a service is
unavailable at build time.

## Build an operator

`NewOperator` takes a `Scheme` and an `OperatorOptions` map of configuration
keys. The keys are the same as each service's configuration fields:

```go
op, err := opendal.NewOperator(s3.Scheme, opendal.OperatorOptions{
	"bucket":   "my-bucket",
	"region":   "us-east-1",
	"endpoint": "https://s3.amazonaws.com",
})
if err != nil {
	log.Fatal(err)
}
defer op.Close()
```

A local filesystem operator roots every path under one directory:

```go
import "github.com/apache/opendal-go-services/fs"

op, err := opendal.NewOperator(fs.Scheme, opendal.OperatorOptions{
	"root": "/tmp/opendal",
})
```

Always `defer op.Close()` after a successful `NewOperator` — the operator holds
native resources that must be released.

## Credentials

Credentials are just more configuration keys. Set them in `OperatorOptions` —
for example S3's `access_key_id` and `secret_access_key`:

```go
op, err := opendal.NewOperator(s3.Scheme, opendal.OperatorOptions{
	"bucket":            "my-bucket",
	"region":            "us-east-1",
	"access_key_id":     "...",
	"secret_access_key": "...",
})
```

Some services also load credentials from their platform's default sources. See
each service's page under [Services](/services) for the exact keys and
credential behavior.

Avoid hard-coding secrets in source. Read them from the environment or a secret
manager and pass them in when you build the operator:

```go
import "os"

op, err := opendal.NewOperator(s3.Scheme, opendal.OperatorOptions{
	"bucket":            "my-bucket",
	"region":            "us-east-1",
	"access_key_id":     os.Getenv("AWS_ACCESS_KEY_ID"),
	"secret_access_key": os.Getenv("AWS_SECRET_ACCESS_KEY"),
})
```

## Verify the connection

`Check` performs a health check against the backend, so you can fail fast if the
configuration or credentials are wrong:

```go
if err := op.Check(); err != nil {
	log.Fatalf("operator is not reachable: %v", err)
}
```

## One operator per service and root

An operator maps to one service and one root path. To work with two buckets or
two roots, build two operators — each is an independent handle. Remember to
`Close` each one when you are done with it.
