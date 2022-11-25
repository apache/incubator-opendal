// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// Args for `write` operation.
#[derive(Debug, Clone, Default)]
pub struct OpWrite {
    size: u64,
    content_type: Option<String>,
}

impl OpWrite {
    /// Create a new `OpWrite`.
    ///
    /// If input path is not a file path, an error will be returned.
    pub fn new(size: u64) -> Self {
        Self {
            size,
            content_type: None,
        }
    }

    /// Set the content type of option
    pub fn with_content_type(self, content_type: &str) -> Self {
        Self {
            size: self.size(),
            content_type: Some(content_type.to_string()),
        }
    }

    /// Get size from option.
    pub fn size(&self) -> u64 {
        self.size
    }
    /// Get the content type from option
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }
}

/// Reply for `write` operation.
#[derive(Debug, Clone, Default)]
pub struct RpWrite {
    written: u64,
}

impl RpWrite {
    /// Create a new reply for write.
    pub fn new(written: u64) -> Self {
        Self { written }
    }

    /// Get the written size (in bytes) of write operation.
    pub fn written(&self) -> u64 {
        self.written
    }
}
