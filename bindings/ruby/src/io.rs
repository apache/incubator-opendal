// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

#![allow(
    rustdoc::broken_intra_doc_links,
    reason = "YARD's syntax for documentation"
)]
#![allow(rustdoc::invalid_html_tags, reason = "YARD's syntax for documentation")]
#![allow(rustdoc::bare_urls, reason = "YARD's syntax for documentation")]

use std::cell::RefCell;
use std::io::BufRead;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;

use magnus::Error;
use magnus::RHash;
use magnus::RModule;
use magnus::RString;
use magnus::Value;
use magnus::io::FMode;
use magnus::method;
use magnus::scan_args::scan_args;

use crate::*;

// `Io` is the rust implementation for `OpenDal::IO`. `Io` follows similar Ruby IO classes, such as:
// - IO
// - StringIO
//
// `Io` is not exactly an `IO` but is unidirectional (either `Reader` or `Writer`).
// TODO: implement encoding.
//
/// @yard
/// `OpenDal::IO` is similar to Ruby's `IO` and `StringIO` for accessing files.
///
/// You can't create an instance of `OpenDal::IO` except using {OpenDal::Operator#open}.
///
/// Constraints:
/// - Only available for reading and writing
/// - Writing doesn't support seek.
#[magnus::wrap(class = "OpenDal::IO", free_immediately, size)]
pub struct Io(RefCell<IoHandle>);

enum FileState {
    Reader(ocore::blocking::StdReader),
    Writer(ocore::blocking::StdWriter),
    Closed,
}

struct IoHandle {
    state: FileState,
    fmode: FMode,
    #[allow(dead_code)]
    external_encoding_name: Option<String>,
    #[allow(dead_code)]
    internal_encoding_name: Option<String>,
    #[allow(dead_code)]
    encoding_flags: i32,
}

pub fn format_io_error(ruby: &Ruby, err: std::io::Error) -> Error {
    Error::new(ruby.exception_runtime_error(), err.to_string())
}

impl Io {
    /// Creates a new `OpenDal::IO` object in Ruby.
    ///
    /// See [`Operator::open`] for more information.
    pub fn new(
        ruby: &Ruby,
        operator: ocore::blocking::Operator,
        path: String,
        mut mode: Value,
        mut permission: Value,
        kwargs: RHash,
    ) -> Result<Self, Error> {
        let (_open_flags, fmode, encoding) =
            ruby.io_extract_modeenc(&mut mode, &mut permission, &kwargs)?;

        let state =
            // Create reader if mode supports reading
            if fmode.contains(FMode::READ) {
                FileState::Reader(
                    operator
                        .reader(&path)
                        .map_err(|err| format_magnus_error(ruby, err))?
                        .into_std_read(..)
                        .map_err(|err| format_magnus_error(ruby, err))?,
                )
            } else if fmode.contains(FMode::WRITE) {
                FileState::Writer(
                operator
                    .writer(&path)
                    .map_err(|err| format_magnus_error(ruby, err))?
                    .into_std_write(),
            )
        } else {
            return Err(Error::new(
                ruby.exception_arg_error(),
                "Invalid mode: must open for reading or writing",
            ));
        };

        Ok(Self(RefCell::new(IoHandle {
            state,
            fmode,
            external_encoding_name: encoding.external.map(|e| e.to_string()),
            internal_encoding_name: encoding.internal.map(|e| e.to_string()),
            encoding_flags: encoding.flags,
        })))
    }

    /// @yard
    /// @def binmode
    /// Enables binary mode for the stream.
    /// @return [nil]
    /// @raise [IOError] when operate on a closed stream
    fn binary_mode(ruby: &Ruby, rb_self: &Self) -> Result<(), Error> {
        let mut handle = rb_self.0.borrow_mut();
        if let FileState::Closed = handle.state {
            return Err(Error::new(ruby.exception_io_error(), "closed stream"));
        }
        handle.fmode = FMode::new(handle.fmode.bits() | FMode::BINARY_MODE);
        Ok(())
    }

    /// @yard
    /// @def binmode?
    /// Returns if the stream is on binary mode.
    /// @return [Boolean]
    /// @raise [IOError] when operate on a closed stream
    fn is_binary_mode(ruby: &Ruby, rb_self: &Self) -> Result<bool, Error> {
        let handle = rb_self.0.borrow();
        if let FileState::Closed = handle.state {
            return Err(Error::new(ruby.exception_io_error(), "closed stream"));
        }
        Ok(handle.fmode.contains(FMode::BINARY_MODE))
    }

    /// @yard
    /// @def close
    /// Close streams.
    /// @return [nil]
    fn close(ruby: &Ruby, rb_self: &Self) -> Result<(), Error> {
        let mut handle = rb_self.0.borrow_mut();
        if let FileState::Writer(writer) = &mut handle.state {
            writer.close().map_err(|err| format_io_error(ruby, err))?;
        }
        handle.state = FileState::Closed;
        Ok(())
    }

