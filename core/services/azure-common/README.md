# Apache OpenDAL™ Azure Common

`opendal-service-azure-common` provides shared Azure Storage configuration,
authentication, request, and response helpers for Apache OpenDAL™ service
implementations.

This crate is an implementation building block rather than a standalone storage
service. Applications should normally use one of the public Azure services
through the `opendal` facade:

- `services-azblob` for Azure Blob Storage;
- `services-azdls` for Azure Data Lake Storage Gen2;
- `services-azfile` for Azure Files.

Service implementers can depend on this crate directly when they need the same
Azure connection-string and authentication behavior.

## Documentation

- [Rust API documentation](https://docs.rs/opendal-service-azure-common)
- [Services and configuration](https://opendal.apache.org/services)
- [Apache OpenDAL documentation](https://opendal.apache.org/docs/)

## License

Licensed under the Apache License, Version 2.0.
