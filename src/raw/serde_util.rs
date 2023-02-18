// Copyright 2023 Datafuse Labs.
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

use crate::*;

/// Parse xml deserialize error into opendal::Error.
pub fn parse_xml_deserialize_error(e: quick_xml::DeError) -> Error {
    Error::new(ErrorKind::Unexpected, "deserialize xml").set_source(e)
}

/// Parse json serialize error into opendal::Error.
pub fn parse_json_serialize_error(e: serde_json::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "serialize json").set_source(e)
}

/// Parse json deserialize error into opendal::Error.
pub fn parse_json_deserialize_error(e: serde_json::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "deserialize json").set_source(e)
}
