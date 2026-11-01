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

//! Generator for the .NET binding capability and metadata mirrors and the
//! service configs.
//!
//! The capability payload crosses the FFI boundary as a `#[repr(C)]` struct,
//! so the Rust mirror, the C# interop struct, and the public C# surface must
//! agree on field order and on the sentinel that encodes an absent limit.
//! All three files are rendered from `core/core/src/types/capability.rs` so
//! they cannot drift from core or from each other.
//!
//! The metadata payload follows the same pattern, with the interop struct
//! also carrying the conversion into the public class. All three files are
//! rendered from the accessors of `core/core/src/types/metadata.rs`.
//!
//! The typed `*ServiceConfig` classes mirror each service's config struct and
//! are rendered from the same parsed service definitions the Java and Python
//! generators consume.

use anyhow::{Result, bail};
use minijinja::{Environment, context};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::capability;
use super::metadata::{self, MetadataKind};
use super::parser::{Config, ConfigType, Services, sorted_services};

/// Render model handed to the capability templates, with every casing
/// precomputed so the templates stay purely structural.
#[derive(Serialize)]
struct CapabilityField {
    /// snake_case, used by the Rust mirror.
    name: String,
    /// PascalCase, used by the public C# properties.
    pascal: String,
    /// camelCase, used by the C# interop struct.
    camel: String,
    /// Plain `bool` flag when true, `Option<usize>` limit when false.
    is_bool: bool,
    /// Field doc from core, XML-escaped for use in C# doc comments.
    doc: String,
}

/// Render model handed to the metadata templates. Every per-kind decision is
/// precomputed in `metadata_field` so the templates stay purely structural.
#[derive(Serialize)]
struct MetadataField {
    /// snake_case accessor name, used by the Rust mirror.
    name: String,
    /// PascalCase, used by the C# interop struct and the public properties.
    pascal: String,
    /// camelCase, used for constructor parameters.
    camel: String,
    /// The FFI shape; the public template derives `IsFile`/`IsDir` from `mode`.
    kind: MetadataKind,
    /// Accessor doc from core, used verbatim in Rust doc comments.
    doc: String,
    /// The same doc, XML-escaped for C# doc comments.
    doc_xml: String,
    /// The mirror fields this accessor flattens into, in declaration order.
    parts: Vec<MetadataPart>,
    /// Pattern that binds every part from `rs_from`: the bare name for a
    /// single part, a tuple otherwise.
    rs_bind: String,
    /// Expression that produces the part values from `metadata`.
    rs_from: String,
    /// Statement that releases the parts' heap memory, empty when none.
    rs_release: String,
    /// The public C# type of the property.
    cs_type: String,
    /// Expression that reads the public value from the interop struct's fields.
    cs_read: String,
}

/// One field of the mirrors. An accessor with an optional or composite
/// return type flattens into several, following the binding's convention of
/// a `*_has_value` byte followed by the parts.
#[derive(Serialize)]
struct MetadataPart {
    /// snake_case field name in the Rust mirror.
    name: String,
    /// PascalCase field name in the C# interop struct.
    pascal: String,
    /// Field type in the Rust `#[repr(C)]` mirror.
    rs_type: String,
    /// Field type in the C# interop struct.
    interop_type: String,
}

/// Render model for one field of a `*ServiceConfig` class.
#[derive(Serialize)]
struct ConfigField {
    /// PascalCase property name.
    pascal: String,
    /// snake_case option key passed to native OpenDAL.
    key: String,
    /// The C# property type, e.g. `string?` or `long`.
    cs_type: String,
    /// Whether `ToOptions` guards the entry with a null check.
    check_null: bool,
    /// The expression producing the option string for this property.
    value_expr: String,
    /// Deprecation message for `[System.Obsolete]`, empty when not deprecated.
    deprecated: String,
    /// Field doc from core, single line, XML-escaped. Empty docs omit the block.
    doc: String,
}

