# Apache OpenDAL™ dav-server integration

[![Build Status]][actions] [![Latest Version]][crates.io] [![Crate Downloads]][crates.io] [![chat]][discord]

[build status]: https://img.shields.io/github/actions/workflow/status/apache/opendal/ci_integration_dav_server.yml?branch=main
[actions]: https://github.com/apache/opendal/actions?query=branch%3Amain
[latest version]: https://img.shields.io/crates/v/dav-server-opendalfs.svg
[crates.io]: https://crates.io/crates/dav-server-opendalfs
[crate downloads]: https://img.shields.io/crates/d/dav-server-opendalfs.svg
[chat]: https://img.shields.io/discord/1081052318650339399
[discord]: https://opendal.apache.org/discord

`dav-server-opendalfs` is an [`dav-server`](https://github.com/messense/dav-server-rs) implementation using opendal.

This crate can help you to access ANY storage services with the same webdav API.

## Useful Links

- Documentation: [release](https://docs.rs/dav-server-opendalfs/) | [dev](https://opendal.apache.org/docs/dav-server-opendalfs/dav_server_opendalfs/)

## Examples

```
use anyhow::Result;
use dav_server::davpath::DavPath;
use dav_server_opendalfs::OpendalFs;
use opendal::services::Memory;
use opendal::Operator;

#[tokio::test]
async fn test() -> Result<()> {
 let op = Operator::new(Memory::default())?;

 let webdavfs = OpendalFs::new(op);

 let metadata = webdavfs
     .metadata(&DavPath::new("/").unwrap())
     .await
     .unwrap();
 println!("{}", metadata.is_dir());

 Ok(())
}
```

## Branding

The first and most prominent mentions must use the full form: **Apache OpenDAL™** of the name for any individual usage (webpage, handout, slides, etc.) Depending on the context and writing style, you should use the full form of the name sufficiently often to ensure that readers clearly understand the association of both the OpenDAL project and the OpenDAL software product to the ASF as the parent organization.

For more details, see the [Apache Product Name Usage Guide](https://www.apache.org/foundation/marks/guide).

## License and Trademarks

Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0

Apache OpenDAL, OpenDAL, and Apache are either registered trademarks or trademarks of the Apache Software Foundation.
