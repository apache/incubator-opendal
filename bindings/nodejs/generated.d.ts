/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export class ExternalObject<T> {
  readonly '': {
    readonly '': unique symbol
    [K: symbol]: T
  }
}
export interface ListOptions {
  limit?: number
  recursive?: boolean
}
export interface OpWriteOptions {
  /** Append bytes into file. */
  append?: boolean
  /**
   * Set the chunk of op.
   *
   * If chunk is set, the data will be chunked by the underlying writer.
   *
   * ## NOTE
   *
   * Service could have their own minimum chunk size while perform write
   * operations like multipart uploads. So the chunk size may be larger than
   * the given buffer size.
   */
  chunk?: bigint
  /** Set the [Content-Type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) of op. */
  contentType?: string
  /** Set the [Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition) of op. */
  contentDisposition?: string
  /** Set the [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) of op. */
  cacheControl?: string
}
/** PresignedRequest is a presigned request return by `presign`. */
export interface PresignedRequest {
  /** HTTP method of this request. */
  method: string
  /** URL of this request. */
  url: string
  /** HTTP headers of this request. */
  headers: Record<string, string>
}
/**
 * Capability is used to describe what operations are supported
 * by current Operator.
 *
 * Via capability, we can know:
 *
 * - Whether current Operator supports read or not.
 * - Whether current Operator supports read with if match or not.
 * - What's current Operator max supports batch operations count.
 *
 * Add fields of Capabilities with be public and can be accessed directly.
 */
