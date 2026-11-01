- Proposal Name: `list_recursive`
- Start Date: 2023-11-08
- RFC PR: [apache/opendal#3526](https://github.com/apache/opendal/pull/3526)
- Tracking Issue: [apache/opendal#0000](https://github.com/apache/opendal/issues/0000)

# Summary

Use `recursive` to replace `delimiter`.

# Motivation

OpenDAL add `delimiter` in `list` to allow users to control the list behavior:

- `delimiter == "/"` means use `/` as delimiter of path, it behaves like list current dir.
- `delimiter == ""` means don't set delimiter of path, it behaves like list current dir and all it's children.

Ideally, we should allow users to input any delimiter such as `|`, `-`, and `+`. 

The `delimiter` concept can be challenging for users unfamiliar with object storage services. Currently, only `/` and empty spaces are accepted as delimiters, despite not being fully implemented across all services. We need to inform users that `delimiter == "/"` is used to list the current directory, while `delimiter == ""` is used for recursive listing. This may not be immediately clear.

So, why not use `recursive` directly for more clear API behavior?

# Guide-level explanation

OpenDAL will use `recursive` to replace `delimiter`. Default behavior is not changed, so users that using `op.list()` is not affected.

For users who is using `op.list_with(path).delimiter(delimiter)`:

- `op.list_with(path).delimiter("")` -> `op.list_with(path).recursive(true)`
- `op.list_with(path).delimiter("/")` -> `op.list_with(path).recursive(false)`
- `op.list_with(path).delimiter(other_value)`: not supported anymore.

# Reference-level explanation

We will add `recursive` as a new arg in `OpList` and remove all fields related to `delimiter`.

# Drawbacks

## Can't support to use `|`, `-`, and `+` as delimiter

We never support this feature before.

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

## Add delete with recursive support

Some services have native support for delete with recursive, such as azfile. We can add this feature in the future if needed.
