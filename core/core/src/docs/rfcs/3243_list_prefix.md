- Proposal Name: `list_prefix`
- Start Date: 2023-10-08
- RFC PR: [apache/opendal#3243](https://github.com/apache/opendal/pull/3243)
- Tracking Issue: [apache/opendal#3247](https://github.com/apache/opendal/issues/3247)

# Summary

Allow users to specify a prefix and remove the requirement that the path must end with `/`.

# Motivation

OpenDAL uses `/` to distinguish between a file and a directory. This design is necessary for object storage services such as S3 and GCS, where both `abc` (file) and `abc/` (directory) can coexist. We require users to provide the correct path to the API. For instance, when using `read("abc/")`, it returns `IsADirectory`, whereas with `list("abc/")` it returns `NotADirectory`. This behavior may be perplexing for users.

As a side-effect of this design, OpenDAL always return exist for `stat("not_exist/")` since there is no way for OpenDAL to check if `not_exist/file_example` is exist via `HeadObject` call.

There are some issues and pull requests related to those issues.

- [Invalid metadata for dir objects in s3](https://github.com/apache/opendal/issues/3199)
- [`is_exist` always return true for key end with '/', in S3 service](https://github.com/apache/opendal/issues/2086)

POSIX-like file systems also have their own issues, as they lack native support for listing a prefix.

Give file tree like the following:

```shell
abc/
abc/def
abc/xyz/
```

Calling `list("ab")` will return `NotFound` after we removing the requirement that the path must end with `/`.

So I propose the following changes of OpenDAL API behaviors:

- Remove the requirement that the path for `list` must end with `/`.
- Object storage services will use `list_object` API to check if a dir is exist.
- Simulate the list prefix behavior for POSIX-like file systems.

# Guide-level explanation

Given the following file tree:

```shell
abc/
abc/def_file
abc/def_dir/
abc/def_dir/xyz_file
abc/def_dir/xyz_dir/
```

While listing a path:

| Case                    | Path            | Result                              | Description                             |
|-------------------------|-----------------|-------------------------------------|-----------------------------------------|
| list dir                | `abc/`          | `abc/def_file` <br/> `abc/def_dir/` | children that matches the dir           |
| list prefix             | `abc/def`       | `abc/def_file` <br/> `abc/def_dir/` | children that matches the prefix        |
| list file               | `abc/def_file`  | `abc/def_file`                      | the only children that matches the path |
| list dir without `/`    | `abc/def_dir`   | `abc/def_dir/`                      | the only children that matches the path |
| list file ends with `/` | `abc/def_file/` | EMPTY                               | no children matches the dir             |
| list not exist dir      | `def/`          | EMPTY                               | no children found matches the dir       |
| list not exist file     | `def`           | EMPTY                               | no children found matches the prefix    |

While listing a path with `delimiter` set to `""`:

| Case                    | Path            | Result                                                                                        | Description                             |
|-------------------------|-----------------|-----------------------------------------------------------------------------------------------|-----------------------------------------|
| list dir                | `abc/`          | `abc/def_file` <br/> `abc/def_dir/` <br/> `abc/def_dir/xyz_file` <br/> `abc/def_dir/xyz_dir/` | children that matches the dir           |
| list prefix             | `abc/def`       | `abc/def_file` <br/> `abc/def_dir/` <br/> `abc/def_dir/xyz_file` <br/> `abc/def_dir/xyz_dir/` | children that matches the prefix        |
| list file               | `abc/def_file`  | `abc/def_file`                                                                                | the only children that matches the path |
| list dir without `/`    | `abc/def_dir`   | `abc/def_dir/` <br/> `abc/def_dir/xyz_file` <br/> `abc/def_dir/xyz_dir/`                      | children that matches the path          |
| list file ends with `/` | `abc/def_file/` | EMPTY                                                                                         | no children matches the dir             |
| list not exist dir      | `def/`          | EMPTY                                                                                         | no children found matches the dir       |
| list not exist file     | `def`           | EMPTY                                                                                         | no children found matches the prefix    |

While stat a path:

| Case                   | Path            | Result                                     |
|------------------------|-----------------|--------------------------------------------|
| stat existing dir      | `abc/`          | Metadata with dir mode                     | 
| stat existing file     | `abc/def_file`  | Metadata with file mode                    | 
| stat dir without `/`   | `abc/def_dir`   | Error `NotFound` or metadata with dir mode | 
| stat file with `/`     | `abc/def_file/` | Error `NotFound`                           |
| stat not existing path | `xyz`           | Error `NotFound`                           |

While create dir on a path:

| Case                        | Path   | Result                     |
|-----------------------------|--------|----------------------------|
| create dir on existing dir  | `abc/` | Ok                         |
| create dir on existing file | `abc`  | Error with `NotADirectory` |
| create dir with `/`         | `xyz/` | Ok                         |
| create dir without `/`      | `xyz`  | Ok with `xyz/` created     |

# Reference-level explanation

For POSIX-like services, we will:

- Simulate the list prefix behavior by listing the parent dir and filter the children that matches the prefix.
- Return `NotFound` while stat an existing file with `/`

For object storage services, we will:

- Use `list_object` API while stat a path ends with `/`.
  - Return dir metadata if the dir is exist or there is at least a children.
  - Return `NotFound` if the dir is not exist and there is no children.
- Check path before create dir with a path not ends with `/`.
  - Return `NotADirectory` if the path is exist.
  - Create the dir with `/` appended.

# Drawbacks

None

# Rationale and alternatives

None

# Prior art

None

# Unresolved questions

None

# Future possibilities

None
