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

//! Parser for `core/core/src/types/options.rs`.
//!
//! Reads the public `*Options` structs from opendal core and classifies each
//! field type into the handful of shapes binding generators know how to
//! carry, so bindings can mirror the operation options from the single source
//! of truth instead of hand-maintaining copies.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::Path;
use syn::{GenericArgument, Item, ItemStruct, PathArguments, Type, TypePath, Visibility};

use super::options::doc_lines;

/// The shape of one option field.
///
/// Only the shapes that actually occur in core are supported. Anything else
/// fails parsing so that a new field shape upstream breaks the generator
/// loudly instead of producing a silently wrong mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    /// `bool`
    Bool,
    /// `usize`
    Usize,
    /// `Option<usize>`
    OptionUsize,
    /// `Option<u64>`
    OptionU64,
    /// `Option<String>`
    OptionString,
    /// `Option<Timestamp>`
    OptionTimestamp,
    /// `Option<HashMap<String, String>>`
    OptionStringMap,
    /// `Option<Metadata>`
    OptionMetadata,
    /// `BytesRange`
    BytesRange,
}

/// One field of a core options struct.
#[derive(Debug, Clone)]
pub struct OptionField {
    /// Field name in snake_case, exactly as written in core.
    pub name: String,
    /// The shape of the field type.
    pub kind: OptionKind,
    /// Doc comment of the field, one entry per line.
    pub doc: Vec<String>,
}

/// One public `*Options` struct of core.
#[derive(Debug, Clone)]
pub struct OptionsStruct {
    /// The struct name, e.g. `ReadOptions`.
    pub name: String,
    /// Doc comment of the struct, one entry per line.
    pub doc: Vec<String>,
    /// The fields in declaration order.
    pub fields: Vec<OptionField>,
}

/// Parse every public `*Options` struct out of the given core source file.
pub fn parse(path: &Path) -> Result<Vec<OptionsStruct>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let ast = syn::parse_file(&content)?;

    let mut structs = Vec::new();
    for item in ast.items {
        if let Item::Struct(struct_) = item
            && matches!(struct_.vis, Visibility::Public(_))
            && struct_.ident.to_string().ends_with("Options")
        {
            structs.push(parse_struct(&struct_)?);
        }
    }
    Ok(structs)
}

fn parse_struct(struct_: &ItemStruct) -> Result<OptionsStruct> {
    let name = struct_.ident.to_string();
    let syn::Fields::Named(named) = &struct_.fields else {
        bail!("options struct {name} must have named fields");
    };

    let mut fields = Vec::with_capacity(named.named.len());
    for field in &named.named {
        let field_name = field
            .ident
            .as_ref()
            .expect("named field always has an ident")
            .to_string();
        let Some(kind) = classify(&field.ty) else {
            bail!(
                "option field `{name}::{field_name}` has an unsupported type; \
                 teach dev/src/generate/core_options.rs about it before regenerating"
            );
        };
        fields.push(OptionField {
            name: field_name,
            kind,
            doc: doc_lines(&field.attrs),
        });
    }

    Ok(OptionsStruct {
        doc: doc_lines(&struct_.attrs),
        name,
        fields,
    })
}

fn classify(ty: &Type) -> Option<OptionKind> {
    let segment = last_segment(ty)?;
    match segment.ident.to_string().as_str() {
        "bool" => Some(OptionKind::Bool),
        "usize" => Some(OptionKind::Usize),
        "BytesRange" => Some(OptionKind::BytesRange),
        "Option" => {
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            let Some(GenericArgument::Type(inner)) = args.args.first() else {
                return None;
            };
            match last_segment(inner)?.ident.to_string().as_str() {
                "usize" => Some(OptionKind::OptionUsize),
                "u64" => Some(OptionKind::OptionU64),
                "String" => Some(OptionKind::OptionString),
                "Timestamp" => Some(OptionKind::OptionTimestamp),
                "HashMap" => Some(OptionKind::OptionStringMap),
                "Metadata" => Some(OptionKind::OptionMetadata),
                _ => None,
            }
        }
        _ => None,
    }
}

fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    path.segments.last()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_core_options() {
        let path = crate::workspace_dir().join("core/core/src/types/options.rs");
        let structs = parse(&path).expect("core options must parse");

        let names: Vec<&str> = structs.iter().map(|s| s.name.as_str()).collect();
        for expected in [
            "ReadOptions",
            "WriteOptions",
            "StatOptions",
            "ListOptions",
            "DeleteOptions",
            "CopyOptions",
            "RenameOptions",
        ] {
            assert!(names.contains(&expected), "missing {expected}");
        }

        let read = structs.iter().find(|s| s.name == "ReadOptions").unwrap();
        let kind = |name: &str| read.fields.iter().find(|f| f.name == name).unwrap().kind;
        assert_eq!(kind("range"), OptionKind::BytesRange);
        assert_eq!(kind("version"), OptionKind::OptionString);
        assert_eq!(kind("if_modified_since"), OptionKind::OptionTimestamp);
        assert_eq!(kind("content_length_hint"), OptionKind::OptionU64);
        assert_eq!(kind("concurrent"), OptionKind::Usize);
        assert_eq!(kind("chunk"), OptionKind::OptionUsize);

        let write = structs.iter().find(|s| s.name == "WriteOptions").unwrap();
        let kind = |name: &str| write.fields.iter().find(|f| f.name == name).unwrap().kind;
        assert_eq!(kind("append"), OptionKind::Bool);
        assert_eq!(kind("user_metadata"), OptionKind::OptionStringMap);
        assert_eq!(kind("if_not_changed"), OptionKind::OptionMetadata);
        assert!(!write.fields[0].doc.is_empty());
    }
}
