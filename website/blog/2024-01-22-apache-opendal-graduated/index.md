---
title: "Apache OpenDAL™ is now Graduated"
date: 2024-01-22
slug: apache-opendal-graduated
tags: [announcement]
authors: xuanwo
---

Hello, everyone! I'm happy to announce that [Apache OpenDAL™](https://opendal.apache.org/) has graduated from the [Apache Incubator](https://incubator.apache.org/) to become a Top-Level Project of [the Apache Software Foundation](https://apache.org/).

<!--truncate-->

## What's Apache OpenDAL?

**Apache OpenDAL** is a data access layer that allows users to easily and efficiently retrieve data from various storage services in a unified way. Our VISION is **access data freely**.

OpenDAL could be used as a **better** SDK for your storage services: A SDK with native integration of [retry](https://opendal.apache.org/docs/rust/opendal/layers/struct.RetryLayer.html), [logging](https://opendal.apache.org/docs/rust/opendal/layers/struct.LoggingLayer.html), [metrics](https://opendal.apache.org/docs/rust/opendal/layers/struct.MetricsLayer.html), [tracing](https://opendal.apache.org/docs/rust/opendal/layers/struct.TracingLayer.html), [timeout](https://opendal.apache.org/docs/rust/opendal/layers/struct.TimeoutLayer.html), [throttle](https://opendal.apache.org/docs/rust/opendal/layers/struct.ThrottleLayer.html), and [more](https://opendal.apache.org/docs/rust/opendal/layers/index.html).

OpenDAL could be used as a **super** connector for your storage services: A connector that supports all kinds of storage services from Object Storage (s3, gcs, azblob), File Storage (fs, azdls, hdfs), Consumer Cloud Storage (gdrive, onedrive), Key-Value Storage (rocksdb, sled) to Cache Storage (memcached, moka).

OpenDAL could be used as an **elegant** client for your storage services: A client with well designed API and  many language bindings: [Rust](https://crates.io/crates/opendal), C, Cpp, Dotnet, Go, Haskell, [Java](https://mvnrepository.com/artifact/org.apache.opendal/opendal-java), Lua, [Node.js](https://www.npmjs.com/package/opendal), OCaml, PHP, [Python](https://pypi.org/project/opendal/), Ruby, Swift and Zig.

Need to access data? Give OpenDAL a try!

```rust
async fn main() -> Result<()> {
    // Init s3 service.
    let mut builder = services::S3::default();
    builder.bucket("test");

    // Init an operator
    let op = Operator::via_map(builder)?
        // Add logging
        .layer(LoggingLayer::default())
        .finish();

    // Write data
    op.write("hello.txt", "Hello, World!").await?;

    // Read data
    let bs = op.read("hello.txt").await?;

    // Fetch metadata
    let meta = op.stat("hello.txt").await?;
    let mode = meta.mode();
    let length = meta.content_length();

    // Delete
    op.delete("hello.txt").await?;

    Ok(())
}
```

## What's the ASF?

The Apache Software Foundation (ASF) is a nonprofit corporation to support a number of open-source software projects. The Apache Software Foundation exists to provide software for the public good. We believe in the power of community over code, known as The Apache Way. Thousands of people around the world contribute to ASF open source projects every day. 

The OpenDAL Community believes [the apache way](https://www.apache.org/theapacheway/) that:

- *Earned Authority*: all individuals are given the opportunity to participate, but their influence is based on publicly earned merit – what they contribute to the community.
- *Community of Peers*: individuals participate at the ASF, not organizations.
- *Open Communications*: as a virtual organization, the ASF requires all communications related to code and decision-making to be publicly accessible to ensure asynchronous collaboration, as necessitated by a globally-distributed community.
- *Consensus Decision Making*: Apache Projects are overseen by a self-selected team of active volunteers who are contributing to their respective projects.
- *Responsible Oversight*: The ASF governance model is based on trust and delegated oversight.

The original creators [Databend](https://github.com/datafuselabs/databend/) chosen to contribute OpenDAL to the ASF, embracing the Apache way through [joining the incubator program](https://opendal.apache.org/blog/opendal-entered-apache-incubator).

## What's graduation?

In the [Apache Incubator](https://incubator.apache.org/), the OpenDAL community is learning the Apache Way through daily development activities, growing its community and producing Apache releases.

During the incubation, we:

- Consist of 19 committers, including mentors, with 12 serving as PPMC members.
- Boast 164 contributors.
- Made 9 releases—averaging at least one per month.
- Had 7 different release managers to date.
- Used by 10 known entities and is a dependency for 263 GitHub projects and 18 crates.io packages.
- Opened 1,200+ issues with 1,100+ successfully resolved.
- Submitted a total of 2,400+ PRs, most of them have been merged or closed.

The graduation signifies that the OpenDAL Community is recognized as a [mature](https://opendal.apache.org/community/maturity) community, which entails:

- CODE: OpenDAL is an [Apache 2.0 licensed](https://github.com/apache/opendal/blob/main/LICENSE) open-source project with [accessible, buildable code](https://github.com/apache/opendal) on GitHub, featuring [a traceable history and authenticated code provenance](https://github.com/apache/opendal/commits/main/). 
- LICENSE: OpenDAL maintains [open-source compliance](https://github.com/apache/opendal/blob/main/DEPENDENCIES.md) for all code and dependencies, requires contributor agreements, and clearly documents copyright ownership.
- Releases: OpenDAL offers standardized, committee-approved [source code releases](https://downloads.apache.org/opendal/) with secure signatures, provides convenience binaries, and has [a well-documented, repeatable release process](https://opendal.apache.org/community/committers/release).
- Quality: OpenDAL is committed to code quality transparency, prioritizes security with quick issue responses, ensures backward compatibility with clear documentation, and actively addresses bug reports in a timely manner.
- Community: OpenDAL offers [a comprehensive homepage](https://opendal.apache.org/), welcomes diverse contributions, promotes a meritocratic approach for active contributors, operates on community consensus, and ensures timely responses to user queries through various channels. 
- Consensus: OpenDAL has a [public list of key decision-makers](https://projects.apache.org/committee.html?opendal) and uses a consensus approach for decisions, documented on its [main communication channel](https://lists.apache.org/list.html?dev@opendal.apache.org). It follows standard voting rules and records all important discussions in writing.
- Independence: OpenDAL is independent, with contributors from various companies acting on their own, not as representatives of any organization.

## What's next?

After graduation, OpenDAL Community will continue to focus on the following aspects under the VISION: **access data freely**.

### More Stable Services

OpenDAL now supports 59 services, although only some of them are *stable*.

*stable* for OpenDAL means that

- Have integration tests covered.
- Have at least one production user.

The *stable* service established a feedback loop between the OpenDAL community and its users. Users can submit bug reports or feature requests to the OpenDAL community, which in turn can enhance the service using this feedback while ensuring existing features remain intact.

After graduation, the OpenDAL community will focus on improving the stability of current services instead of just expanding our offerings.

We plan to:

- Add features users wanted to services like [file version](https://github.com/apache/opendal/issues/2611), [concurrently list](https://github.com/apache/opendal/issues/3977) and [glob pattern](https://github.com/apache/opendal/issues/1251).
- Add integration tests for newly added services.

### More Useful Documents

OpenDAL have good docs for its rust core, but not for other language bindings.

The lack of comprehensive documentation makes OpenDAL challenging for users to operate in Java or Python. Without user feedback, the community is unable to enhance this documentation, leading to a detrimental cycle that must be broken.

After graduation, the OpenDAL community will improve the documentation of other language bindings.

We plan to:

- Introduce code generation to automatically create documentation for the service builder due to its numerous configurations.
- Add more API Docs and examples for other language bindings.

OpenDAL have good docs for its public API, but not for its internal design.

OpenDAL is proud of its elegant design, but it is not well documented. This makes it difficult for new contributors to understand the codebase and make contributions.

After graduation, the OpenDAL community will improve the documentation of its internal design.

We plan to:

- Optimize the codebase to make it easier to understand.
- Add more blog posts to explain the design of OpenDAL.

### More Production Users

OpenDAL requires more production users, as they are vital to the success of our project. Increased user production leads to more valuable feedback, a more engaged contributor base, and a stronger community. We've started the initial loop; let's expand it!

After graduation, the OpenDAL community will focus on attracting more production users.

We plan to:

- Optimize the feature set for adoption like [uri initiation](https://github.com/apache/opendal/issues/3022) and [config](https://github.com/apache/opendal/issues/3240).
- Expand more ways to use OpenDAL via [fuse](https://github.com/apache/opendal/tree/main/bin/ofs), [cli](https://github.com/apache/opendal/tree/main/bin/oli), [S3/WebDAV API](https://github.com/apache/opendal/tree/main/bin/oli), [object_store binding](https://github.com/apache/opendal/tree/main/integrations/object_store).

## Conclusion

The OpenDAL Community aims to create a world where users can freely access data across any storage service in any manner they choose. Graduation is just the beginning—let's work together to make our VISION a reality!
