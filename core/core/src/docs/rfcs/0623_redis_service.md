- Proposal Name: `redis_service`
- Start Date: 2022-08-31
- RFC PR: [apache/opendal#0623](https://github.com/apache/opendal/pull/0623)
- Tracking Issue: [apache/opendal#641](https://github.com/apache/opendal/issues/0641)

# Summary

Use [redis](https://redis.io) as a service of OpenDAL.

# Motivation

Redis is a fast, in-memory cache with persistent and distributed storage functionalities. It's widely used in production.

Users also demand more backend support. Redis is a good candidate.

# Guide-level explanation

Users only need to provide the network address, username and password to create a Redis Operator. Then everything will act as other operators do.

```rust
use opendal::services::redis::Builder;
use opendal::Operator;

let builder = Builder::default();

// set the endpoint of redis server
builder.endpoint("tcps://domain.to.redis:2333");
// set the username of redis
builder.username("example");
// set the password
builder.password(&std::env::var("OPENDAL_REDIS_PASSWORD").expect("env OPENDAL_REDIS_PASSWORD not set"));
// root path
builder.root("/example/");

let op = Operator::new(
    builder.build()?    // services::redis::Backend
);

// congratulations, you can use `op` just like any other operators!
```

# Reference-level explanation

To ease the development, [redis-rs](https://crates.io/crates/redis) will be used.

Redis offers a key-value view, so the path of files could be represented as the key of the key-value pair.

The content of file will be represented directly as `String`, and metadata will be encoded as [`bincode`](https://github.com/bincode-org/bincode.git) before storing as `String`.

```text
+------------------------------------------+
|Object: /home/monika/                     |
|                                          |           SET
|child: Key: v0:k:/home/monika/           -+---------> 1) /home/monika/poem0.txt
|                                          |
|/* directory has no content  */           |
|                                          |
|metadata: Key: v0:m:/home/monika/         |
+------------------------------------------+

+------------------------------------------+
|Object: /home/monika/poem0.txt            |
|                                          |
| /*      file has no children        */   |
|                                          |
|content: Key: v0:c:/home/monika/poem0.txt-+--+
|                                          |  |
|metadata: Key: v0:m:/home/monika/poem0.txt|  |
|  |                                       |  |
+--+---------------------------------------+  |
   |                                          v
   +> STRING                                STRING
     +----------------------+              +--------------------+
     |\x00\x00\x00\x00\xe6\a|              |1JU5T3MON1K413097321|
     |\x00\x00\xf8\x00\a4)!V|              |&JU5$T!M0N1K4$%#@#$%|
     |\x81&\x00\x00\x00Q\x00|              |3231J)U_ST#MONIKA@#$|
     |         ...          |              |1557(m0N1ka3just4M  |
     +----------------------+              |      ...           |
                                           +--------------------+
```

The [redis-rs](https://crates.io/crates/redis)'s high level APIs is preferred.

```rust
const VERSION: usize = 0;

/// meta_key will produce the key to object's metadata
/// "/path/to/object/" -> "v{VERSION}:m:/path/to/object"
fn meta_key(path: &str) -> String {
    format!("v{}:m:{}", VERSION, path)
}

/// content_key will produce the key to object's content
/// "/path/to/object/" -> "v{VERSION}:c:/path/to/object"
fn content_key(path: &str) -> String {
    format!("v{}:c:{}", VERSION, path)
}

fn connect() -> Result<()> {
    let client = redis::Client::open("redis://localhost:6379")?;
    let con = client.get_async_connection()?;
}
```

## Forward Compatibility

All keys used will have a `v0` prefix, indicating it's using the very first version of `OpenDAL` `Redis` API.

When there are changes to the layout, like refactoring the layout of storage, the version number should be updated, too. Further versions should take the compatibility with former implementations into consideration.

## Create File

If user is creating a file with root `/home/monika/`, and relative path `poem0.txt`.

```rust
// mode: ObjectMode
// path: relative path string
let path = get_abs_path(path);  // /home/monika/ <> /poem.txt -> /home/monika/poem.txt
let m_path = meta_key(path);  // path of metadata
let c_path = content_key(path);  // path of content
let last_modified = OffsetDatetime::now_utc().to_string();

let mut meta = ObjectMeta::default();
meta.set_mode(ObjectMode::FILE);
meta.set_last_modified(OffsetDatetime::now_utc());

let encoded = bincode::encode_to_vec(meta)?;

con.set(c_path, "".to_string())?;
con.set(m_path, encoded.as_slice())?;
```

This will create two key-value pair. For object content, its key is `v0:c:/home/monika/poem0.txt`, the value is an empty `String`; For metadata, the key is `v0:m:/home/monika/poem0.txt`, the value is a `bincode` encoded `ObjectMetadata` structure binary string.

On creating a file or directory, the backend should also create its all parent directories if not present.

```rust
// create a file under `PATH`
let mut path = std::path::PathBuf::new(PATH);
let mut con = client.new_async_connection().await?;

while let Some(parent) = path.parent() {
    let (p, c): (String, String) = (parent.display(), path.display());
    let to_children = format!("v0:ch:{}", p);
    con.sadd(to_children, c).await?;
    path = parent;
}
```

## Read File

Opendal empowers users to read with the `path` object, `offset` of the cursor and `size` to read. Redis provided a `GETRANGE` command which perfectly fit into it.

```rust
// path: "poem0.txt"
// offset: Option<u64>, the offset of reading
// size: Option<u64>, the size of reading
let path = get_abs_path(path);
let c_path = content_key(path);
let (mut start, mut end) = (0, -1);
if let Some(offset) = offset {
    start = offset;
}
if let Some(size) = size {
    end = start + size;
}
let buf: Vec<u8> = con.getrange(c_path, start, end).await?;
Box::new(buf)
```

```redis
GET v0:c:/home/monika/poem0.txt
```

## Write File

Redis ensures the writing of a single entry to be atomic, no locking is required.

What needs to take care by opendal, besides the content of object, is its metadata. For example, though offering a `OBJECT IDLETIME` command, redis cannot record the last modified time of a key, so this should be done in opendal.

```rust
use redis::AsyncCommands;
// args: &OpWrite
// r: BytesReader
let mut buf = vec![];
let content_length: u64 = futures::io::copy(r, &mut buf).await?;
let last_modified: String = OffsetDateTime::now().to_string();

// content md5 will not be offered

let mut meta = ObjectMetadata::default();
meta.set_content_length(content_length);
meta.set_last_modified(last_modified);

// `ObjectMetadata` has implemented the `Serialize` and `Deserialize` trait of `Serde`
// so bincode could serialize and deserialize it.
let bytes = bincode::encode_to_vec(&meta)?;

let path = get_abs_path(args.path());
let m_path = meta_key(path);
let c_path = content_key(path);

con.set(c_path, content).await?;
con.set(m_path, bytes).await?;
```

```redis
SET v0:c:/home/monika/poem.txt content_string
SET v0:m:/home/monika/poem.txt <bincode encoded metadata>
```

## Stat

To get the metadata of an object, using the `GET` command and deserialize from bincode.

```rust
let path = get_abs_path(args.path());
let meta = meta_key(path);
let bin: Vec<u8> = con.get(meta).await?;
let meta: ObjectMeta = bincode::deserialize(bin.as_slice())?;
```

```redis
GET v0:m:/home/monika/poem.txt
```

## List

For listing directories, just `SSCAN` through the child list of the directory, nice and correct.

```rust
// print all sub-directories of `PATH`

let s_key = format!("v0:k:{}", PATH);
let mut con = client.new_async_connection().await?;
let mut it = con.sscan::<&str, String>(s_key).await?;

while let Some(child) = it.next_item().await {
    println!("get sub-dir: {}", child);
}
```

## Delete

All subdirectories of path will be listed and removed.

On deleting a file or directory, the backend should remove the entry from its parent's `SET`, and remove all children of entry.

This could be done by postorder deleting.

```rust
async fn remove_entry(con: &mut redis::aio::AsyncConnection, entry: String) {
    let skey = format!("v0:ch:{}", entry);
    let it = con.sscan::<&str, String>(skey).await?;
    while let Some(child) = it.next_item().await {
        remove_entry(&mut con, child).await;
    }
    if let Some(parent) = std::PathBuf::new(entry).parent() {
        let p: String = parent.display();
        let parent_skey = format!("v0:ch:{}", p);
        let _ = con.srem(parent_skey, entry).await;
    }
    // remove metadata and content
}
```

## Blocking APIs

`redis-rs` also offers a synchronous version of API, just port the functions above to its synchronous version.

# Drawbacks

1. New dependencies is introduced: `redis-rs` and `bincode`;
2. Some calculations have to be done in client side, this will affect the performance;
3. Grouping atomic operations together doesn't promise transactional access, this may lead to data racing issues.
4. Writing large binary strings requiring copying all data from pipe(or `BytesReader` in opendal) to RAM, and then send to redis.

# Rationale and alternatives

## RedisJSON module

The [`RedisJSON`](https://redis.io/docs/stack/json/) module provides JSON support for Redis, and supports depth up to 128. Working on a JSON api could be easier than manually parsing or deserializing from `HASH`.

Since `bincode` also offers the ability of deserializing and serializing, `RedisJSON` won't be used.

# Prior art

None

# Unresolved questions

None

# Future possibilities

The implementation proposed here is far from perfect.

- The data organization could be optimized to make it acts more like a filesystem
- Making a customized redis module to calculate metadata on redis side
- Wait for stable of `bincode` 2.0, and bump to it.
