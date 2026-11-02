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

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::Path;
use syn::{
    Expr, ExprLit, Field, GenericArgument, Item, ItemStruct, Lit, Meta, PathArguments, Type,
    TypePath,
};

/// The Python type an operator option field maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    /// `Option<String>`
    Str,
    /// `Option<usize>`
    Int,
    /// `Option<bool>`
    Bool,
    /// `Option<jiff::Timestamp>`
    DateTime,
    /// `Option<HashMap<String, String>>`
    Dict,
}

/// One keyword option of an operator method.
#[derive(Debug, Clone)]
pub struct OptionField {
    /// The option name, e.g. `if_match`.
    pub name: String,
    /// The value type of this option.
    pub ty: OptionType,
    /// The doc comment of the field, trimmed.
    pub doc: String,
}

/// One option type, parsed from a `#[pyclass]` struct in
/// `bindings/python/src/options.rs`, e.g. `ReadOptions`.
#[derive(Debug, Clone)]
pub struct Options {
    /// The struct name, e.g. `ReadOptions`.
    pub name: String,
    /// The doc comment of the struct, trimmed.
    pub doc: String,
    /// The fields of the struct.
    pub fields: Vec<OptionField>,
}

/// Parse all `#[pyclass]` option structs from `bindings/python/src/options.rs`.
///
/// A struct is treated as an option type when it is annotated with
/// `#[pyclass]` and its name ends with `Options`.
pub fn parse(path: &Path) -> Result<Vec<Options>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let ast = syn::parse_file(&content)?;

    let mut options = Vec::new();
    for item in ast.items {
        if let Item::Struct(struct_) = item
            && is_option_struct(&struct_)
        {
            options.push(parse_struct(&struct_)?);
        }
    }
    Ok(options)
}

/// Whether a struct maps to a generated `*OptionsKwargs` TypedDict.
fn is_option_struct(struct_: &ItemStruct) -> bool {
    struct_.ident.to_string().ends_with("Options")
        && struct_
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("pyclass"))
}

fn parse_struct(struct_: &ItemStruct) -> Result<Options> {
    let name = struct_.ident.to_string();
    let doc = doc_lines(&struct_.attrs).join("\n").trim().to_string();

    let fields = match &struct_.fields {
        syn::Fields::Named(named) => &named.named,
        _ => bail!("option struct {name} must have named fields"),
    };

    let mut parsed = Vec::with_capacity(fields.len());
    for field in fields {
        parsed.push(parse_field(field)?);
    }

    Ok(Options {
        name,
        doc,
        fields: parsed,
    })
}

fn parse_field(field: &Field) -> Result<OptionField> {
    let name = field
        .ident
        .as_ref()
        .ok_or_else(|| anyhow!("option field is unnamed: {field:?}"))?
        .to_string();
    let ty = parse_type(&field.ty)?;
    let doc = doc_lines(&field.attrs).join("\n").trim().to_string();

    Ok(OptionField { name, ty, doc })
}

/// Parse the type of an option field, which must be `Option<T>`.
fn parse_type(ty: &Type) -> Result<OptionType> {
    let Type::Path(TypePath { path, .. }) = ty else {
        bail!("unsupported option field type: {ty:?}");
    };
    let segment = path
        .segments
        .last()
        .ok_or_else(|| anyhow!("option type path is empty: {ty:?}"))?;
    if segment.ident != "Option" {
        bail!("option field must be Option<T>, found {segment:?}");
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        bail!("Option must have angle bracketed arguments");
    };
    let Some(GenericArgument::Type(Type::Path(inner))) = args.args.first() else {
        bail!("Option must contain a type argument");
    };
    let inner_segment = inner
        .path
        .segments
        .last()
        .ok_or_else(|| anyhow!("option inner type path is empty: {inner:?}"))?;

    Ok(match inner_segment.ident.to_string().as_str() {
        "String" => OptionType::Str,
        "usize" => OptionType::Int,
        "bool" => OptionType::Bool,
        // jiff::Timestamp
        "Timestamp" => OptionType::DateTime,
        // HashMap<String, String>
        "HashMap" => OptionType::Dict,
        other => bail!("unsupported option type: {other}"),
    })
}

/// Collect the doc comment of a struct or field as trimmed lines.
pub(super) fn doc_lines(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            if let Meta::NameValue(meta) = &attr.meta
                && let Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &meta.value
            {
                return Some(s.value().trim().to_string());
            }

            None
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_options() {
        let path = crate::workspace_dir().join("bindings/python/src/options.rs");
        let options = parse(&path).unwrap();

        let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "ReadOptions",
                "WriteOptions",
                "ListOptions",
                "StatOptions",
                "DeleteOptions"
            ]
        );

        let read = &options[0];
        assert_eq!(read.doc, "Options for `read` operations.");
        assert_eq!(read.fields.len(), 14);
        assert_eq!(read.fields[0].name, "version");
        assert_eq!(read.fields[0].ty, OptionType::Str);
        assert_eq!(read.fields[4].name, "offset");
        assert_eq!(read.fields[4].ty, OptionType::Int);
        assert_eq!(read.fields[9].name, "if_modified_since");
        assert_eq!(read.fields[9].ty, OptionType::DateTime);
        assert_eq!(read.fields[0].doc, "The version of the file.");

        let write = &options[1];
        assert_eq!(write.fields.len(), 11);
        assert_eq!(write.fields[10].name, "user_metadata");
        assert_eq!(write.fields[10].ty, OptionType::Dict);
        assert_eq!(
            write.fields[10].doc,
            "The user metadata to set on the file."
        );

        let delete = &options[4];
        assert_eq!(delete.fields.len(), 3);
        assert_eq!(
            delete.fields[1].doc,
            "If True, delete the path recursively.\n\nOnly supported on backends that support recursive delete."
        );
        assert_eq!(delete.fields[2].name, "if_match");
        assert_eq!(delete.fields[2].ty, OptionType::Str);
        assert_eq!(
            delete.fields[2].doc,
            "The ETag that the object must match before deletion."
        );
    }
}
