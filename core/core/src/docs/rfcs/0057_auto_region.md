- Proposal Name: `auto_region`
- Start Date: 2022-02-24
- RFC PR: [apache/opendal#57](https://github.com/apache/opendal/pull/57)
- Tracking Issue: [apache/opendal#58](https://github.com/apache/opendal/issues/58)

# Summary

Automatically detecting user's s3 region.

# Motivation

Current behavior for `region` and `endpoint` is buggy. `endpoint=https://s3.amazonaws.com` and `endpoint=""` are expected to be the same, because `endpoint=""` means take the default value `https://s3.amazonaws.com`. However, they aren't.

S3 SDK has a mechanism to construct the correct API endpoint. It works like `format!("s3.{}.amazonaws.com", region)` internally. But if we specify the endpoint to `https://s3.amazonaws.com`, SDK will take this endpoint static.

So users could meet errors like:

```shell
attempting to access must be addressed using the specified endpoint
```

Automatically detecting the user's s3 region will help resolve this problem. Users don't need to care about the region anymore, `OpenDAL` will figure it out. Everything works regardless of whether the input is `s3.amazonaws.com` or `s3.us-east-1.amazonaws.com`.

# Guide-level explanation

`OpenDAL` will remove `region` option, and users only need to set the `endpoint` now.

Valid input including:

- `https://s3.amazonaws.com`
- `https://s3.us-east-1.amazonaws.com`
- `https://oss-ap-northeast-1.aliyuncs.com`
- `http://127.0.0.1:9000`

`OpenDAL` will handle the `region` internally and automatically.

# Reference-level explanation

S3 services support mechanism to indicate the correct region on itself.

Sending a `HEAD` request to `<endpoint>/<bucket>` will get a response like:

```shell
:) curl -I https://s3.amazonaws.com/databend-shared
HTTP/1.1 301 Moved Permanently
x-amz-bucket-region: us-east-2
x-amz-request-id: NPYSWK7WXJD1KQG7
x-amz-id-2: 3FJSJ5HACKqLbeeXBUUE3GoPL1IGDjLl6SZx/fw2MS+k0GND0UwDib5YQXE6CThiQxpYBWZjgxs=
Content-Type: application/xml
Date: Thu, 24 Feb 2022 05:15:13 GMT
Server: AmazonS3
```

`x-amz-bucket-region: us-east-2` will be returned, and we can use this region to construct the correct endpoint for this bucket:

```shell
:) curl -I https://s3.us-east-2.amazonaws.com/databend-shared
HTTP/1.1 403 Forbidden
x-amz-bucket-region: us-east-2
x-amz-request-id: 98CN5MYV3GQ1XMPY
x-amz-id-2: Tdxy36bRRP21Oip18KMQ7FG63MTeXOpXdd5/N3izFH0oalPODVaRlpCkDU3oUN0HIE24/ezX5Dc=
Content-Type: application/xml
Date: Thu, 24 Feb 2022 05:16:57 GMT
Server: AmazonS3
```

It also works for S3 compilable services like minio:

```shell
# Start minio with `MINIO_SITE_REGION` configured
:) MINIO_SITE_REGION=test minio server .
# Sending request to minio bucket
:) curl -I 127.0.0.1:9900/databend
HTTP/1.1 403 Forbidden
Accept-Ranges: bytes
Content-Length: 0
Content-Security-Policy: block-all-mixed-content
Server: MinIO
Strict-Transport-Security: max-age=31536000; includeSubDomains
Vary: Origin
Vary: Accept-Encoding
X-Amz-Bucket-Region: test
X-Amz-Request-Id: 16D6A12DCA57E0FA
X-Content-Type-Options: nosniff
X-Xss-Protection: 1; mode=block
Date: Thu, 24 Feb 2022 05:18:51 GMT
```

We can use this mechanism to detect `region` automatically. The algorithm works as follows:

- If `endpoint` is empty, fill it will `https://s3.amazonaws.com` and the corresponding template: `https://s3.{region}.amazonaws.com`.
- Sending a `HEAD` request to `<endpoint>/<bucket>`.
- If got `200` or `403` response, the endpoint works.
  - Use this endpoint directly without filling the template.
  - Take the header `x-amz-bucket-region` as the region to fill the endpoint.
  - Use the fallback value `us-east-1` to make SDK happy if the header not exists.
- If got a `301` response, the endpoint needs construction.
  - Take the header `x-amz-bucket-region` as the region to fill the endpoint.
  - Return an error to the user if not exist.
- If got `404`, the bucket could not exist, or the endpoint is incorrect.
  - Return an error to the user.

# Drawbacks

None.

# Rationale and alternatives

## Use virtual style `<bucket>.<endpoint>`?

The virtual style works too. But not all services support this kind of API endpoint. For example, using `http://testbucket.127.0.0.1` is wrong, and we need to do extra checks.

Using `<endpoint>/<bucket>` makes everything easier.

## Use `ListBuckets` API?

`ListBuckets` requires higher permission than normal bucket read and write operations. It's better to finish the job without requesting more permission. 

## Misbehavior S3 Compilable Services

Many services didn't implement S3 API correctly.

Aliyun OSS will return `404` for every bucket:

```shell
:) curl -I https://aliyuncs.com/<my-existing-bucket>
HTTP/2 404
date: Thu, 24 Feb 2022 05:32:57 GMT
content-type: text/html
content-length: 690
ufe-result: A6
set-cookie: thw=cn; Path=/; Domain=.taobao.com; Expires=Fri, 24-Feb-23 05:32:57 GMT;
server: Tengine/Aserver
```

QingStor Object Storage will return `307` with the `Location` header:

```shell
:) curl -I https://s3.qingstor.com/community
HTTP/1.1 301 Moved Permanently
Server: nginx/1.13.6
Date: Thu, 24 Feb 2022 05:33:55 GMT
Connection: keep-alive
Location: https://pek3a.s3.qingstor.com/community
X-Qs-Request-Id: 05b83b615c801a3d
```

In this proposal, we will not figure them out. It's easier for the user to fill the correct endpoint instead of automatically detecting them.

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
