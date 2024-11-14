## Capabilities

This service can be used to:

- [x] stat
- [x] read
- [x] write
- [x] create_dir
- [x] delete
- [x] copy
- [x] rename
- [x] list
- [ ] ~~presign~~
- [ ] blocking

## Configuration

- `root`: Set the working directory of `OpenDAL`
- `endpoint`: Set the network address of redis server
- `cluster_endpoints`: Set the network address of redis cluster server. This parameter is mutually exclusive with the `endpoint` parameter.
- `username`: Set the username of Redis
- `password`: Set the password for authentication
- `db`: Set the DB of redis

You can refer to [`RedisBuilder`]'s docs for more information

## Example

### Via Builder

```rust,no_run
use anyhow::Result;
use opendal::services::Redis;
use opendal::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Redis::default();

    // this will build a Operator accessing Redis which runs on tcp://localhost:6379
    let op: Operator = Operator::new(builder)?.finish();
    Ok(())
}
```