export class Capability {
  /** If operator supports stat. */
  get stat(): boolean
  /** If operator supports stat with if match. */
  get statWithIfMatch(): boolean
  /** If operator supports stat with if none match. */
  get statWithIfNoneMatch(): boolean
  /** If operator supports read. */
  get read(): boolean
  /** If operator supports read with if match. */
  get readWithIfMatch(): boolean
  /** If operator supports read with if none match. */
  get readWithIfNoneMatch(): boolean
  /** if operator supports read with override cache control. */
  get readWithOverrideCacheControl(): boolean
  /** if operator supports read with override content disposition. */
  get readWithOverrideContentDisposition(): boolean
  /** if operator supports read with override content type. */
  get readWithOverrideContentType(): boolean
  /** If operator supports write. */
  get write(): boolean
  /** If operator supports write can be called in multi times. */
  get writeCanMulti(): boolean
  /** If operator supports write with empty content. */
  get writeCanEmpty(): boolean
  /** If operator supports write by append. */
  get writeCanAppend(): boolean
  /** If operator supports write with content type. */
  get writeWithContentType(): boolean
  /** If operator supports write with content disposition. */
  get writeWithContentDisposition(): boolean
  /** If operator supports write with cache control. */
  get writeWithCacheControl(): boolean
  /**
   * write_multi_max_size is the max size that services support in write_multi.
   *
   * For example, AWS S3 supports 5GiB as max in write_multi.
   */
  get writeMultiMaxSize(): bigint | null
  /**
   * write_multi_min_size is the min size that services support in write_multi.
   *
   * For example, AWS S3 requires at least 5MiB in write_multi expect the last one.
   */
  get writeMultiMinSize(): bigint | null
  /**
   * write_multi_align_size is the align size that services required in write_multi.
   *
   * For example, Google GCS requires align size to 256KiB in write_multi.
   */
  get writeMultiAlignSize(): bigint | null
  /**
   * write_total_max_size is the max size that services support in write_total.
   *
   * For example, Cloudflare D1 supports 1MB as max in write_total.
   */
  get writeTotalMaxSize(): bigint | null
  /** If operator supports create dir. */
  get createDir(): boolean
  /** If operator supports delete. */
  get delete(): boolean
  /** If operator supports copy. */
  get copy(): boolean
  /** If operator supports rename. */
  get rename(): boolean
  /** If operator supports list. */
  get list(): boolean
  /** If backend supports list with limit. */
  get listWithLimit(): boolean
  /** If backend supports list with start after. */
  get listWithStartAfter(): boolean
  /** If backend supports list with recursive. */
  get listWithRecursive(): boolean
  /** If operator supports presign. */
  get presign(): boolean
  /** If operator supports presign read. */
  get presignRead(): boolean
  /** If operator supports presign stat. */
  get presignStat(): boolean
  /** If operator supports presign write. */
  get presignWrite(): boolean
  /** If operator supports batch. */
  get batch(): boolean
  /** If operator supports batch delete. */
  get batchDelete(): boolean
  /** The max operations that operator supports in batch. */
  get batchMaxOperations(): bigint | null
  /** If operator supports blocking. */
  get blocking(): boolean
}
export class Operator {
  /** @see For a detailed definition of scheme, see https://opendal.apache.org/docs/category/services */
  constructor(scheme: string, options?: Record<string, string> | undefined | null)
  /** Get current operator(service)'s full capability. */
  capability(): Capability
  /**
   * Get current path's metadata **without cache** directly.
   *
   * ### Notes
   * Use stat if you:
   *
   * - Want detect the outside changes of path.
   * - Don’t want to read from cached metadata.
   *
   * You may want to use `metadata` if you are working with entries returned by `Lister`. It’s highly possible that metadata you want has already been cached.
   *
   * ### Example
   * ```javascript
   * const meta = await op.stat("test");
   * if (meta.isDir) {
   *   // do something
   * }
   * ```
   */
  stat(path: string): Promise<Metadata>
  /**
   * Get current path's metadata **without cache** directly and synchronously.
   *
   * ### Example
   * ```javascript
   * const meta = op.statSync("test");
   * if (meta.isDir) {
   *   // do something
   * }
   * ```
   */
  statSync(path: string): Metadata
  /**
   * Check if this operator can work correctly.
   *
   * We will send a `list` request to path and return any errors we met.
   *
   * ### Example
   * ```javascript
   * await op.check();
   * ```
   */
  check(): Promise<void>
  /**
   * Check if this path exists or not.
   *
   * ### Example
   * ```javascript
   * await op.isExist("test");
   * ```
   */
  isExist(path: string): Promise<boolean>
  /**
   * Check if this path exists or not synchronously.
   *
   * ### Example
   * ```javascript
   * op.isExistSync("test");
   * ```
   */
  isExistSync(path: string): boolean
  /**
   * Create dir with given path.
   *
   * ### Example
   * ```javascript
   * await op.createDir("path/to/dir/");
   * ```
   */
  createDir(path: string): Promise<void>
  /**
   * Create dir with given path synchronously.
   *
   * ### Example
   * ```javascript
   * op.createDirSync("path/to/dir/");
   * ```
   */
  createDirSync(path: string): void
  /**
   * Read the whole path into a buffer.
   *
   * ### Example
   * ```javascript
   * const buf = await op.read("path/to/file");
   * ```
   */
  read(path: string): Promise<Buffer>
  /**
   * Create a reader to read the given path.
   *
   * It could be used to read large file in a streaming way.
   */
  reader(path: string): Promise<Reader>
  /**
   * Read the whole path into a buffer synchronously.
   *
   * ### Example
   * ```javascript
   * const buf = op.readSync("path/to/file");
   * ```
   */
  readSync(path: string): Buffer
  /**
   * Create a reader to read the given path synchronously.
   *
   * It could be used to read large file in a streaming way.
   */
  readerSync(path: string): BlockingReader
  /**
   * Write bytes into path.
   *
   * ### Example
   * ```javascript
   * await op.write("path/to/file", Buffer.from("hello world"));
   * // or
   * await op.write("path/to/file", "hello world");
   * // or
   * await op.write("path/to/file", Buffer.from("hello world"), { contentType: "text/plain" });
   * ```
   */
  write(path: string, content: Buffer | string, options?: OpWriteOptions | undefined | null): Promise<void>
  /**
   * Write multiple bytes into path.
   *
   * It could be used to write large file in a streaming way.
   */
  writer(path: string, options?: OpWriteOptions | undefined | null): Promise<Writer>
  /**
   * Write multiple bytes into path synchronously.
   *
   * It could be used to write large file in a streaming way.
   */
  writerSync(path: string, options?: OpWriteOptions | undefined | null): BlockingWriter
  /**
   * Write bytes into path synchronously.
   *
   * ### Example
   * ```javascript
   * op.writeSync("path/to/file", Buffer.from("hello world"));
   * // or
   * op.writeSync("path/to/file", "hello world");
   * // or
   * op.writeSync("path/to/file", Buffer.from("hello world"), { contentType: "text/plain" });
   * ```
   */
  writeSync(path: string, content: Buffer | string, options?: OpWriteOptions | undefined | null): void
  /**
   * Append bytes into path.
   *
   * ### Notes
   *
   * - It always appends content to the end of the file.
   * - It will create file if the path not exists.
   *
   * ### Example
   * ```javascript
   * await op.append("path/to/file", Buffer.from("hello world"));
   * // or
   * await op.append("path/to/file", "hello world");
   * ```
   */
  append(path: string, content: Buffer | string): Promise<void>
  /**
   * Copy file according to given `from` and `to` path.
   *
   * ### Example
   * ```javascript
   * await op.copy("path/to/file", "path/to/dest");
   * ```
   */
  copy(from: string, to: string): Promise<void>
  /**
   * Copy file according to given `from` and `to` path synchronously.
   *
   * ### Example
   * ```javascript
   * op.copySync("path/to/file", "path/to/dest");
   * ```
   */
  copySync(from: string, to: string): void
  /**
   * Rename file according to given `from` and `to` path.
   *
   * It's similar to `mv` command.
   *
   * ### Example
   * ```javascript
   * await op.rename("path/to/file", "path/to/dest");
   * ```
   */
  rename(from: string, to: string): Promise<void>
  /**
   * Rename file according to given `from` and `to` path synchronously.
   *
   * It's similar to `mv` command.
   *
   * ### Example
   * ```javascript
   * op.renameSync("path/to/file", "path/to/dest");
   * ```
   */
  renameSync(from: string, to: string): void
  /**
   * Delete the given path.
   *
   * ### Notes
   * Delete not existing error won’t return errors.
   *
   * ### Example
   * ```javascript
   * await op.delete("test");
   * ```
   */
  delete(path: string): Promise<void>
  /**
   * Delete the given path synchronously.
   *
   * ### Example
   * ```javascript
   * op.deleteSync("test");
   * ```
   */
  deleteSync(path: string): void
  /**
   * Remove given paths.
   *
   * ### Notes
   * If underlying services support delete in batch, we will use batch delete instead.
   *
   * ### Examples
   * ```javascript
   * await op.remove(["abc", "def"]);
   * ```
   */
  remove(paths: Array<string>): Promise<void>
  /**
   * Remove the path and all nested dirs and files recursively.
   *
   * ### Notes
   * If underlying services support delete in batch, we will use batch delete instead.
   *
   * ### Examples
   * ```javascript
   * await op.removeAll("path/to/dir/");
   * ```
   */
  removeAll(path: string): Promise<void>
  /**
   * List given path.
   *
   * This function will return an array of entries.
   *
   * An error will be returned if given path doesn't end with `/`.
   *
   * ### Example
   *
   * ```javascript
   * const list = await op.list("path/to/dir/");
   * for (let entry of list) {
   *   let meta = await op.stat(entry.path);
   *   if (meta.isFile) {
   *     // do something
   *   }
   * }
   * ```
   *
   * #### List recursively
   *
   * With `recursive` option, you can list recursively.
   *
   * ```javascript
   * const list = await op.list("path/to/dir/", { recursive: true });
   * for (let entry of list) {
   *   let meta = await op.stat(entry.path);
   *   if (meta.isFile) {
   *     // do something
   *   }
   * }
   * ```
   */
  list(path: string, options?: ListOptions | undefined | null): Promise<Array<Entry>>
  /**
   * List given path synchronously.
   *
   * This function will return a array of entries.
   *
   * An error will be returned if given path doesn't end with `/`.
   *
   * ### Example
   *
   * ```javascript
   * const list = op.listSync("path/to/dir/");
   * for (let entry of list) {
   *   let meta = op.statSync(entry.path);
   *   if (meta.isFile) {
   *     // do something
   *   }
   * }
   * ```
   *
   * #### List recursively
   *
   * With `recursive` option, you can list recursively.
   *
   * ```javascript
   * const list = op.listSync("path/to/dir/", { recursive: true });
   * for (let entry of list) {
   *   let meta = op.statSync(entry.path);
   *   if (meta.isFile) {
   *     // do something
   *   }
   * }
   * ```
   */
  listSync(path: string, options?: ListOptions | undefined | null): Array<Entry>
  /**
   * Get a presigned request for read.
   *
   * Unit of expires is seconds.
   *
   * ### Example
   *
   * ```javascript
   * const req = await op.presignRead(path, parseInt(expires));
   *
   * console.log("method: ", req.method);
   * console.log("url: ", req.url);
   * console.log("headers: ", req.headers);
   * ```
   */
  presignRead(path: string, expires: number): Promise<PresignedRequest>
  /**
   * Get a presigned request for write.
   *
   * Unit of expires is seconds.
   *
   * ### Example
   *
   * ```javascript
   * const req = await op.presignWrite(path, parseInt(expires));
   *
   * console.log("method: ", req.method);
   * console.log("url: ", req.url);
   * console.log("headers: ", req.headers);
   * ```
   */
  presignWrite(path: string, expires: number): Promise<PresignedRequest>
  /**
   * Get a presigned request for stat.
   *
   * Unit of expires is seconds.
   *
   * ### Example
   *
   * ```javascript
   * const req = await op.presignStat(path, parseInt(expires));
   *
   * console.log("method: ", req.method);
   * console.log("url: ", req.url);
   * console.log("headers: ", req.headers);
   * ```
   */
  presignStat(path: string, expires: number): Promise<PresignedRequest>
  /** Add a layer to this operator. */
  layer(layer: ExternalObject<Layer>): this
}
/** Entry returned by Lister or BlockingLister to represent a path and it's relative metadata. */
export class Entry {
  /** Return the path of this entry. */
  path(): string
}
/** Metadata carries all metadata associated with a path. */
export class Metadata {
  /** Returns true if the <op.stat> object describes a file system directory. */
  isDirectory(): boolean
  /** Returns true if the <op.stat> object describes a regular file. */
  isFile(): boolean
  /** Content-Disposition of this object */
  get contentDisposition(): string | null
  /** Content Length of this object */
  get contentLength(): bigint | null
  /** Content MD5 of this object. */
  get contentMd5(): string | null
  /** Content Type of this object. */
  get contentType(): string | null
  /** ETag of this object. */
  get etag(): string | null
  /**
   * Last Modified of this object.
   *
   * We will output this time in RFC3339 format like `1996-12-19T16:39:57+08:00`.
   */
  get lastModified(): string | null
}
/**
 * BlockingReader is designed to read data from given path in an blocking
 * manner.
 */
