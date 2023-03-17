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

export class Operator {
  constructor(scheme: string, options?: Record<string, string> | undefined | null)
  /** Get current path's metadata **without cache** directly. */
  stat(path: string): Promise<Metadata>
  /** Get current path's metadata **without cache** directly and synchronously. */
  statSync(path: string): Metadata
  /** Create dir with given path. */
  createDir(path: string): Promise<void>
  /** Create dir with given path synchronously. */
  createDirSync(path: string): void
  /** Write bytes into path. */
  write(path: string, content: Buffer | string): Promise<void>
  /** Write bytes into path synchronously. */
  writeSync(path: string, content: Buffer | string): void
  /** Read the whole path into a buffer. */
  read(path: string): Promise<Buffer>
  /** Read the whole path into a buffer synchronously. */
  readSync(path: string): Buffer
  /** List dir in flat way. */
  scan(path: string): Promise<Lister>
  /** List dir in flat way synchronously. */
  scanSync(path: string): BlockingLister
  /** Delete the given path. */
  delete(path: string): Promise<void>
  /** Delete the given path synchronously. */
  deleteSync(path: string): void
  /**
   * List given path.
   *
   * This function will create a new handle to list entries.
   *
   * An error will be returned if given path doesn't end with `/`.
   */
  list(path: string): Promise<Lister>
  /**
   * List given path synchronously.
   *
   * This function will create a new handle to list entries.
   *
   * An error will be returned if given path doesn't end with `/`.
   */
  listSync(path: string): BlockingLister
}
export class Entry {
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
