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

/// <reference types="node" />

const { Writable, Readable } = require('stream')

class ReadStream extends Readable {
  constructor(reader, options) {
    super(options)
    this.reader = reader
  }

  _read(size) {
    const buf = Buffer.alloc(size)
    this.reader
      .read(buf)
      .then((s) => {
        if (s === 0n) {
          this.push(null)
        } else {
          this.push(buf.subarray(0, Number(s)))
        }
      })
      .catch((e) => {
        this.emit('error', e)
      })
  }
}

class BlockingReadStream extends Readable {
  constructor(reader, options) {
    super(options)
    this.reader = reader
  }

  _read(size) {
    try {
      const buf = Buffer.alloc(size)
      let s = this.reader.read(buf)
      if (s === 0n) {
        this.push(null)
      } else {
        this.push(buf.subarray(0, Number(s)))
      }
    } catch (e) {
      this.emit('error', e)
    }
  }
}

class WriteStream extends Writable {
  constructor(writer, options) {
    super(options)
    this.writer = writer
  }

  _write(chunk, encoding, callback) {
    this.writer
      .write(chunk)
      .then(() => {
        callback()
      })
      .catch((e) => {
        callback(e)
      })
  }

  _final(callback) {
    this.writer
      .close()
      .then(() => {
        callback()
      })
      .catch((e) => {
        callback(e)
      })
  }
}

class BlockingWriteStream extends Writable {
  constructor(writer, options) {
    super(options)
    this.writer = writer
  }

  _write(chunk, encoding, callback) {
    try {
      this.writer.write(chunk)
      callback()
    } catch (e) {
      callback(e)
    }
  }

  _final(callback) {
    try {
      this.writer.close()
      callback()
    } catch (e) {
      callback(e)
    }
  }
}

const {
  Operator,
  RetryLayer,
  ConcurrentLimitLayer,
  BlockingReader,
  Reader,
  BlockingWriter,
  Writer,
} = require('./generated.js')

BlockingReader.prototype.createReadStream = function (options) {
  return new BlockingReadStream(this, options)
}

Reader.prototype.createReadStream = function (options) {
  return new ReadStream(this, options)
}

BlockingWriter.prototype.createWriteStream = function (options) {
  return new BlockingWriteStream(this, options)
}

Writer.prototype.createWriteStream = function (options) {
  return new WriteStream(this, options)
}

module.exports.Operator = Operator
module.exports.layers = {
  RetryLayer,
  ConcurrentLimitLayer,
}