    /// @yard
    /// @def close_read
    /// Closes the read stream.
    /// @return [nil]
    fn close_read(&self) -> Result<(), Error> {
        let mut handle = self.0.borrow_mut();
        if let FileState::Reader(_) = &handle.state {
            handle.state = FileState::Closed;
        }
        Ok(())
    }

    /// @yard
    /// @def close_write
    /// Closes the write stream.
    /// @return [nil]
    fn close_write(ruby: &Ruby, rb_self: &Self) -> Result<(), Error> {
        let mut handle = rb_self.0.borrow_mut();
        if let FileState::Writer(writer) = &mut handle.state {
            writer.close().map_err(|err| format_io_error(ruby, err))?;
            handle.state = FileState::Closed;
        }
        Ok(())
    }

    /// @yard
    /// @def closed?
    /// Returns if streams are closed.
    /// @return [Boolean]
    fn is_closed(&self) -> Result<bool, Error> {
        let handle = self.0.borrow();
        Ok(matches!(handle.state, FileState::Closed))
    }

    /// @yard
    /// @def closed_read?
    /// Returns if the read stream is closed.
    /// @return [Boolean]
    fn is_closed_read(&self) -> Result<bool, Error> {
        let handle = self.0.borrow();
        Ok(!matches!(handle.state, FileState::Reader(_)))
    }

    /// @yard
    /// @def closed_write?
    /// Returns if the write stream is closed.
    /// @return [Boolean]
    fn is_closed_write(&self) -> Result<bool, Error> {
        let handle = self.0.borrow();
        Ok(!matches!(handle.state, FileState::Writer(_)))
    }
}

impl Io {
    /// Reads data from the stream.
    /// TODO:
    ///   - support encoding
    ///
    /// @param size [Integer, nil] The maximum number of bytes to read. Reads all data when not provided.
    /// @param buffer [String, nil] The output buffer to append to.
    fn read(ruby: &Ruby, rb_self: &Self, args: &[Value]) -> Result<Option<bytes::Bytes>, Error> {
        let args = scan_args::<(), (Option<Option<i64>>, Option<RString>), (), (), (), ()>(args)?;
        let (option_size, mut option_output_buffer) = args.optional;
        let size = option_size.unwrap_or_default(); // allow nil

        let mut handle = rb_self.0.borrow_mut();

        if let FileState::Reader(reader) = &mut handle.state {
            let buffer: Option<bytes::Bytes> = match size {
                Some(size) => {
                    if size <= 0 {
                        return Err(Error::new(
                            ruby.exception_arg_error(),
                            format!("negative length {} given", size),
                        ));
                    }
                    let mut bs = vec![0; size as usize];
                    let n = reader
                        .read(&mut bs)
                        .map_err(|err| format_io_error(ruby, err))?;
                    if n == 0 && size > 0 {
                        // when called at end of file, read(positive_integer) returns nil.
                        None
                    } else {
                        bs.truncate(n);
                        Some(bs.into())
                    }
                }
                None => {
                    let mut buffer = Vec::new();
                    reader
                        .read_to_end(&mut buffer)
                        .map_err(|err| format_io_error(ruby, err))?;
                    Some(buffer.into())
                }
            };

            // when provided the buffer parameter, append read buffer
            if let (Some(output_buffer), Some(inner)) =
                (option_output_buffer.as_mut(), buffer.as_ref())
            {
                output_buffer.cat(inner);
            }

            Ok(buffer)
        } else {
            Err(Error::new(
                ruby.exception_runtime_error(),
                "I/O operation failed for reading on write-only file.",
            ))
        }
    }

    /// @yard
    /// @def readline
    /// Reads a single line from the stream.
    /// @return [String]
    // TODO: extend readline with parameters
    fn readline(ruby: &Ruby, rb_self: &Self) -> Result<String, Error> {
        let mut handle = rb_self.0.borrow_mut();

        if let FileState::Reader(reader) = &mut handle.state {
            let mut buffer = String::new();
            let size = reader
                .read_line(&mut buffer)
                .map_err(|err| format_io_error(ruby, err))?;
            if size == 0 {
                return Err(Error::new(
                    ruby.exception_eof_error(),
                    "end of file reached",
                ));
            }

            Ok(buffer)
        } else {
            Err(Error::new(
                ruby.exception_runtime_error(),
                "I/O operation failed for reading on write-only file.",
            ))
        }
    }

    /// @yard
    /// @def write(buffer)
    /// Writes data to the stream.
    /// @param buffer [String]
    /// @return [Integer] the written byte size
    fn write(ruby: &Ruby, rb_self: &Self, bs: String) -> Result<usize, Error> {
        let mut handle = rb_self.0.borrow_mut();

        if let FileState::Writer(writer) = &mut handle.state {
            let bytes_written = bs.len();
            writer
                .write_all(bs.as_bytes())
                .map_err(|err| format_io_error(ruby, err))?;
            Ok(bytes_written)
        } else {
            Err(Error::new(
                ruby.exception_runtime_error(),
                "I/O operation failed for writing on read-only file.",
            ))
        }
    }
}

