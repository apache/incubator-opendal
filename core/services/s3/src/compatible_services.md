
## Compatible Services

### AWS S3

[AWS S3](https://aws.amazon.com/s3/) is the default implementations of s3 services. Only `bucket` is required.

```rust,ignore
builder.bucket("<bucket_name>");
```

### Alibaba Object Storage Service (OSS)

[OSS](https://www.alibabacloud.com/product/object-storage-service) is a s3 compatible service provided by [Alibaba Cloud](https://www.alibabacloud.com).

To connect to OSS, we need to set:

- `endpoint`: The endpoint of oss, for example: `https://oss-cn-hangzhou.aliyuncs.com`
- `bucket`: The bucket name of oss.

> OSS provide internal endpoint for used at alibabacloud internally, please visit [OSS Regions and endpoints](https://www.alibabacloud.com/help/en/object-storage-service/latest/regions-and-endpoints) for more details.

> OSS only supports the virtual host style, users could meet errors like:
>
> ```xml
> <?xml version="1.0" encoding="UTF-8"?>
> <Error>
>  <Code>SecondLevelDomainForbidden</Code>
>  <Message>The bucket you are attempting to access must be addressed using OSS third level domain.</Message>
>  <RequestId>62A1C265292C0632377F021F</RequestId>
>  <HostId>oss-cn-hangzhou.aliyuncs.com</HostId>
> </Error>
> ```
>
> In that case, please enable virtual host style for requesting.

```rust,ignore
builder.endpoint("https://oss-cn-hangzhou.aliyuncs.com");
builder.region("<region>");
builder.bucket("<bucket_name>");
builder.enable_virtual_host_style();
```

### Minio

[minio](https://min.io/) is an open-source s3 compatible services.

To connect to minio, we need to set:

- `endpoint`: The endpoint of minio, for example: `http://127.0.0.1:9000`
- `region`: The region of minio. If you don't care about it, just set it to "auto", it will be ignored.
- `bucket`: The bucket name of minio.

```rust,ignore
builder.endpoint("http://127.0.0.1:9000");
builder.region("<region>");
builder.bucket("<bucket_name>");
```

### Neon Object Storage

[Neon Object Storage](https://neon.com/docs/storage/overview) is a S3-compatible service built into [Neon](https://neon.com/) projects. Buckets belong to a Neon branch, and each branch has its own S3 endpoint.

To connect to Neon Object Storage, we need to set:

- `endpoint`: The S3 endpoint of the branch, for example: `https://<branch_id>.storage.<cell>.<region>.aws.neon.tech`. The Neon Console and `neon env pull` (as `AWS_ENDPOINT_URL_S3`) show it.
- `region`: The region of the Neon project, for example: `us-east-2`. Neon accepts any signing region, but OpenDAL requires one. `S3::detect_region()` derives it from the endpoint.
- `bucket`: The bucket name.
- `access_key_id` and `secret_access_key`: The token ID (`nak_live_...`) and S3 secret access key of a Neon credential with the `storage:read` and `storage:write` scopes.

```rust,ignore
builder.endpoint("https://<branch_id>.storage.<cell>.<region>.aws.neon.tech");
builder.region("<region>");
builder.bucket("<bucket_name>");
builder.access_key_id("<token_id>");
builder.secret_access_key("<s3_secret_access_key>");
```

> Neon supports path-style requests only. Don't call `enable_virtual_host_style()`.

A new branch inherits the buckets and objects of its parent, and a credential issued on a branch also works on its descendants. To work against a copy of the data, point `endpoint` at the child branch. Writes there don't reach the parent.

Neon Object Storage has the following capability differences from S3:

- `write_can_append`: Neon ignores the append offset and replaces the object. Please override it to `false`.
- `stat_with_version`, `read_with_version`, `delete_with_version`, `list_with_versions`, `list_with_deleted`, `copy_with_source_version`, `restore`, `restore_with_version`, `restore_with_if_not_exists`: Neon doesn't enforce versioning and always returns the `null` version ID. Please override them to `false`.
- `copy_with_if_not_exists`, `copy_with_if_match`, `delete_with_if_match`: Neon ignores conditional headers on `CopyObject` and `DeleteObject`. Please override them to `false`.
- `write_with_if_not_exists`, `write_with_if_match`: Neon checks these conditions for a single `PutObject`, but ignores them on `CompleteMultipartUpload` and doesn't check them atomically across concurrent writers. Please override them to `false` when you rely on them for locking or compare-and-swap.

### Oracle Cloud Infrastructure Object Storage

[Oracle Cloud Infrastructure (OCI) Object Storage](https://www.oracle.com/cloud/storage/object-storage/) provides an Amazon S3 Compatibility API.

To connect with path-style requests, set:

- `endpoint`: The endpoint containing the Object Storage namespace and OCI region, for example: `https://<namespace>.compat.objectstorage.<region>.oci.customer-oci.com`.
- `region`: The OCI region identifier, for example: `us-ashburn-1`.
- `bucket`: The bucket name.
- `access_key_id` and `secret_access_key`: The access key and secret key from an OCI Customer Secret Key.

```rust,ignore
// Accepted endpoint includes "oci.customer-oci.com" and "oraclecloud.com", while "oci.customer-oci.com" is preferred.
builder.endpoint(
    "https://<namespace>.compat.objectstorage.<region>.oci.customer-oci.com",
);
builder.region("<region>");
builder.bucket("<bucket_name>");
builder.access_key_id("<access_key>");
builder.secret_access_key("<secret_key>");
```

OCI also supports virtual-hosted-style requests. Use the endpoint
`https://vhcompat.objectstorage.<region>.oci.customer-oci.com` and call
`enable_virtual_host_style()` when selecting that form.

OCI Object Storage differs from Amazon S3 in the following ways:

- OCI uses an Object Storage namespace instead of Amazon S3's global bucket namespace. Bucket names must be unique within the namespace according to the configured bucket scope.
- OCI organizes resources in compartments. Buckets created through the S3 Compatibility API use the tenancy's designated compartment, which defaults to the root compartment.
- OCI encrypts every object at rest and does not allow clients to enable or disable encryption through the API.
- OCI does not support object-level ACLs. OCI IAM policies control access instead, so S3 object ACL settings such as `default_acl` do not apply.
- OCI requires AWS Signature Version 4; Signature Version 2 is not supported.

### QingStor Object Storage

[QingStor Object Storage](https://www.qingcloud.com/products/qingstor) is a S3-compatible service provided by [QingCloud](https://www.qingcloud.com/).

To connect to QingStor Object Storage, we need to set:

- `endpoint`: The endpoint of QingStor s3 compatible endpoint, for example: `https://s3.pek3b.qingstor.com`
- `bucket`: The bucket name.

### Scaleway Object Storage

[Scaleway Object Storage](https://www.scaleway.com/en/object-storage/) is a S3-compatible and multi-AZ redundant object storage service.

To connect to Scaleway Object Storage, we need to set:

- `endpoint`: The endpoint of scaleway, for example: `https://s3.nl-ams.scw.cloud`
- `region`: The region of scaleway.
- `bucket`: The bucket name of scaleway.

### Tencent Cloud Object Storage (COS)

[COS](https://intl.cloud.tencent.com/products/cos) is a s3 compatible service provided by [Tencent Cloud](https://intl.cloud.tencent.com/).

To connect to COS, we need to set:

- `endpoint`: The endpoint of cos, for example: `https://cos.ap-beijing.myqcloud.com`
- `bucket`: The bucket name of cos.

### Tigris

[Tigris](https://www.tigrisdata.com/) is a globally distributed S3-compatible object storage service.

> Tigris serves data from the region nearest to the requester through a single global endpoint, and charges no egress fees.

To connect to Tigris, we need to set:

- `endpoint`: The endpoint of Tigris: `https://t3.storage.dev`
- `region`: The region of Tigris. Please set it to `auto`.
- `bucket`: The bucket name of Tigris.

Credentials are standard S3 access keys, provided via `access_key_id`/`secret_access_key` or the `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` environment variables. See [Tigris S3 SDK docs](https://www.tigrisdata.com/docs/sdks/s3/) for details.

```rust,ignore
builder.endpoint("https://t3.storage.dev");
builder.region("auto");
builder.bucket("<bucket_name>");
```

> `https://fly.storage.tigris.dev` is a legacy endpoint that still works, but `https://t3.storage.dev` is the current one.

Tigris buckets support zero-copy [snapshots and forks](https://www.tigrisdata.com/docs/snapshots-and-forks/). A forked bucket is a regular bucket, so it can be read and written through OpenDAL like any other — useful for testing against production data without copying it.

### Wasabi Object Storage

[Wasabi](https://wasabi.com/) is a s3 compatible service.

> Cloud storage pricing that is 80% less than Amazon S3.

To connect to wasabi, we need to set:

- `endpoint`: The endpoint of wasabi, for example: `https://s3.us-east-2.wasabisys.com`
- `bucket`: The bucket name of wasabi.

> Refer to [What are the service URLs for Wasabi's different storage regions?](https://wasabi-support.zendesk.com/hc/en-us/articles/360015106031) for more details.

### Cloudflare R2

[Cloudflare R2](https://developers.cloudflare.com/r2/) provides s3 compatible API.

> Cloudflare R2 Storage allows developers to store large amounts of unstructured data without the costly egress bandwidth fees associated with typical cloud storage services.


To connect to r2, we need to set:

- `endpoint`: The endpoint of r2, for example: `https://<account_id>.r2.cloudflarestorage.com`
- `bucket`: The bucket name of r2.
- `region`: When you create a new bucket, the data location is set to Automatic by default. So please use `auto` for region.
- `enable_exact_buf_write`: R2 requires the non-tailing parts size to be exactly the same. Please enable this option to avoid the error `All non-trailing parts must have the same length`.

R2 has the following capability differences from S3:

- `delete_max_size`: R2's delete objects will return `Internal Error` if the batch is larger than `700`. Please override `delete_max_size` to `700`.
- `stat_with_override_cache_control`, `stat_with_override_content_disposition`, `stat_with_override_content_type`: R2 doesn't support stat with response override queries. Please override them to `false`.

### Google Cloud Storage XML API
[Google Cloud Storage XML API](https://cloud.google.com/storage/docs/xml-api/overview) provides s3 compatible API.
- `endpoint`: The endpoint of Google Cloud Storage XML API, for example: `https://storage.googleapis.com`
- `bucket`: The bucket name.
- To access GCS via S3 API, please enable `features = ["native-tls"]` in your `Cargo.toml` to avoid connection being reset when using `rustls`. Tracking in <https://github.com/seanmonstar/reqwest/issues/1809>

### Ceph Rados Gateway
Ceph supports a RESTful API that is compatible with the basic data access model of the Amazon S3 API.

For more information, refer: <https://docs.ceph.com/en/latest/radosgw/s3/>

### SeaweedFS

[SeaweedFS](https://github.com/seaweedfs/seaweedfs) is an open-source distributed storage system that serves an S3 API from its gateway.

To connect to SeaweedFS, we need to set:

- `endpoint`: The endpoint of the SeaweedFS S3 gateway, for example: `http://127.0.0.1:8333`
- `region`: The region of SeaweedFS. SeaweedFS ignores it, so set it to `us-east-1` when a region is required.
- `bucket`: The bucket name.

```rust,ignore
builder.endpoint("http://127.0.0.1:8333");
builder.region("us-east-1");
builder.bucket("<bucket_name>");
```

Credentials are standard S3 access keys. A gateway started without any configured identity accepts anonymous requests, so `allow_anonymous` also works.

SeaweedFS has the following capability differences from S3:

- `write_can_append`: SeaweedFS doesn't support appending to an existing object through the S3 API. Please override it to `false`.
- Object versioning is per-bucket and off by default. Enable it with `PutBucketVersioning` before relying on the version capabilities, or override them to `false`.
