<p align="center">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="website/static/img/logo_dark.svg">
  <img alt="Apache OpenDAL(incubating)" src="website/static/img/logo.svg" width="300px">
</picture>
</p>

---

OpenDAL is a data access layer that allows users to easily and efficiently retrieve data from various storage services in a unified way.

![](https://user-images.githubusercontent.com/5351546/222356748-14276998-501b-4d2a-9b09-b8cff3018204.png)


Major components of the project include:

**Libraries**

- [Rust Core](core/README.md)
- [Node.js Binding](bindings/nodejs/README.md)
- [Python Binding](bindings/python/README.md)
- [C Binding](bindings/c) *working on*
- [Java Binding](bindings/java) *working on*
- [Ruby Binding](bindings/ruby) *working on*

**Applications**

- [oli](bin/oli): OpenDAL Command Line Interface
- [oay](bin/oay): OpenDAL Gateway

## Documentation

The documentation is available at <https://opendal.apache.org>.

We are engaged in a practice we call "documentation as code". You can also view the documentation directly in project's source code. And we welcome you to contribute to the documentation.

## Contribute

OpenDAL is an active open-source project. We are always open to people who want to use it or contribute to it. Here are some ways to go.

- Start with [Contributing Guide](CONTRIBUTING.md).
- Submit [Issues](https://github.com/apache/incubator-opendal/issues/new) for bug report or feature requests.
- Asking questions in the [Discussions](https://github.com/apache/incubator-opendal/discussions/new?category=q-a).
- Talk to community directly at [Discord](https://discord.gg/XQy8yGR2dg).
- [Subscribe our dev mailing list](mailto:dev-subscribe@opendal.apache.org), then you can use it to ask questions, discuss design and implementation, etc. View the archive at <https://lists.apache.org/list.html?dev@opendal.apache.org>.

## Who is using OpenDAL?

- [Databend](https://github.com/datafuselabs/databend/): A modern Elasticity and Performance cloud data warehouse.
- [GreptimeDB](https://github.com/GreptimeTeam/greptimedb): An open-source, cloud-native, distributed time-series database.
- [deepeth/mars](https://github.com/deepeth/mars): The powerful analysis platform to explore and visualize data from blockchain.
- [mozilla/sccache](https://github.com/mozilla/sccache/): `sccache` is [`ccache`](https://github.com/ccache/ccache) with cloud storage
- [RisingWave](https://github.com/risingwavelabs/risingwave): A Distributed SQL Database for Stream Processing
- [Vector](https://github.com/vectordotdev/vector): A high-performance observability data pipeline.

## License

Licensed under the [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
