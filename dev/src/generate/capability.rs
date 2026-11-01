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

//! Parser for `core/core/src/types/capability.rs`.
//!
//! Reads the `Capability` struct from opendal core and exposes its fields as
//! a flat list, so that binding generators can render language mirrors from
//! the single source of truth instead of hand-maintaining copies.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::Path;
use syn::{GenericArgument, Item, ItemStruct, PathArguments, Type, TypePath};

use super::options::doc_lines;

/// A single field of the core `Capability` struct.
///
/// Only the two shapes that actually occur in core are supported: plain
/// `bool` flags and `Option<usize>` limits. Anything else fails parsing so
/// that a new field shape upstream breaks the generator loudly instead of
/// producing a silently wrong mirror.
#[derive(Debug, Serialize)]
pub struct CapabilityField {
    /// Field name in snake_case, exactly as written in core.
    pub name: String,
    /// Whether the field is a plain `bool` flag (`false` means `Option<usize>`).
    pub is_bool: bool,
    /// Doc comment of the field, joined to a single line.
    pub doc: String,
}

/// Parse the `Capability` struct out of the given core source file.
pub fn parse(path: &Path) -> Result<Vec<CapabilityField>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let ast = syn::parse_file(&content)?;

    for item in ast.items {
        if let Item::Struct(struct_) = item
            && struct_.ident == "Capability"
        {
            return parse_struct(&struct_);
        }
    }

    bail!("struct Capability not found in {}", path.display())
}

fn parse_struct(struct_: &ItemStruct) -> Result<Vec<CapabilityField>> {
    let syn::Fields::Named(named) = &struct_.fields else {
        bail!("struct Capability must have named fields");
    };

    let mut fields = Vec::with_capacity(named.named.len());
    for field in &named.named {
        let name = field
            .ident
            .as_ref()
            .expect("named field always has an ident")
            .to_string();
        let is_bool = match parse_type(&field.ty) {
            Some(is_bool) => is_bool,
            None => bail!(
                "capability field `{name}` has an unsupported type; \
                 teach dev/src/generate/capability.rs about it before regenerating"
            ),
        };
        let doc = doc_lines(&field.attrs).join(" ").trim().to_string();
        fields.push(CapabilityField { name, is_bool, doc });
    }

    Ok(fields)
}

/// Returns `Some(true)` for `bool`, `Some(false)` for `Option<usize>`, and
/// `None` for anything else.
fn parse_type(ty: &Type) -> Option<bool> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;

    if segment.ident == "bool" {
        return Some(true);
    }

    if segment.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(Type::Path(inner))) = args.args.first()
        && inner.path.is_ident("usize")
    {
        return Some(false);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_core_capability() {
        let path = crate::workspace_dir().join("core/core/src/types/capability.rs");
        let fields = parse(&path).expect("core capability must parse");

        // The struct is large and every field must be either a flag or a
        // limit; spot-check both shapes and that docs came through.
        assert!(fields.len() > 50);
        let stat = fields.iter().find(|f| f.name == "stat").unwrap();
        assert!(stat.is_bool);
        assert!(!stat.doc.is_empty());
        let limit = fields
            .iter()
            .find(|f| f.name == "write_multi_max_size")
            .unwrap();
        assert!(!limit.is_bool);
    }
}