impl Io {
    /// @yard
    /// @def seek(offset, whence)
    /// Moves the file position based on the offset and whence.
    /// @param offset [Integer] The position offset.
    /// @param whence [Integer] The reference point:
    ///   - 0 = IO:SEEK_SET (Start)
    ///   - 1 = IO:SEEK_CUR (Current position)
    ///   - 2 = IO:SEEK_END (From the end)
    ///
    /// @return [Integer] always 0 if the seek operation is successful
    fn seek(ruby: &Ruby, rb_self: &Self, offset: i64, whence: u8) -> Result<u8, Error> {
        let mut handle = rb_self.0.borrow_mut();

        if let FileState::Reader(reader) = &mut handle.state {
            // Calculate the new position first
            let new_reader_position = match whence {
                0 => {
                    // SEEK_SET - absolute position
                    if offset < 0 {
                        return Err(Error::new(
                            ruby.exception_runtime_error(),
                            "Cannot seek to negative reader_position.",
                        ));
                    }
                    offset as u64
                }
                1 => {
                    // SEEK_CUR - relative to current position
                    let position = reader
                        .stream_position()
                        .map_err(|err| format_io_error(ruby, err))?;
                    if offset < 0 && (-offset as u64) > position {
                        return Err(Error::new(
                            ruby.exception_runtime_error(),
                            "Cannot seek before start of stream.",
                        ));
                    }
                    (position as i64 + offset) as u64
                }
                2 => {
                    // SEEK_END
                    let end_pos = reader
                        .seek(SeekFrom::End(0))
                        .map_err(|err| format_io_error(ruby, err))?;
                    if offset < 0 && (-offset as u64) > end_pos {
                        return Err(Error::new(
                            ruby.exception_runtime_error(),
                            "Cannot seek before start of stream.",
                        ));
                    }
                    (end_pos as i64 + offset) as u64
                }
                _ => return Err(Error::new(ruby.exception_arg_error(), "invalid whence")),
            };

            let _ = reader.seek(std::io::SeekFrom::Start(new_reader_position));
        } else {
            return Err(Error::new(
                ruby.exception_runtime_error(),
                "Cannot seek from end on write-only stream.",
            ));
        }

        Ok(0)
    }

    /// @yard
    /// @def tell
    /// Returns the current reader_position of the file pointer in the stream.
    /// @return [Integer] the current reader_position in bytes
    /// @raise [IOError] when cannot operate on the operation mode
    fn tell(ruby: &Ruby, rb_self: &Self) -> Result<u64, Error> {
        let mut handle = rb_self.0.borrow_mut();

        match &mut handle.state {
            FileState::Reader(reader) => Ok(reader
                .stream_position()
                .map_err(|err| format_io_error(ruby, err))?),
            FileState::Writer(_) => Err(Error::new(
                ruby.exception_runtime_error(),
                "I/O operation failed for reading on write only file.",
            )),
            FileState::Closed => Err(Error::new(
                ruby.exception_runtime_error(),
                "I/O operation failed for tell on closed stream.",
            )),
        }
    }

    // TODO: consider implement:
    // - lineno
    // - set_lineno
    // - getc
    // - putc
    // - gets
    // - puts
}

/// Defines the `OpenDal::IO` class in the given Ruby module and binds its methods.
///
/// This function uses Magnus's built-in Ruby thread-safety features to define the
/// `OpenDal::IO` class and its methods in the provided Ruby module (`gem_module`).
///
/// # Ruby Object Lifetime and Safety
///
/// Ruby objects can only exist in the Ruby heap and are tracked by Ruby's garbage collector (GC).
/// While we can allocate and store Ruby-related objects in the Rust heap, Magnus does not
/// automatically track such objects. Therefore, it is critical to work within Magnus's safety
/// guidelines when integrating Rust objects with Ruby. Read more in the Magnus documentation:
/// [Magnus Safety Documentation](https://github.com/matsadler/magnus#safety).
pub fn include(ruby: &Ruby, gem_module: &RModule) -> Result<(), Error> {
    let class = gem_module.define_class("IO", ruby.class_object())?;
    class.define_method("binmode", method!(Io::binary_mode, 0))?;
    class.define_method("binmode?", method!(Io::is_binary_mode, 0))?;
    class.define_method("close", method!(Io::close, 0))?;
    class.define_method("close_read", method!(Io::close_read, 0))?;
    class.define_method("close_write", method!(Io::close_write, 0))?;
    class.define_method("closed?", method!(Io::is_closed, 0))?;
    class.define_method("closed_read?", method!(Io::is_closed_read, 0))?;
    class.define_method("closed_write?", method!(Io::is_closed_write, 0))?;
    class.define_method("read", method!(Io::read, -1))?;
    class.define_method("write", method!(Io::write, 1))?;
    class.define_method("readline", method!(Io::readline, 0))?;
    class.define_method("seek", method!(Io::seek, 2))?;
    class.define_method("tell", method!(Io::tell, 0))?;

    Ok(())
}
