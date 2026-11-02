# Apache OpenDAL™ Python Binding

[![Status](https://img.shields.io/badge/status-released-blue)](https://pypi.org/project/opendal/)
[![PyPI](https://img.shields.io/pypi/v/opendal.svg?logo=PyPI)](https://pypi.org/project/opendal/)
[![Website](https://img.shields.io/badge/opendal-OpenDAL_Website-red?logo=Apache&logoColor=red)](https://opendal.apache.org/docs/bindings/python)

This package provides a native Python binding for **Apache OpenDAL™**, a data access
layer that allows you to access various storage services in a unified way.

![OpenDAL Architecture](https://opendal.apache.org/img/architectural.png)

We release the OpenDAL Python binding independently of the
[`opendal` crate](https://crates.io/crates/opendal) (Rust core). For updates
and compatibility, use the Python binding version instead of the `opendal` crate
version.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/python](https://opendal.apache.org/docs/bindings/python) — install, connect, common tasks, and going to production.
- **API reference**: [opendal.apache.org/docs/python](https://opendal.apache.org/docs/python/)
- **Services & configuration**: [opendal.apache.org/services](https://opendal.apache.org/services)
- **Upgrade guide**: [`upgrade.md`](./upgrade.md)
- **Examples**: [`docs/examples`](./docs/examples)

## Installation

Install from PyPI:

```bash
pip install opendal
```

Or install from [Anaconda main](https://anaconda.org/main/opendal):

```bash
conda install opendal
```

Or install from [conda-forge](https://anaconda.org/conda-forge/opendal):

```bash
conda install conda-forge::opendal
```

## Quickstart

```python
import opendal

# Configure a service, then build an operator from it.
op = opendal.Operator("fs", root="/tmp")

# The same verbs work on every service.
op.write("test.txt", b"Hello World")
print(op.read("test.txt"))
print(op.stat("test.txt").content_length)
```

To use a real backend, change the scheme and pass its configuration — the
operations stay identical:

```python
op = opendal.Operator("s3", bucket="your_bucket", region="your_region")
```

OpenDAL also has a first-class async API via `opendal.AsyncOperator`. See
[Getting started](https://opendal.apache.org/docs/bindings/python/getting-started)
and [Connecting to your storage](https://opendal.apache.org/docs/bindings/python/connecting)
for the full guide.

## Contributing

This project uses [`just`](https://github.com/casey/just) as a command runner.
For a complete guide on building, testing, and contributing, see
**[CONTRIBUTING.md](./CONTRIBUTING.md)**.

## Used By

Check out the [users list](./users.md) for more details on who is using OpenDAL.

## License and Trademarks

Licensed under the Apache License, Version 2.0:
http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of
the Apache Software Foundation.

### Third-party software

Compiled distributions include the following Mozilla Public License 2.0
components. Their source code is available from the linked project pages:

- `colored` 3.1.1 ([source](https://crates.io/crates/colored/3.1.1), [homepage](https://github.com/mackwic/colored)), licensed under MPL-2.0.
- `option-ext` 0.2.0 ([source](https://crates.io/crates/option-ext/0.2.0), [homepage](https://github.com/soc/option-ext)), licensed under MPL-2.0.
- `persy` 1.8.1 ([source](https://crates.io/crates/persy/1.8.1), [homepage](https://persy.rs)), licensed under MPL-2.0.

The distribution includes the MPL-2.0 text in `LICENSE-MPL-2.0.txt`.

Musllinux wheels also bundle `libgcc_s`, the [GCC runtime library](https://gcc.gnu.org/),
licensed under `GPL-3.0-or-later WITH GCC-exception-3.1`. The complete license and
exception texts are included in `LICENSE-libgcc.txt`.
