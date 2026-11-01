# Apache OpenDAL™ Node.js Binding

[![](https://img.shields.io/badge/status-released-blue)](https://www.npmjs.com/package/opendal)
[![npm](https://img.shields.io/npm/v/opendal.svg?logo=npm)](https://www.npmjs.com/package/opendal)
[![Website](https://img.shields.io/badge/opendal-OpenDAL_Website-red?logo=Apache&logoColor=red)](https://opendal.apache.org/docs/nodejs/)

![OpenDAL Architecture](https://opendal.apache.org/img/architectural.png)

A native Node.js binding for Apache OpenDAL: access S3, GCS, Azure Blob, HDFS, the local filesystem, and 50+ more services through one API.

We release the OpenDAL Node.js binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Node.js binding version instead of the `opendal`
crate version.

## Useful Links

- [User guide](https://opendal.apache.org/docs/bindings/nodejs)
- [API documentation](https://opendal.apache.org/docs/nodejs/)
- [Services & configuration](https://opendal.apache.org/services)
- [Upgrade Guide](./upgrade.md)

## Installation

```shell
npm install opendal
```

## Quickstart

```javascript
import { Operator } from "opendal";

async function main() {
  const op = new Operator("fs", { root: "/tmp" });
  await op.write("test", "Hello, World!");
  const bs = await op.read("test");
  console.log(new TextDecoder().decode(bs));
  const meta = await op.stat("test");
  console.log(`contentLength: ${meta.contentLength}`);
}

main();
```

The API is promise-based; every operation has a synchronous `*Sync` variant too.
For configuring backends, streaming, layers (retry, timeout, logging, throttle,
concurrency limits), presigned URLs, and Next.js usage, see the full
[user guide](https://opendal.apache.org/docs/bindings/nodejs).

## Contributing

- Start with the [Contributing Guide](CONTRIBUTING.md).
- Submit [Issues](https://github.com/apache/opendal/issues/new) for bug reports or feature requests.
- Ask questions in the [Discussions](https://github.com/apache/opendal/discussions/new?category=q-a).
- Talk to the community on [Discord](https://opendal.apache.org/discord).

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.

### Third-party software

Compiled distributions include the following Mozilla Public License 2.0
components. Their source code is available from the linked project pages:

- `colored` 3.1.1 ([source](https://crates.io/crates/colored/3.1.1), [homepage](https://github.com/mackwic/colored)), licensed under MPL-2.0.
- `option-ext` 0.2.0 ([source](https://crates.io/crates/option-ext/0.2.0), [homepage](https://github.com/soc/option-ext)), licensed under MPL-2.0.
- `persy` 1.8.1 ([source](https://crates.io/crates/persy/1.8.1), [homepage](https://persy.rs)), licensed under MPL-2.0.

The distribution includes the MPL-2.0 text in `LICENSE-MPL-2.0.txt`.
</content>
