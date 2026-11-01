- Proposal Name: `list_with_deleted`
- Start Date: 2025-01-02
- RFC PR: [apache/opendal#5495](https://github.com/apache/opendal/pull/0000)
- Tracking Issue: [apache/opendal#5496](https://github.com/apache/opendal/issues/5496)

# Summary

Add `list_with(path).deleted(true)` to enable users to list deleted files from storage services.

# Motivation

OpenDAL is currently working on adding support for file versions, allowing users to read, list, and delete them.

```rust
// Read given version
op.read_with(path).version(version_id).await;
// Fetch the metadata of given version.
op.stat_with(path).version(version_id).await;
// Delete the given version.
op.delete_with(path).version(version_id).await;
// List the path's versions.
op.list_with(path).versions().await;
```

However, to implement the complete data recovery workflow, we should also include support for recovering deleted files from storage services. This feature is referred to as `DeleteMarker` in S3 and `Soft Deleted` in Azure Blob Storage or Google Cloud Storage. Users can utilize these deleted files (or versions) to restore files that may have been accidentally deleted.

# Guide-level explanation

I suggest adding `list_with(path).deleted(true)` to allow users to list deleted files from storage services.

```rust
let entries = op.list_with(path).deleted(true).await;
```

Please note that `deleted` here means "including deleted files" rather than "only deleted files." Therefore, `list_with(path).deleted(true)` will list both current files and deleted ones.

At the same time, we will add an `is_deleted` field to the `Metadata` struct to indicate whether the file has been deleted. Together with the existing `is_current` field, we will have the following matrix:

| `is_current`  | `is_deleted` | Description                                                                                                                                                                                                                                                                                                                                                                          |
|---------------|--------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Some(true)`  | `false`      | **The metadata's associated version is the latest, current version.** This is the normal state, indicating that this version is the most up-to-date and accessible version.                                                                                                                                                                                                          |
| `Some(true)`  | `true`       | **The metadata's associated version is the latest, deleted version (Latest Delete Marker or Soft Deleted).** This is particularly important in object storage systems like S3. It signifies that this version is the **most recent delete marker**, indicating the object has been deleted. Subsequent GET requests will return 404 errors unless a specific version ID is provided. |
| `Some(false)` | `false`      | **The metadata's associated version is neither the latest version nor deleted.** This indicates that this version is a previous version, still accessible by specifying its version ID.                                                                                                                                                                                              |
| `Some(false)` | `true`       | **The metadata's associated version is not the latest version and is deleted.** This represents a historical version that has been marked for deletion. Users will need to specify the version ID to access it, and accessing it may be subject to specific delete marker behavior (e.g., in S3, it might not return actual data but a specific delete marker response).             |
| `None`        | `false`      | **The metadata's associated file is not deleted, but its version status is either unknown or it is not the latest version.** This likely indicates that versioning is not enabled for this file, or versioning information is unavailable.                                                                                                                                           |
| `None`        | `true`       | **The metadata's associated file is deleted, but its version status is either unknown or it is not the latest version.** This typically means the file was deleted without versioning enabled, or its versioning information is unavailable. This may represent an actual data deletion operation rather than an S3 delete marker.                                                   |


# Reference-level explanation

- Implement the `list_with(path).deleted(true)` API for the `Operator`.
- Add an `is_deleted` field to `Metadata`.
- Integrate logic for including deleted files into the `list` method of the storage service.

# Drawbacks

None.

# Rationale and alternatives

## Why "including deleted files" rather than "only deleted files"?

Most storage services are designed to list files along with deleted files, rather than exclusively listing deleted files. For example:

- S3's [ListObjectVersions](https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectVersions.html) API lists all versions of an object, including delete markers.
- GCS's [list](https://cloud.google.com/storage/docs/json_api/v1/objects/list) API includes a parameter `softDeleted` to display soft-deleted files.
- AzBlob's [List Blobs](https://learn.microsoft.com/en-us/rest/api/storageservices/list-blobs) API supports the parameter `include=deleted` to list soft-deleted blobs.

So, it is more natural to list files along with deleted files, rather than only listing deleted files.

# Prior art

None.

# Unresolved questions

None.

# Future possibilities

None.