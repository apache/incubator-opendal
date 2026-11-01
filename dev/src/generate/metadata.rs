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

//! Parser for the accessor surface of `core/core/src/types/metadata.rs`.
//!
//! `Metadata` is a compact encoding, so it has no field list to mirror; its
//! public shape is the set of `pub fn name(&self) -> T` accessors. This module
//! reads those accessors in source order and classifies each return type into
//! the handful of shapes the .NET FFI mirror knows how to carry, so the Rust
//! mirror, the C# interop struct, the marshaller, and the public class are all
//! rendered from the same list.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::Path;
use syn::{FnArg, GenericArgument, ImplItem, Item, PathArguments, ReturnType, Type};

use super::options::doc_lines;

/// The FFI shape of one accessor's return type.
///
/// Only the shapes that actually occur in core are supported. Anything else
/// fails parsing so that a new accessor shape upstream breaks the generator
/// loudly instead of producing a silently wrong mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataKind {
    /// `EntryMode`, carried as an `i32` discriminant.
    Mode,
    /// `bool`, carried as a `u8`.
    Bool,
    /// `u64`, carried as is.
    U64,
    /// `Option<bool>`, carried as a presence byte plus a value byte.
    OptionBool,
    /// `Option<&str>`, carried as an owned C string or null.
    OptionStr,
    /// `Option<Timestamp>`, carried as a presence byte plus seconds and nanoseconds.
    OptionTimestamp,
    /// `Option<UserMetadata>`, carried as a presence byte plus parallel key and
    /// value arrays with a shared length.
    OptionUserMetadata,
}

/// A single accessor of core `Metadata`.
#[derive(Debug, Serialize)]
pub struct MetadataField {
    /// Accessor name in snake_case, exactly as written in core.
    pub name: String,
    /// The FFI shape of the accessor's return type.
    pub kind: MetadataKind,
    /// First paragraph of the accessor doc, joined to a single line.
    pub doc: String,
}

/// Accessors derived from another field, which have no FFI representation.
const DERIVED: &[&str] = &["is_file", "is_dir"];

/// Parse the `impl Metadata` accessors out of the given core source file.
pub fn parse(path: &Path) -> Result<Vec<MetadataField>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let ast = syn::parse_file(&content)?;

    let mut fields = Vec::new();
    for item in ast.items {
        let Item::Impl(impl_) = item else {
            continue;
        };
        if impl_.trait_.is_some() || !is_metadata(&impl_.self_ty) {
            continue;
        }

        for item in impl_.items {
            let ImplItem::Fn(func) = item else {
                continue;
            };
            if !matches!(func.vis, syn::Visibility::Public(_)) || !takes_only_ref_self(&func.sig) {
                continue;
            }

            let name = func.sig.ident.to_string();
            if DERIVED.contains(&name.as_str()) {
                continue;
            }

            let Some(kind) = classify(&func.sig.output) else {
                bail!(
                    "metadata accessor `{name}` returns an unsupported type; \
                     teach dev/src/generate/metadata.rs about it before regenerating"
                );
            };
            let doc = first_paragraph(&doc_lines(&func.attrs));
            fields.push(MetadataField { name, kind, doc });
        }
    }

    if fields.is_empty() {
        bail!("impl Metadata not found in {}", path.display());
    }

    Ok(fields)
}

fn is_metadata(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.is_ident("Metadata"))
}

/// Whether the signature is exactly `(&self)`, which excludes consuming
/// conversions such as `into_builder(self)`.
fn takes_only_ref_self(sig: &syn::Signature) -> bool {
    let mut inputs = sig.inputs.iter();
    let receiver = match inputs.next() {
        Some(FnArg::Receiver(receiver)) => receiver,
        _ => return false,
    };
    receiver.reference.is_some() && receiver.mutability.is_none() && inputs.next().is_none()
}

fn classify(output: &ReturnType) -> Option<MetadataKind> {
    let ReturnType::Type(_, ty) = output else {
        return None;
    };
    let Type::Path(path) = ty.as_ref() else {
        return None;
    };
    let segment = path.path.segments.last()?;

    if segment.ident == "EntryMode" {
        return Some(MetadataKind::Mode);
    }
    if segment.ident == "bool" {
        return Some(MetadataKind::Bool);
    }
    if segment.ident == "u64" {
        return Some(MetadataKind::U64);
    }
    if segment.ident != "Option" {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return None;
    };

    match inner {
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Path(elem) if elem.path.is_ident("str") => Some(MetadataKind::OptionStr),
            _ => None,
        },
        Type::Path(inner) => {
            let ident = &inner.path.segments.last()?.ident;
            if ident == "bool" {
                Some(MetadataKind::OptionBool)
            } else if ident == "Timestamp" {
                Some(MetadataKind::OptionTimestamp)
            } else if ident == "UserMetadata" {
                Some(MetadataKind::OptionUserMetadata)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Join the doc lines up to the first blank line into a single line.
fn first_paragraph(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| line.trim())
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_core_metadata() {
        let path = crate::workspace_dir().join("core/core/src/types/metadata.rs");
        let fields = parse(&path).expect("core metadata must parse");

        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names.first(), Some(&"mode"));
        assert_eq!(names.last(), Some(&"user_metadata"));
        assert!(!names.contains(&"is_file"));
        assert!(!names.contains(&"into_builder"));

        let current = fields.iter().find(|f| f.name == "is_current").unwrap();
        assert_eq!(current.kind, MetadataKind::OptionBool);
        assert!(
            !current.doc.contains("None"),
            "doc must stop at the first paragraph"
        );

        let etag = fields.iter().find(|f| f.name == "etag").unwrap();
        assert_eq!(etag.kind, MetadataKind::OptionStr);
    }
}
