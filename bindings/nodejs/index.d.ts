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

export interface PresignedRequest {
  /** HTTP method of this request. */
  method: string
  /** URL of this request. */
  url: string
  /** HTTP headers of this request. */
  headers: Record<string, string>
}
export class Operator {
  constructor(scheme: string, options?: Record<string, string> | undefined | null)
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
   * Write bytes into path.
   *
   * ### Example
   * ```javascript
   * await op.write("path/to/file", Buffer.from("hello world"));
   * // or
   * await op.write("path/to/file", "hello world");
   * ```
   */
  write(path: string, content: Buffer | string): Promise<void>
  /**
   * Write bytes into path synchronously.
   *
   * ### Example
   * ```javascript
   * op.writeSync("path/to/file", Buffer.from("hello world"));
   * // or
   * op.writeSync("path/to/file", "hello world");
   * ```
   */
  writeSync(path: string, content: Buffer | string): void
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
   * Read the whole path into a buffer synchronously.
   *
   * ### Example
   * ```javascript
   * const buf = op.readSync("path/to/file");
   * ```
   */
  readSync(path: string): Buffer
  /**
   * List dir in flat way.
   *
   * This function will create a new handle to list entries.
   *
   * An error will be returned if given path doesn’t end with /.
   *
   * ### Example
   * ```javascript
   * const lister = await op.scan("/path/to/dir/");
   * while (true)) {
   *   const entry = await lister.next();
   *   if (entry === null) {
   *     break;
   *   }
   *   let meta = await op.stat(entry.path);
   *   if (meta.is_file) {
   *     // do something
   *   }
   * }
   * `````
   */
  scan(path: string): Promise<Lister>
  /**
   * List dir in flat way synchronously.
   *
   * This function will create a new handle to list entries.
   *
   * An error will be returned if given path doesn’t end with /.
   *
   * ### Example
   * ```javascript
   * const lister = op.scan_sync(/path/to/dir/");
   * while (true)) {
   *   const entry = lister.next();
   *   if (entry === null) {
   *     break;
   *   }
   *   let meta = op.statSync(entry.path);
   *   if (meta.is_file) {
   *     // do something
   *   }
   * }
   * `````
   */
  scanSync(path: string): BlockingLister
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
   * This function will create a new handle to list entries.
   *
   * An error will be returned if given path doesn't end with `/`.
   *
   * ### Example
   * ```javascript
   * const lister = await op.list("path/to/dir/");
   * while (true)) {
   *   const entry = await lister.next();
   *   if (entry === null) {
   *     break;
   *   }
   *   let meta = await op.stat(entry.path);
   *   if (meta.isFile) {
   *     // do something
   *   }
   * }
   * ```
   */
  list(path: string): Promise<Lister>
  /**
   * List given path synchronously.
   *
   * This function will create a new handle to list entries.
   *
   * An error will be returned if given path doesn't end with `/`.
   *
   * ### Example
   * ```javascript
   * const lister = op.listSync("path/to/dir/");
   * while (true)) {
   *   const entry = lister.next();
   *   if (entry === null) {
   *     break;
   *   }
   *   let meta = op.statSync(entry.path);
   *   if (meta.isFile) {
   *     // do something
   *   }
   * }
   * ```
   */
  listSync(path: string): BlockingLister
  /**
   * Get a presigned request for read.
   *
   * Unit of expires is seconds.
   */
  presignRead(path: string, expires: number): PresignedRequest
  /**
   * Get a presigned request for write.
   *
   * Unit of expires is seconds.
   */
  presignWrite(path: string, expires: number): PresignedRequest
  /**
   * Get a presigned request for stat.
   *
   * Unit of expires is seconds.
   */
  presignStat(path: string, expires: number): PresignedRequest
}
export class Entry {
  /** Return the path of this entry. */
  path(): string
}
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
  /** Last Modified of this object.(UTC) */
  get lastModified(): string | null
}
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
export class BlockingLister {
  next(): Entry | null
}