pub fn generate(workspace_dir: PathBuf, services: Services) -> Result<()> {
    generate_capability(&workspace_dir)?;
    generate_metadata(&workspace_dir)?;
    generate_service_configs(&workspace_dir, services)
}

fn generate_capability(workspace_dir: &Path) -> Result<()> {
    let source = workspace_dir.join("core/core/src/types/capability.rs");
    let fields: Vec<CapabilityField> = capability::parse(&source)?
        .into_iter()
        .map(|f| CapabilityField {
            pascal: heck::AsUpperCamelCase(&f.name).to_string(),
            camel: heck::AsLowerCamelCase(&f.name).to_string(),
            name: f.name,
            is_bool: f.is_bool,
            doc: xml_escape(&f.doc),
        })
        .collect();

    let mut env = Environment::new();
    env.add_template("capability_rs", include_str!("dotnet/capability_rs.j2"))?;
    env.add_template(
        "interop_cs",
        include_str!("dotnet/capability_interop_cs.j2"),
    )?;
    env.add_template("public_cs", include_str!("dotnet/capability_public_cs.j2"))?;

    let outputs = [
        ("capability_rs", "bindings/dotnet/src/capability.rs"),
        (
            "interop_cs",
            "bindings/dotnet/OpenDAL/Interop/NativeObject/OpenDALCapability.cs",
        ),
        ("public_cs", "bindings/dotnet/OpenDAL/Capability.cs"),
    ];
    for (template, relative) in outputs {
        let tmpl = env.get_template(template)?;
        let rendered = tmpl.render(context! { fields => fields })?;
        write_rendered(&workspace_dir.join(relative), rendered)?;
    }

    Ok(())
}

fn generate_metadata(workspace_dir: &Path) -> Result<()> {
    let source = workspace_dir.join("core/core/src/types/metadata.rs");
    let fields: Vec<MetadataField> = metadata::parse(&source)?
        .into_iter()
        .map(metadata_field)
        .collect();

    let mut env = Environment::new();
    env.add_template("metadata_rs", include_str!("dotnet/metadata_rs.j2"))?;
    env.add_template(
        "metadata_interop_cs",
        include_str!("dotnet/metadata_interop_cs.j2"),
    )?;
    env.add_template(
        "metadata_public_cs",
        include_str!("dotnet/metadata_public_cs.j2"),
    )?;

    let outputs = [
        ("metadata_rs", "bindings/dotnet/src/metadata.rs"),
        (
            "metadata_interop_cs",
            "bindings/dotnet/OpenDAL/Interop/NativeObject/OpenDALMetadata.cs",
        ),
        ("metadata_public_cs", "bindings/dotnet/OpenDAL/Metadata.cs"),
    ];
    for (template, relative) in outputs {
        let tmpl = env.get_template(template)?;
        let rendered = tmpl.render(context! { fields => fields })?;
        write_rendered(&workspace_dir.join(relative), rendered)?;
    }

    Ok(())
}

/// The C# name for a metadata accessor, allowing the established casing of
/// names that predate codegen.
fn metadata_pascal(name: &str) -> String {
    match name {
        "etag" => "ETag".to_string(),
        _ => heck::AsUpperCamelCase(name).to_string(),
    }
}

