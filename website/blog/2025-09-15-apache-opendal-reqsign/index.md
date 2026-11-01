---
title: "reqsign is now Apache OpenDAL Reqsign"
date: 2025-09-15
tags: [announcement]
authors: xuanwo
---

I’m happy to announce that my personal project has been donated to the ASF and accepted by the Apache OpenDAL PMC as a subproject: [Apache OpenDAL Reqsign](https://github.com/apache/opendal-reqsign). This means Reqsign will now be governed by the Apache OpenDAL PMC under [the Apache Way](https://www.apache.org/theapacheway/).

## What is Reqsign?

<!--truncate-->

Reqsign stands for `Request Signing`. Its goal is to make signing API requests simple. Many APIs look straightforward, but they quickly become complicated once hidden behind complex abstractions.

One major challenge is authentication. For example, AWS V4 requires users to build a signature from the request and sign it using HMAC with a secret key. Another challenge is credential loading. Cloud providers like AWS, Azure, and GCP support various types of credentials and loading methods to balance security and ease of use.

All these features together make simple APIs feel complex. To send a single `GetObject` call, you often need to rely on many AWS crates and go through deep abstraction layers.

Reqsign's goal is to bring simplicity back: build, sign, send.

In general, Reqsign provides three main parts for different services:

- `ProvideCredential`: Lets users provide credentials in different ways such as environment variables, profiles, `IMDS`, `OIDC`, and more. Reqsign also provides a default chain for every service so it works out of the box without extra configuration.
- `SignRequest`: Implements the signing logic so that requests are correctly signed with the given provider credentials.
- `Context`: Offers a pluggable context mechanism that supports HTTP send, command execution, and environment access. This allows users to configure the runtime context as they wish and makes Reqsign available in WASM.

For example:

```rust
use anyhow::Result;
use reqsign::aws;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a default signer for S3 in us-east-1
    // 
    // This will automatically:
    // - Load credentials from environment variables, config files, or IAM roles
    // - Set up the default HTTP client and file reader
    let signer = aws::default_signer("s3", "us-east-1");
    
    // Build your request
    let mut req = http::Request::builder()
        .method("GET")
        .uri("https://s3.amazonaws.com/testbucket")
        .body(())
        .unwrap()
        .into_parts()
        .0;
    
    // Sign the request
    signer.sign(&mut req, None).await?;
    
    // Send the request with your preferred HTTP client
    println!("Request has been signed!");
    Ok(())
}
```

Currently, Reqsign supports Aliyun OSS, AWS V4, Azure Storage, Google, Huawei Cloud OBS, Oracle, and Tencent COS. The community is working on adding more services so that users do not need to reimplement them.

## What has changed and what has not?

After this donation, Reqsign has become a subproject of Apache OpenDAL. The Apache OpenDAL PMC will govern the project under the Apache Way. The repository has been transferred from `Xuanwo/reqsign` to `apache/opendal-reqsign`. All Reqsign committers are now OpenDAL committers.

Reqsign is licensed under Apache 2.0. Joining the ASF strengthens this license and makes it permanent. Everyone can now use Reqsign in a safe and trusted way. The community will dedicate more effort to fixing bugs and adding new features to help Reqsign grow into a mature project.

## What’s next?

The OpenDAL community is working to make Reqsign ready as a mature ASF project and integrate it with Reqsign v1.0. We also plan to move some existing service implementations into Reqsign for better maintainability.

Welcome to join us!
