/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export const enum ObjectMode {
  /** FILE means the object has data to read. */
  FILE = 0,
  /** DIR means the object can be listed. */
  DIR = 1,
  /** Unknown means we don't know what we can do on this object. */
  Unknown = 2
}
export class Memory {
  constructor()
  build(): Operator
}
export class Operator {
  object(path: string): object
}
export class ObjectMeta {
  location: string
  lastModified: number
  size: number
}
export class ObjectMetadata {
  /** Mode of this object. */
  mode: ObjectMode
  /** Content-Disposition of this object */
  contentDisposition?: string
  /** Content Length of this object */
  contentLength?: number
  /** Content MD5 of this object. */
  contentMd5?: string
  /** Content Range of this object. */
  contentRange?: Array<number>
  /** Content Type of this object. */
  contentType?: string
  /** ETag of this object. */
  etag?: string
  /** Last Modified of this object. */
  lastModified: number
}
export class Object {
  stat(): Promise<ObjectMetadata>
  statSync(): ObjectMetadata
  write(content: Buffer): Promise<void>
  writeSync(content: Buffer): void
  read(): Promise<Buffer>
  readSync(): Buffer
  delete(): Promise<void>
  deleteSync(): void
}