/// Precompute every per-kind decision for one accessor.
///
/// This table is the only place that knows how a return type crosses the FFI
/// boundary: the mirror fields it flattens into, how the Rust side fills and
/// releases them, and how the C# side reads them back. A new return type
/// needs one row here and, when it flattens into several fields, a helper in
/// `bindings/dotnet/src/utils.rs`. Accessors that reuse a known return type
/// need nothing.
fn metadata_field(f: metadata::MetadataField) -> MetadataField {
    let name = &f.name;
    let pascal = metadata_pascal(name);
    let camel = heck::AsLowerCamelCase(name).to_string();
    let part = |suffix: &str, rs_type: &str, interop_type: &str| MetadataPart {
        name: format!("{name}{suffix}"),
        pascal: format!("{pascal}{}", heck::AsUpperCamelCase(suffix)),
        rs_type: rs_type.to_string(),
        interop_type: interop_type.to_string(),
    };

    let (parts, rs_from, rs_release, cs_type, cs_read) = match f.kind {
        MetadataKind::Mode => (
            vec![part("", "i32", "int")],
            format!("crate::utils::entry_mode_code(metadata.{name}())"),
            String::new(),
            "EntryMode",
            format!("Utilities.ToEntryMode({pascal})"),
        ),
        MetadataKind::Bool => (
            vec![part("", "u8", "byte")],
            format!("u8::from(metadata.{name}())"),
            String::new(),
            "bool",
            format!("{pascal} != 0"),
        ),
        MetadataKind::U64 => (
            vec![part("", "u64", "ulong")],
            format!("metadata.{name}()"),
            String::new(),
            "ulong",
            pascal.clone(),
        ),
        MetadataKind::OptionBool => (
            vec![part("_has_value", "u8", "byte"), part("", "u8", "byte")],
            format!("crate::utils::optional_bool(metadata.{name}())"),
            String::new(),
            "bool?",
            format!("{pascal}HasValue != 0 ? {pascal} != 0 : null"),
        ),
        MetadataKind::OptionStr => (
            vec![part("", "*mut c_char", "IntPtr")],
            format!("crate::utils::optional_c_string(metadata.{name}())"),
            format!("crate::utils::release_c_string(&mut metadata.{name});"),
            "string?",
            format!("Utilities.ReadNullableUtf8({pascal})"),
        ),
        MetadataKind::OptionTimestamp => (
            vec![
                part("_has_value", "u8", "byte"),
                part("_second", "i64", "long"),
                part("_nanosecond", "i32", "int"),
            ],
            format!("crate::utils::optional_timestamp(metadata.{name}())"),
            String::new(),
            "DateTimeOffset?",
            format!(
                "{pascal}HasValue != 0 ? Utilities.ToDateTimeOffset({pascal}Second, {pascal}Nanosecond) : null"
            ),
        ),
        MetadataKind::OptionUserMetadata => (
            vec![
                part("_has_value", "u8", "byte"),
                part("_keys", "*mut *mut c_char", "IntPtr"),
                part("_values", "*mut *mut c_char", "IntPtr"),
                part("_len", "usize", "nuint"),
            ],
            format!("crate::utils::string_pairs(metadata.{name}())"),
            format!(
                "crate::utils::release_string_pairs(&mut metadata.{name}_keys, &mut metadata.{name}_values, &mut metadata.{name}_len);"
            ),
            "IReadOnlyDictionary<string, string>?",
            format!(
                "{pascal}HasValue != 0 ? Utilities.ReadStringPairs({pascal}Keys, {pascal}Values, {pascal}Len, StringComparer.Ordinal) : null"
            ),
        ),
    };

    let rs_bind = match parts.as_slice() {
        [only] => only.name.clone(),
        many => {
            let names: Vec<&str> = many.iter().map(|p| p.name.as_str()).collect();
            format!("({})", names.join(", "))
        }
    };

    MetadataField {
        kind: f.kind,
        doc_xml: xml_escape(&f.doc),
        parts,
        rs_bind,
        rs_from,
        rs_release,
        cs_type: cs_type.to_string(),
        cs_read,
        pascal,
        camel,
        doc: f.doc,
        name: f.name,
    }
}

fn enabled_service(srv: &str) -> bool {
    match srv {
        // not enabled in bindings/dotnet/Cargo.toml
        "foundationdb" | "foyer" | "ftp" | "hdfs" | "rocksdb" => false,
        _ => true,
    }
}

/// The class name and scheme string for a service, allowing the few services
/// whose established .NET surface deviates from the service directory name.
fn service_identity(srv: &str) -> (String, String) {
    match srv {
        // The public class predates codegen and must keep its name.
        "hf" => ("Huggingface".to_string(), "huggingface".to_string()),
        _ => (heck::AsUpperCamelCase(srv).to_string(), srv.to_string()),
    }
}