export class BlockingReader {
  read(buf: Buffer): bigint
}
/**
 * Reader is designed to read data from given path in an asynchronous
 * manner.
 */
export class Reader {
  /**
   * # Safety
   *
   * > &mut self in async napi methods should be marked as unsafe
   *
   * Read bytes from this reader into given buffer.
   */
  read(buf: Buffer): Promise<bigint>
}
/**
 * BlockingWriter is designed to write data into given path in an blocking
 * manner.
 */
export class BlockingWriter {
  /**
   * # Safety
   *
   * > &mut self in async napi methods should be marked as unsafe
   *
   * Write bytes into this writer.
   *
   * ### Example
   * ```javascript
   * const writer = await op.writer("path/to/file");
   * await writer.write(Buffer.from("hello world"));
   * await writer.close();
   * ```
   */
  write(content: Buffer | string): void
  /**
   * # Safety
   *
   * > &mut self in async napi methods should be marked as unsafe
   *
   * Close this writer.
   *
   * ### Example
   *
   * ```javascript
   * const writer = op.writerSync("path/to/file");
   * writer.write(Buffer.from("hello world"));
   * writer.close();
   * ```
   */
  close(): void
}
/**
 * Writer is designed to write data into given path in an asynchronous
 * manner.
 */
export class Writer {
  /**
   * # Safety
   *
   * > &mut self in async napi methods should be marked as unsafe
   *
   * Write bytes into this writer.
   *
   * ### Example
   * ```javascript
   * const writer = await op.writer("path/to/file");
   * await writer.write(Buffer.from("hello world"));
   * await writer.close();
   * ```
   */
  write(content: Buffer | string): Promise<void>
  /**
   * # Safety
   *
   * > &mut self in async napi methods should be marked as unsafe
   *
   * Close this writer.
   *
   * ### Example
   * ```javascript
   * const writer = await op.writer("path/to/file");
   * await writer.write(Buffer.from("hello world"));
   * await writer.close();
   * ```
   */
  close(): Promise<void>
}
/**
 * Lister is designed to list entries at given path in an asynchronous
 * manner.
 */
