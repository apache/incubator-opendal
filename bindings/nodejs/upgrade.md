# Upgrade to v0.48

## Breaking change

### Public API

Now, nodejs binding `op.is_exist` changed to `op.exists` to align with nodejs API style.

# Upgrade to v0.47

## Breaking change

### Public API

Now, the `append` operation has been removed. You can use below code instead.

```js
op.write("path/to/file", Buffer.from("hello world"), { append: true });
```

# Upgrade to v0.44

## Breaking change

### Services

Because of [a TLS lib issue](https://github.com/apache/opendal/issues/3650), we temporarily disable the `services-ftp` feature.

### Public API

Now, the `list` operation returns `Array<Entry>` instead of a lister.
Also, we removed `scan`, you can use `list('some/path', {recursive: true})`/`listSync('some/path', {recursive: true})` instead of `scan('some/path')`/`scanSync('some/path')`.
