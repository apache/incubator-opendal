---
title: Vision
sidebar_label: Vision
---

## Charter

**One Layer, All Storage.**

## Principles

Principles are guiding principles. They guide how decisions are made for the whole project. Ideally, we do all of them all the time. In some cases, though, we may be forced to decide between slightly penalizing one goal or another. In that case, we tend to support those goals that come earlier in the list over those that come later (but every case is different).

### 0. Open Community

OpenDAL SHOULD be an **open** storage library.

OpenDAL is an ASF project governed by the OpenDAL PMC. At ASF, we believe in "Community Over Code" and adhere to [the Apache Way](https://www.apache.org/theapacheway/). We aim to develop OpenDAL to meet the needs of our community. We do not maintain private versions or include features that aren't useful to others.

For example, OpenDAL prefers to have clear and readable code, as this allows more people in the community to join the development.

### 1. Solid Foundation

OpenDAL SHOULD be a **solid** storage library.

OpenDAL is a solid foundation of user projects that users can trust OpenDAL to perform operations on real-world storage services. OpenDAL SHOULD always focus on building a Solid Foundation.

For example, OpenDAL performs additional error checks for AWS S3 complete multipart operations, as S3 may return an error in response with a 200 status code, even though this may add extra costs that conflict with "Fast Access.”

### 2. Fast Access

OpenDAL SHOULD be a **fast** storage library.

Its fast access ensures that OpenDAL implements storage support with zero overhead. Users can integrate with OpenDAL without concerns about additional costs. OpenDAL should be as fast as, or faster than, the SDK for any given storage service.

For example, OpenDAL uses Capability to describe the capabilities of different services and adopts native features of those services whenever possible.

### 3. Object Storage First

OpenDAL SHOULD be an **object storage first** library.

OpenDAL supports many storage services, but it is usually optimized for modern storage services. At the time of writing, we can say OpenDAL is object storage first. When designing features, OpenDAL tends to prioritize optimization for object storage.

For example, OpenDAL's Buffer design is primarily optimized for HTTP-based services, helping to reduce extra allocation, in-memory copying, and memory usage.

### 4. Extensible Architecture

OpenDAL SHOULD be an extensible storage library.

OpenDAL can be extended to various languages, backends, and layers. Each is independent and does not depend on the others. Users can combine different layers, such as metrics, logging, tracing, and retry, and extend their own languages, backends, and layers.

For example, OpenDAL's core never relies on the behavior or dependency of a single layer. Users can stack as many layers as they want on a given operator.

## Use Cases

Who are typical OpenDAL *users*? How would they use OpenDAL?

### Infrastructure Builders

Examples:

- [Databend](https://github.com/databendlabs/databend)
- [RisingWave](https://github.com/risingwavelabs/risingwave)
- [GreptimeDB](https://github.com/GreptimeTeam/greptimedb)
- [Apache Iceberg Rust](https://github.com/apache/iceberg-rust)

Use Cases:

- Building storage systems like databases
- Developing data processing pipelines
- Creating backup and archive solutions

Primary Concerns:

- **Solid Foundation**: Need guaranteed consistency and predictability for storage operations
- **Fast Access**: Require minimal overhead and optimal performance
- *Why*: Infrastructure services demand both reliability and performance as foundational requirements

### Application Developers

Examples:

- [Sccache](https://github.com/mozilla/sccache)
- [Vector](https://github.com/vectordotdev/vector)
- [Rustic](https://github.com/rustic-rs/rustic)

Use Cases:

- Building end-user applications
- Developing CLI tools
- Creating web services

Primary Concerns:

- **Fast Access**: Need efficient integration and optimal performance
- **Object Storage First**: Benefit from optimizations for modern cloud storage
- *Why*: Modern applications commonly use object storage and require responsive performance

### Platform Developers

Examples:

- [Pants](https://github.com/pantsbuild/pants)
- [Zino](https://github.com/zino-rs/zino)
- [Shuttle](https://github.com/shuttle-hq/shuttle)

Use Cases:

- Building AI/ML platforms
- Developing cloud services
- Creating developer tools

Primary Concerns:

- **Extensible Architecture**: Need to customize and extend storage capabilities
- **Solid Foundation**: Require dependable storage operations
- *Why*: Platforms need flexibility to adapt to various use cases while maintaining reliability

---

*This documentation is inspired a lot by [hyper’s VISION document](https://hyper.rs/contrib/vision/).*