export class Lister {
  /**
   * # Safety
   *
   * > &mut self in async napi methods should be marked as unsafe
   *
   * napi will make sure the function is safe, and we didn't do unsafe
   * thing internally.
   */
  next(): Promise<Entry | null>
}
/**
 * BlockingLister is designed to list entries at given path in a blocking
 * manner.
 */
export class BlockingLister {
  next(): Entry | null
}
/** A public layer wrapper */
export class Layer { }
/**
 * Retry layer
 *
 * Add retry for temporary failed operations.
 *
 * # Notes
 *
 * This layer will retry failed operations when [`Error::is_temporary`]
 * returns true. If operation still failed, this layer will set error to
 * `Persistent` which means error has been retried.
 *
 * `write` and `blocking_write` don't support retry so far, visit [this issue](https://github.com/apache/opendal/issues/1223) for more details.
 *
 * # Examples
 *
 * ```javascript
 * const op = new Operator("file", { root: "/tmp" })
 *
 * const retry = new RetryLayer();
 * retry.max_times = 3;
 * retry.jitter = true;
 *
 * op.layer(retry.build());
 * ```
 */
export class RetryLayer {
  constructor()
  /**
   * Set jitter of current backoff.
   *
   * If jitter is enabled, ExponentialBackoff will add a random jitter in `[0, min_delay)
   * to current delay.
   */
  set jitter(v: boolean)
  /**
   * Set max_times of current backoff.
   *
   * Backoff will return `None` if max times is reaching.
   */
  set maxTimes(v: number)
  /**
   * Set factor of current backoff.
   *
   * # Panics
   *
   * This function will panic if input factor smaller than `1.0`.
   */
  set factor(v: number)
  /**
   * Set max_delay of current backoff.
   *
   * Delay will not increasing if current delay is larger than max_delay.
   *
   * # Notes
   *
   * - The unit of max_delay is millisecond.
   */
  set maxDelay(v: number)
  /**
   * Set min_delay of current backoff.
   *
   * # Notes
   *
   * - The unit of min_delay is millisecond.
   */
  set minDelay(v: number)
  build(): ExternalObject<Layer>
}
