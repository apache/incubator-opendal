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

use std::ffi::{c_char, CString};

use ::opendal as core;

use crate::opendal_error;

/// A native logging callback. Levels are DEBUG=0 (including TRACE), INFO=1,
/// WARN=2, and ERROR=3; these values are not glog severity values.
///
/// File and message are non-null, NUL-terminated strings borrowed only for the
/// callback invocation. Copy any value retained after the callback returns.
/// Embedded NUL bytes are rendered as `\0`. An unavailable file is empty and
/// an unavailable line is zero.
///
/// The callback must remain valid for the lifetime of the program, support
/// concurrent calls, and not unwind through the C ABI. Recursive logging from
/// the callback is not supported.
pub type opendal_log_callback = Option<
    unsafe extern "C" fn(level: i32, file: *const c_char, line: u32, message: *const c_char),
>;

/// \brief Installs one process-global native callback for the library's Rust log facade.
///
/// The default C operator constructor emits operation events through this
/// callback. The explicit `opendal_operator_new_with_layers` constructor applies
/// only its supplied layers. The callback can also receive records from other
/// Rust components using the same log facade.
///
/// Returns NULL on success. A null callback or an already installed logger
/// returns OPENDAL_CONFIG_INVALID; free that error with `opendal_error_free`.
/// A registration failure leaves the installed logger and its level unchanged.
/// This function cannot replace or unregister a logger.
///
/// # Safety
///
/// A non-null callback must satisfy the lifetime and concurrency requirements
/// documented on `opendal_log_callback`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opendal_logging_init(
    callback: opendal_log_callback,
) -> *mut opendal_error {
    let Some(callback) = callback else {
        return opendal_error::new(core::Error::new(
            core::ErrorKind::ConfigInvalid,
            "native logging callback must not be null",
        ));
    };
    if let Err(error) = log::set_boxed_logger(Box::new(NativeLogger { callback })) {
        return opendal_error::new(
            core::Error::new(
                core::ErrorKind::ConfigInvalid,
                "a process-global Rust logger is already installed",
            )
            .set_source(error),
        );
    }
    log::set_max_level(log::LevelFilter::Trace);
    std::ptr::null_mut()
}

struct NativeLogger {
    callback: unsafe extern "C" fn(i32, *const c_char, u32, *const c_char),
}

impl log::Log for NativeLogger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        let level = match record.level() {
            log::Level::Trace | log::Level::Debug => 0,
            log::Level::Info => 1,
            log::Level::Warn => 2,
            log::Level::Error => 3,
        };
        let file = callback_string(record.file().unwrap_or_default());
        let message = callback_string(&record.args().to_string());
        // The initializer requires a program-lifetime, thread-safe callback.
        // These strings remain owned here until the foreign call returns.
        unsafe {
            (self.callback)(
                level,
                file.as_ptr(),
                record.line().unwrap_or_default(),
                message.as_ptr(),
            );
        }
    }

    fn flush(&self) {}
}

fn callback_string(value: &str) -> CString {
    // Escape NUL rather than truncating the message at the C boundary.
    CString::new(value.replace('\0', "\\0")).unwrap_or_default()
}
