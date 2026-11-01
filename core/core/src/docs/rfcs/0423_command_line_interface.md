- Proposal Name: `command_line_interface`
- Start Date: 2022-07-08
- RFC PR: [apache/opendal#423](https://github.com/apache/opendal/pull/423)
- Tracking Issue: [apache/opendal#422](https://github.com/apache/opendal/issues/422)

# Summary

Add command line interface for OpenDAL.

# Motivation

> **Q**: There are so many cli out there, why we still need a cli for OpenDAL?
> 
> **A**: Because there are so many cli out there.

To manipulate our date store in different could service, we need to install different clis:

- [`aws-cli`]/[`s3cmd`]/... for AWS (S3)
- [`azcopy`] for Azure Storage Service
- [`gcloud`] for Google Cloud

Those clis provide native and seamless experiences for their own products but also lock us and our data. 

However, for 80% cases, we just want to do simple jobs like `cp`, `mv` and `rm`. It's boring to figure out how to use them:

- `aws --endpoint-url http://127.0.0.1:9900/ s3 cp data s3://testbucket/data  --recursive`
- `azcopy copy 'C:\myDirectory' 'https://mystorageaccount.blob.core.windows.net/mycontainer' --recursive`
- `gsutil cp data gs://testbucket/`

Can we use them in the same way? Can we let the data flow freely? 

Let's look back OpenDAL's slogan:

**Open Data Access Layer that connect the whole world together**

This is a natural extension for OpenDAL: providing a command line interface!

# Guide-level explanation

OpenDAL will provide a new cli called: `oli`. It's a shortcut of `OpenDAL Command Line Interface`.

Users can install this cli via:

```shell
cargo install oli
```

Or using they favourite package management:

```shell
# Archlinux
pacman -S oli
# Debian / Ubuntu
apt install oli
# Rocky Linux / Fedora
dnf install oli
# macOS
brew install oli
```

With `oli`, users can:

- Upload files to s3: `oli cp books.csv s3://bucket/books.csv`
- Download files from azblob: `oli cp azblob://bucket/books.csv /tmp/books.csv`
- Move data between storage services: `oli mv s3://bucket/dir azblob://bucket/dir`
- Delete all files: `oli rm -rf s3://bucket`

`oli` also provide alias to make cloud data manipulating even natural:

- `ocp` for `oli cp`
- `ols` for `oli ls`
- `omv` for `oli mv`
- `orm` for `oli rm`
- `ostat` for `oli stat`

`oli` will provide profile management so users don't need to provide credential every time:

- `oli profile add my_s3 --bucket test --access-key-id=example --secret-access-key=example`
- `ocp my_s3://dir /tmp/dir`

# Reference-level explanation

`oli` will be a separate crate apart from `opendal` so we will not pollute the dependencies of `opendal`. But `oli` will be releases at the same time with the same version of `opendal`. That means `oli` will always use the same (latest) version of opendal.

Most operations of `oli` should be trivial, we will propose new RFCs if requiring big changes.

`oli` won't keep configuration. All config will go through environment, for example:

- `OIL_COLOR=always`
- `OIL_CONCURRENCY=16`

Besides, `oil` will read profile from env like `cargo`:

- `OIL_PROFILE_TEST_TYPE=s3`
- `OIL_PROFILE_TEST_ENDPOINT=http://127.0.0.1:1090`
- `OIL_PROFILE_TEST_BUCKET=test_bucket`
- `OIL_PROFILE_TEST_ACCESS_KEPT_ID=access_key_id`
- `OIL_PROFILE_TEST_SECRET_ACCESS_KEY=secret_access_key`

With those environments, we can:

```shell
ocp path/to/dir test://test/to/dir
```

# Drawbacks

None

# Rationale and alternatives

## s3cmd

[s3cmd](https://s3tools.org/s3cmd) is a command line s3 client for Linux and Mac.

```shell
Usage: s3cmd [options] COMMAND [parameters]

S3cmd is a tool for managing objects in Amazon S3 storage. It allows for
making and removing "buckets" and uploading, downloading and removing
"objects" from these buckets.

Commands:
  Make bucket
      s3cmd mb s3://BUCKET
  Remove bucket
      s3cmd rb s3://BUCKET
  List objects or buckets
      s3cmd ls [s3://BUCKET[/PREFIX]]
  List all object in all buckets
      s3cmd la 
  Put file into bucket
      s3cmd put FILE [FILE...] s3://BUCKET[/PREFIX]
  Get file from bucket
      s3cmd get s3://BUCKET/OBJECT LOCAL_FILE
  Delete file from bucket
      s3cmd del s3://BUCKET/OBJECT
  Delete file from bucket (alias for del)
      s3cmd rm s3://BUCKET/OBJECT
  Restore file from Glacier storage
      s3cmd restore s3://BUCKET/OBJECT
  Synchronize a directory tree to S3 (checks files freshness using 
       size and md5 checksum, unless overridden by options, see below)
      s3cmd sync LOCAL_DIR s3://BUCKET[/PREFIX] or s3://BUCKET[/PREFIX] LOCAL_DIR
  Disk usage by buckets
      s3cmd du [s3://BUCKET[/PREFIX]]
  Get various information about Buckets or Files
      s3cmd info s3://BUCKET[/OBJECT]
  Copy object
      s3cmd cp s3://BUCKET1/OBJECT1 s3://BUCKET2[/OBJECT2]
  Modify object metadata
      s3cmd modify s3://BUCKET1/OBJECT
  Move object
      s3cmd mv s3://BUCKET1/OBJECT1 s3://BUCKET2[/OBJECT2]
  Modify Access control list for Bucket or Files
      s3cmd setacl s3://BUCKET[/OBJECT]
  Modify Bucket Policy
      s3cmd setpolicy FILE s3://BUCKET
  Delete Bucket Policy
      s3cmd delpolicy s3://BUCKET
  Modify Bucket CORS
      s3cmd setcors FILE s3://BUCKET
  Delete Bucket CORS
      s3cmd delcors s3://BUCKET
  Modify Bucket Requester Pays policy
      s3cmd payer s3://BUCKET
  Show multipart uploads
      s3cmd multipart s3://BUCKET [Id]
  Abort a multipart upload
      s3cmd abortmp s3://BUCKET/OBJECT Id
  List parts of a multipart upload
      s3cmd listmp s3://BUCKET/OBJECT Id
  Enable/disable bucket access logging
      s3cmd accesslog s3://BUCKET
  Sign arbitrary string using the secret key
      s3cmd sign STRING-TO-SIGN
  Sign an S3 URL to provide limited public access with expiry
      s3cmd signurl s3://BUCKET/OBJECT <expiry_epoch|+expiry_offset>
  Fix invalid file names in a bucket
      s3cmd fixbucket s3://BUCKET[/PREFIX]
  Create Website from bucket
      s3cmd ws-create s3://BUCKET
  Delete Website
      s3cmd ws-delete s3://BUCKET
  Info about Website
      s3cmd ws-info s3://BUCKET
  Set or delete expiration rule for the bucket
      s3cmd expire s3://BUCKET
  Upload a lifecycle policy for the bucket
      s3cmd setlifecycle FILE s3://BUCKET
  Get a lifecycle policy for the bucket
      s3cmd getlifecycle s3://BUCKET
  Remove a lifecycle policy for the bucket
      s3cmd dellifecycle s3://BUCKET
  List CloudFront distribution points
      s3cmd cflist 
  Display CloudFront distribution point parameters
      s3cmd cfinfo [cf://DIST_ID]
  Create CloudFront distribution point
      s3cmd cfcreate s3://BUCKET
  Delete CloudFront distribution point
      s3cmd cfdelete cf://DIST_ID
  Change CloudFront distribution point parameters
      s3cmd cfmodify cf://DIST_ID
  Display CloudFront invalidation request(s) status
      s3cmd cfinvalinfo cf://DIST_ID[/INVAL_ID]
```

## aws-cli

[aws-cli](https://aws.amazon.com/cli/) is the official cli provided by AWS.

```shell
$ aws s3 ls s3://mybucket
        LastWriteTime            Length Name
        ------------             ------ ----
                                PRE myfolder/
2013-09-03 10:00:00           1234 myfile.txt

$ aws s3 cp myfolder s3://mybucket/myfolder --recursive
upload: myfolder/file1.txt to s3://mybucket/myfolder/file1.txt
upload: myfolder/subfolder/file1.txt to s3://mybucket/myfolder/subfolder/file1.txt

$ aws s3 sync myfolder s3://mybucket/myfolder --exclude *.tmp
upload: myfolder/newfile.txt to s3://mybucket/myfolder/newfile.txt
```

## azcopy

[azcopy](https://github.com/Azure/azure-storage-azcopy) is the new Azure Storage data transfer utility.

```shell
azcopy copy 'C:\myDirectory\myTextFile.txt' 'https://mystorageaccount.blob.core.windows.net/mycontainer/myTextFile.txt'

azcopy copy 'https://mystorageaccount.blob.core.windows.net/mycontainer/myTextFile.txt' 'C:\myDirectory\myTextFile.txt'

azcopy sync 'C:\myDirectory' 'https://mystorageaccount.blob.core.windows.net/mycontainer' --recursive
```

## gsutil

[gsutil](https://cloud.google.com/storage/docs/gsutil) is a Python application that lets you access Cloud Storage from the command line.

```shell
gsutil cp [OPTION]... src_url dst_url
gsutil cp [OPTION]... src_url... dst_url
gsutil cp [OPTION]... -I dst_url

gsutil mv [-p] src_url dst_url
gsutil mv [-p] src_url... dst_url
gsutil mv [-p] -I dst_url

gsutil rm [-f] [-r] url...
gsutil rm [-f] [-r] -I
```

# Unresolved questions

None.

# Future possibilities

None.

[`aws-cli`]: https://github.com/aws/aws-cli
[`s3cmd`]: https://s3tools.org/s3cmd 
[`azcopy`]: https://github.com/Azure/azure-storage-azcopy
[`gcloud`]: https://cloud.google.com/sdk/docs/install