fn generate_service_configs(workspace_dir: &Path, services: Services) -> Result<()> {
    let srvs = sorted_services(services, enabled_service);

    let mut env = Environment::new();
    env.add_template("service_config", include_str!("dotnet/service_config.j2"))?;
    let tmpl = env.get_template("service_config")?;

    let mut names: Vec<&String> = srvs.keys().collect();
    names.sort();
    for name in names {
        let (class_name, scheme) = service_identity(name);
        let fields = srvs[name]
            .config
            .iter()
            // A HashMap config has no string encoding in the options map, so
            // it cannot be expressed through IServiceConfig.ToOptions.
            .filter(|config| config.value != ConfigType::HashMap)
            .map(config_field)
            .collect::<Result<Vec<ConfigField>>>()?;

        let rendered = tmpl.render(context! {
            service => name,
            class_name => class_name,
            scheme => scheme,
            fields => fields,
        })?;
        let output = workspace_dir.join(format!(
            "bindings/dotnet/OpenDAL/ServiceConfig/{class_name}ServiceConfig.cs"
        ));
        write_rendered(&output, rendered)?;
    }

    Ok(())
}

fn config_field(config: &Config) -> Result<ConfigField> {
    let pascal = heck::AsUpperCamelCase(&config.name).to_string();

    // Strings and durations stay nullable even when core requires them, so a
    // partially built config still converts and native-side validation stays
    // the single source of required-ness. Numeric requiredness is kept because
    // a non-nullable numeric property always has a value to emit.
    let (cs_type, check_null, value_expr) = match config.value {
        ConfigType::Bool => ("bool?", true, to_option_string(&pascal)),
        ConfigType::String | ConfigType::Duration => ("string?", true, to_option_string(&pascal)),
        ConfigType::Usize | ConfigType::U64 | ConfigType::I64 => {
            if config.optional {
                ("long?", true, to_option_string(&pascal))
            } else {
                ("long", false, to_option_string(&pascal))
            }
        }
        ConfigType::U32 | ConfigType::U16 => {
            if config.optional {
                ("int?", true, to_option_string(&pascal))
            } else {
                ("int", false, to_option_string(&pascal))
            }
        }
        ConfigType::Vec => (
            "IReadOnlyList<string>?",
            true,
            format!("string.Join(\",\", {pascal})"),
        ),
        ConfigType::HashMap => bail!(
            "config `{}` is a HashMap and should have been filtered out",
            config.name
        ),
    };

    let deprecated = match &config.deprecated {
        Some(attr) => cs_escape(&pascalize_backticks(&attr.note)),
        None => String::new(),
    };

    Ok(ConfigField {
        pascal,
        key: config.name.clone(),
        cs_type: cs_type.to_string(),
        check_null,
        value_expr,
        deprecated,
        doc: xml_escape(&single_line(&config.comments)),
    })
}

fn to_option_string(pascal: &str) -> String {
    format!("Utilities.ToOptionString({pascal})")
}

/// Collapse a multi-line doc into a single line.
fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Rewrite backticked snake_case identifiers as the PascalCase property they
/// correspond to on this class, e.g. `` `skip_signature` `` -> `SkipSignature`.
fn pascalize_backticks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        result.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('`') {
            Some(end)
                if after[..end]
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') =>
            {
                result.push_str(&heck::AsUpperCamelCase(&after[..end]).to_string());
                rest = &after[end + 1..];
            }
            _ => {
                result.push('`');
                rest = after;
            }
        }
    }
    result.push_str(rest);
    result
}

/// Escape the characters that are special inside C# XML doc comments.
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape a string for embedding in a C# string literal.
fn cs_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_rendered(output: &std::path::Path, mut rendered: String) -> Result<()> {
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    fs::write(output, rendered)?;
    Ok(())
}
