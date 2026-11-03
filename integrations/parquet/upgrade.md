# Upgrade to v0.10

## Upgrade OpenDAL to v0.59

`parquet_opendal` 0.10 uses `opendal` 0.59. Update both dependencies together when they appear in the same dependency graph:

```diff
-opendal = "0.58"
-parquet_opendal = "0.9"
+opendal = "0.59"
+parquet_opendal = "0.10"
```

# Upgrade to v0.9

## Bump Arrow and Parquet versions to v59

`parquet_opendal` now requires `arrow` and `parquet` version 59.0.0 or higher.

# Upgrade to v0.8

## Bump arrow version to v58

`parquet_opendal` now requires `arrow` version 58.0.0 or higher.

# Upgrade to v0.5

## Bump arrow version to v54

`parquet_opendal` now requires `arrow` version 54.0.0 or higher.
