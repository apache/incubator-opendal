- Proposal Name: `operation_extension`
- Start Date: 2023-03-23
- RFC PR: [apache/opendal#1735](https://github.com/apache/opendal/pull/1735)
- Tracking Issue: [apache/opendal#1738](https://github.com/apache/opendal/issues/1738)

# Summary

Extend operation capabilities to support additional native features.

# Motivation

OpenDAL only supports a limited set of capabilities for operations.

- `read`/`stat`: only supports `range`
- `write`: only supports `content_type` and `content_disposition`

Our community has a strong need for more additional native features. For example:

- [opendal#892](https://github.com/apache/opendal/issues/892) wants [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control): Allow users to specify the cache control headers for the uploaded files.
- [opendal#825](https://github.com/apache/opendal/issues/825) wants [If-Match](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/If-Match) and [If-None-Match](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/If-None-Match): Allow users to makes a request conditional.
- [opendal#1726](https://github.com/apache/opendal/issues/1726) wants [response-content-disposition](https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html): Allow users to specify the content disposition for the downloaded files.

All of these feature requests are essentially asking for the same thing: the capability to define supplementary arguments for operations, particularly about HTTP services.

# Guide-level explanation

In this RFC, we will allow users to specify the standard HTTP headers like cache_control/if_match:

```rust
let op = OpRead::default().
    with_cache_control("max-age=3600").
    with_if_match("\"bfc13a64729c4290ef5b2c2730249c88ca92d82d\"");
let bs = o.read_with(op).await?;
```

Also, we will support some non-standard but widely used features like `response-content-disposition`:

```rust
let op = OpRead::default().
    with_override_content_disposition("attachment; filename=\"foo.txt\"");
let req = op.presign_read_with("filename", Duration::hours(1), op)?;
```

And, finally, we will support allow specify the default headers for services. Take `s3`'s `storage_class` as an example:

```rust
let mut builder = S3::default();
builder.default_storage_class("STANDARD_IA");
builder.default_cache_control("max-age=3600");
```

In general, we will support the following features:

- Allow users to specify the (non-)standard HTTP headers for operations.
- Allow users to specify the default HTTP headers for services.

# Reference-level explanation

We will make the following changes in OpenDAL:

For `OpRead` & `OpStat`:

```diff
pub struct OpRead {
    br: BytesRange,
+   cache_control: Option<String>,
+   if_match: Option<String>,
+   if_none_match: Option<String>,
+   override_content_disposition: Option<String>,
}
```

For `OpWrite`:

```diff
pub struct OpWrite {
    append: bool,

    content_type: Option<String>,
    content_disposition: Option<String>,
+   cache_control: Option<String>,
}
```

We will provide different default options for each service. For example, we can add `default_storage_class` for `s3`.


# Drawbacks

None

# Rationale and alternatives

## Why using `override_content_disposition` instead of `response_content_disposition`?

`response_content_disposition` is not a part of HTTP standard, it's the private API provided by `s3`.

- `azblob` will use `x-ms-blob-content-disposition` header
- `ocios` will use `httpResponseContentDisposition` query

OpenDAL does not accept the query as is. Instead, we have created a more readable name `override_content_disposition` to clarify its purpose.

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Strict Mode

Additionally, we have implemented a `strict` option for the `Operator`. If users enable this option, OpenDAL will return an error message for unsupported options. Otherwise, it will ignore them.

For instance, if users rely on the `if_match` behavior but services like `fs` and `hdfs` do not support it natively, they can use the `op.with_strict()` function to prompt OpenDAL to return an error.
