# Google Cloud Storage gRPC Protobuf Contract

`google/storage/v2/storage.proto` contains the object operations and fields used
by OpenDAL. It is a wire-compatible subset of the `googleapis` definition at
commit `2e9c5681901a2eebf7f547f0b60c895b1732415e`. The subset also retains the
resumable write operations required to recover uploads across gRPC streams.

`upstream-descriptor.bin` contains the complete upstream protobuf descriptor set
at that commit. The generator rejects method, streaming, message, field, oneof,
or presence changes that are incompatible with the pinned upstream contract.

Run the following command from the repository root after changing the subset:

```shell
just generate gcs-grpc
```

The generator writes the Tonic client modules to `src/generated`.

To update the upstream contract, generate a new descriptor set from the complete
upstream `google/storage/v2/storage.proto` and its imports with
`protoc --include_imports`, update the retained definitions, and update the
commit references together.
