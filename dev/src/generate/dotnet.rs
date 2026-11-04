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
//!
//! The operation options cross the FFI boundary as one `#[repr(C)]` struct
//! per call, which native code only borrows and converts into the core
//! options struct before the call returns. The public C# classes, their
//! interop structs, and the Rust mirrors are all rendered from
//! `core/core/src/types/options.rs`, so a new core option only needs a
//! regeneration.

use anyhow::{Result, bail};
use minijinja::{Environment, context};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::capability;
use super::core_options::{self, OptionField, OptionKind, OptionsStruct};
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
    /// Statement that applies the borrowed mirror fields to a
    /// `MetadataBuilder`, empty for the members the builder is constructed
    /// from.
    rs_rebuild: String,
    /// C# statement that fills the interop fields from the public value when
    /// metadata travels back to native code.
    cs_write: String,
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

/// Render model for one public `*Options` class and its native parser.
#[derive(Serialize)]
struct OptionsClass {
    /// The class name, identical to the core struct name, e.g. `ReadOptions`.
    name: String,
    /// snake_case operation used in native validation messages, e.g. `read`.
    operation: String,
    /// Class summary, already rendered as C# XML doc text.
    summary: String,
    /// The public properties in core field order.
    properties: Vec<OptionProperty>,
    /// C# statements that validate the properties before crossing the FFI.
    validations: Vec<String>,
    /// Fields of the `#[repr(C)]` mirror and its C# interop struct, in order.
    parts: Vec<OptionPart>,
    /// C# statements that fill the interop struct from the properties.
    cs_writes: Vec<String>,
    /// Rust statements that convert the mirror into core options.
    rs_convert: Vec<String>,
}

/// One field of an options mirror. An optional or composite value flattens
/// into several, following the metadata mirror's `*_has_value` convention.
#[derive(Serialize)]
struct OptionPart {
    /// snake_case field name in the Rust mirror.
    name: String,
    /// PascalCase field name in the C# interop struct.
    pascal: String,
    /// Field type in the Rust `#[repr(C)]` mirror.
    rs_type: String,
    /// Field type in the C# interop struct.
    cs_type: String,
    /// The field's value in the default mirror, matching the C# defaults.
    rs_default: String,
}

/// One public property of an `*Options` class. A core field usually maps to
/// one property; a `BytesRange` flattens into several.
#[derive(Serialize)]
struct OptionProperty {
    /// PascalCase property name.
    pascal: String,
    /// The C# property type.
    cs_type: String,
    /// Property initializer including the leading ` = `, empty when none.
    initializer: String,
    /// Property summary, already rendered as C# XML doc text.
    summary: String,
    /// `cref` targets of the capabilities the core doc refers to.
    see_also: Vec<String>,
}

/// The core option structs the binding exposes, with the snake_case
/// operation their native validation messages name.
const EXPOSED_OPTIONS: &[(&str, &str)] = &[
    ("ReadOptions", "read"),
    ("WriteOptions", "write"),
    ("StatOptions", "stat"),
    ("ListOptions", "list"),
    ("DeleteOptions", "delete"),
    ("CopyOptions", "copy"),
    ("RenameOptions", "rename"),
];

pub fn generate(workspace_dir: PathBuf, services: Services) -> Result<()> {
    generate_capability(&workspace_dir)?;
    generate_metadata(&workspace_dir)?;
    generate_options(&workspace_dir)?;
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
    validate_metadata_rebuild(&fields, &metadata::parse_builder_setters(&source)?)?;

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

/// Check that native code can rebuild every metadata accessor.
///
/// Each accessor needs a `MetadataBuilder` setter of the same name, except
/// `mode` and `content_length`, which the template passes to
/// `MetadataBuilder::file`. Anything else fails loudly, so a new accessor
/// upstream is never silently dropped on the way back to native code.
fn validate_metadata_rebuild(fields: &[MetadataField], setters: &[String]) -> Result<()> {
    for required in ["mode", "content_length"] {
        if !fields.iter().any(|f| f.name == required) {
            bail!(
                "metadata accessor `{required}` is gone; \
                 update the builder construction in dotnet/metadata_rs.j2"
            );
        }
    }

    for f in fields {
        if f.rs_rebuild.is_empty() {
            if !matches!(f.name.as_str(), "mode" | "content_length") {
                bail!(
                    "metadata accessor `{}` is not rebuilt; \
                     teach dev/src/generate/dotnet.rs how to apply it",
                    f.name
                );
            }
        } else if !setters.contains(&f.name) {
            bail!(
                "metadata accessor `{}` has no `MetadataBuilder` setter of the same name; \
                 teach dev/src/generate/dotnet.rs how to rebuild it",
                f.name
            );
        }
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

    // Metadata travels back to native code in the same struct, for options
    // such as `if_not_changed`, with every pointer borrowed from the caller.
    // `mode` and `content_length` have no setter of their own: the template
    // rebuilds them through `MetadataBuilder::file`.
    let (rs_rebuild, cs_write) = match f.kind {
        MetadataKind::Mode => (
            String::new(),
            format!("native.{pascal} = (int)metadata.{pascal};"),
        ),
        MetadataKind::U64 => (
            String::new(),
            format!("native.{pascal} = metadata.{pascal};"),
        ),
        MetadataKind::Bool => (
            format!("builder.{name}(self.{name} != 0);"),
            format!("native.{pascal} = metadata.{pascal} ? (byte)1 : (byte)0;"),
        ),
        MetadataKind::OptionBool => (
            format!("builder.{name}((self.{name}_has_value != 0).then_some(self.{name} != 0));"),
            format!(
                "if (metadata.{pascal} is {{ }} {camel})\n        {{\n            native.{pascal}HasValue = 1;\n            native.{pascal} = {camel} ? (byte)1 : (byte)0;\n        }}"
            ),
        ),
        MetadataKind::OptionStr => (
            format!(
                "if let Some(value) = crate::utils::borrowed_string(self.{name}, \"{name}\")? {{\n            builder.{name}(value);\n        }}"
            ),
            format!("native.{pascal} = writer.Utf8(metadata.{pascal});"),
        ),
        MetadataKind::OptionTimestamp => (
            format!(
                "if let Some(value) = crate::utils::borrowed_timestamp(\n            self.{name}_has_value,\n            self.{name}_second,\n            self.{name}_nanosecond,\n        )? {{\n            builder.{name}(value);\n        }}"
            ),
            format!(
                "if (metadata.{pascal} is {{ }} {camel})\n        {{\n            native.{pascal}HasValue = 1;\n            (native.{pascal}Second, native.{pascal}Nanosecond) = Utilities.ToUnixSecondAndNanosecond({camel});\n        }}"
            ),
        ),
        MetadataKind::OptionUserMetadata => (
            format!(
                "if let Some(values) = crate::utils::borrowed_string_pairs(\n            self.{name}_has_value,\n            self.{name}_keys as *const *const c_char,\n            self.{name}_values as *const *const c_char,\n            self.{name}_len,\n            \"{name}\",\n        )? {{\n            builder.{name}(values);\n        }}"
            ),
            format!(
                "(native.{pascal}HasValue, native.{pascal}Keys, native.{pascal}Values, native.{pascal}Len) = writer.Pairs(metadata.{pascal});"
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
        rs_rebuild,
        cs_write,
        pascal,
        camel,
        doc: f.doc,
        name: f.name,
    }
}

fn generate_options(workspace_dir: &Path) -> Result<()> {
    let source = workspace_dir.join("core/core/src/types/options.rs");
    let structs = core_options::parse(&source)?;

    let mut classes = Vec::with_capacity(EXPOSED_OPTIONS.len());
    for (name, operation) in EXPOSED_OPTIONS {
        let Some(options) = structs.iter().find(|s| s.name == *name) else {
            bail!("options struct {name} not found in {}", source.display());
        };
        classes.push(options_class(options, operation));
    }

    let mut env = Environment::new();
    env.add_template("options_rs", include_str!("dotnet/options_rs.j2"))?;
    env.add_template("options_cs", include_str!("dotnet/options_cs.j2"))?;

    let rendered = env
        .get_template("options_rs")?
        .render(context! { classes => classes })?;
    write_rendered(
        &workspace_dir.join("bindings/dotnet/src/options.rs"),
        rendered,
    )?;

    let tmpl = env.get_template("options_cs")?;
    for class in &classes {
        let rendered = tmpl.render(context! { class => class })?;
        let output =
            workspace_dir.join(format!("bindings/dotnet/OpenDAL/Options/{}.cs", class.name));
        write_rendered(&output, rendered)?;
    }

    Ok(())
}

/// Precompute every per-kind decision for one options struct.
///
/// This table is the only place that knows how an option shape crosses the
/// FFI boundary: the C# properties it surfaces as, how C# validates them and
/// fills the interop struct, and how the Rust mirror declares and converts
/// them. A new shape needs one arm here; fields that reuse a known shape need
/// nothing.
///
/// Counts (`usize` and `Option<usize>`) must be positive, matching every
/// count core exposes today.
fn options_class(options: &OptionsStruct, operation: &str) -> OptionsClass {
    let ctx = DocContext {
        fields: &options.fields,
    };

    let mut properties = Vec::new();
    let mut validations = Vec::new();
    let mut parts = Vec::new();
    let mut cs_writes = Vec::new();
    let mut rs_convert = Vec::new();

    for f in &options.fields {
        let key = f.name.as_str();
        let pascal = heck::AsUpperCamelCase(key).to_string();
        let camel = heck::AsLowerCamelCase(key).to_string();
        let property = |cs_type: &str, initializer: &str| OptionProperty {
            pascal: pascal.clone(),
            cs_type: cs_type.to_string(),
            initializer: initializer.to_string(),
            summary: ctx.summary(&f.doc),
            see_also: capability_refs(&f.doc),
        };
        let part = |name: &str, rs_type: &str, cs_type: &str| OptionPart {
            name: name.to_string(),
            pascal: heck::AsUpperCamelCase(name).to_string(),
            rs_type: rs_type.to_string(),
            cs_type: cs_type.to_string(),
            rs_default: if rs_type.starts_with('*') {
                "std::ptr::null()".to_string()
            } else {
                "0".to_string()
            },
        };
        let has_value = format!("{key}_has_value");

        match f.kind {
            OptionKind::Bool => {
                properties.push(property("bool", ""));
                parts.push(part(key, "u8", "byte"));
                cs_writes.push(format!("native.{pascal} = {pascal} ? (byte)1 : (byte)0;"));
                rs_convert.push(format!("options.{key} = self.{key} != 0;"));
            }
            OptionKind::Usize => {
                properties.push(property("int", " = 1;"));
                validations.push(format!(
                    "OptionValidators.RequireGreaterThanZero({pascal}, nameof({pascal}));"
                ));
                parts.push(OptionPart {
                    rs_default: "1".to_string(),
                    ..part(key, "usize", "nuint")
                });
                cs_writes.push(format!("native.{pascal} = (nuint){pascal};"));
                rs_convert.push(format!(
                    "validate_positive_integer(self.{key}, \"{operation} {key}\")?;\n        options.{key} = self.{key};"
                ));
            }
            OptionKind::OptionUsize => {
                properties.push(property("long?", ""));
                validations.push(format!(
                    "OptionValidators.RequireNullableGreaterThanZero({pascal}, nameof({pascal}));"
                ));
                parts.push(part(&has_value, "u8", "byte"));
                parts.push(part(key, "usize", "nuint"));
                cs_writes.push(format!(
                    "if ({pascal} is {{ }} {camel})\n        {{\n            native.{pascal}HasValue = 1;\n            native.{pascal} = (nuint){camel};\n        }}"
                ));
                rs_convert.push(format!(
                    "if self.{has_value} != 0 {{\n            validate_positive_integer(self.{key}, \"{operation} {key}\")?;\n            options.{key} = Some(self.{key});\n        }}"
                ));
            }
            OptionKind::OptionU64 => {
                properties.push(property("long?", ""));
                validations.push(format!(
                    "OptionValidators.RequireNullableGreaterThanOrEqualZero({pascal}, nameof({pascal}));"
                ));
                parts.push(part(&has_value, "u8", "byte"));
                parts.push(part(key, "u64", "ulong"));
                cs_writes.push(format!(
                    "if ({pascal} is {{ }} {camel})\n        {{\n            native.{pascal}HasValue = 1;\n            native.{pascal} = (ulong){camel};\n        }}"
                ));
                rs_convert.push(format!(
                    "options.{key} = (self.{has_value} != 0).then_some(self.{key});"
                ));
            }
            OptionKind::OptionString => {
                properties.push(property("string?", ""));
                parts.push(part(key, "*const c_char", "IntPtr"));
                cs_writes.push(format!("native.{pascal} = writer.String({pascal});"));
                rs_convert.push(format!(
                    "options.{key} = borrowed_string(self.{key}, \"{key}\")?;"
                ));
            }
            OptionKind::OptionTimestamp => {
                properties.push(property("DateTimeOffset?", ""));
                parts.push(part(&has_value, "u8", "byte"));
                parts.push(part(&format!("{key}_second"), "i64", "long"));
                parts.push(part(&format!("{key}_nanosecond"), "i32", "int"));
                cs_writes.push(format!(
                    "if ({pascal} is {{ }} {camel})\n        {{\n            native.{pascal}HasValue = 1;\n            (native.{pascal}Second, native.{pascal}Nanosecond) = Utilities.ToUnixSecondAndNanosecond({camel});\n        }}"
                ));
                rs_convert.push(format!(
                    "options.{key} =\n            borrowed_timestamp(self.{has_value}, self.{key}_second, self.{key}_nanosecond)?;"
                ));
            }
            OptionKind::OptionStringMap => {
                properties.push(property("IReadOnlyDictionary<string, string>?", ""));
                parts.push(part(&has_value, "u8", "byte"));
                parts.push(part(&format!("{key}_keys"), "*const *const c_char", "IntPtr"));
                parts.push(part(&format!("{key}_values"), "*const *const c_char", "IntPtr"));
                parts.push(part(&format!("{key}_len"), "usize", "nuint"));
                cs_writes.push(format!(
                    "(native.{pascal}HasValue, native.{pascal}Keys, native.{pascal}Values, native.{pascal}Len) = writer.Pairs({pascal});"
                ));
                rs_convert.push(format!(
                    "options.{key} = borrowed_string_pairs(\n            self.{has_value},\n            self.{key}_keys,\n            self.{key}_values,\n            self.{key}_len,\n            \"{key}\",\n        )?;"
                ));
            }
            OptionKind::OptionMetadata => {
                properties.push(property("Metadata?", ""));
                parts.push(part(key, "*const OpendalMetadata", "IntPtr"));
                cs_writes.push(format!("native.{pascal} = writer.MetadataPointer({pascal});"));
                rs_convert.push(format!(
                    "options.{key} = borrowed_metadata(self.{key})?;"
                ));
            }
            OptionKind::BytesRange => {
                // The established surface spells a range as an offset and an
                // optional length; a suffix range is a third, exclusive form.
                let range = |pascal: &str, cs_type: &str, summary: &str, see_also: &[&str]| {
                    OptionProperty {
                        pascal: pascal.to_string(),
                        cs_type: cs_type.to_string(),
                        initializer: String::new(),
                        summary: summary.to_string(),
                        see_also: see_also.iter().map(|s| s.to_string()).collect(),
                    }
                };
                properties.push(range(
                    "Offset",
                    "long",
                    "The byte offset to start reading from.",
                    &[],
                ));
                properties.push(range(
                    "Length",
                    "long?",
                    "The number of bytes to read from <see cref=\"Offset\"/>, or <see langword=\"null\"/> to read to the end.",
                    &[],
                ));
                properties.push(range(
                    "SuffixLength",
                    "long?",
                    "Read the last <see cref=\"SuffixLength\"/> bytes instead of a range from <see cref=\"Offset\"/>. It cannot be combined with <see cref=\"Offset\"/> or <see cref=\"Length\"/>.",
                    &["Capability.ReadWithSuffix"],
                ));
                validations.extend([
                    "OptionValidators.RequireGreaterThanOrEqualZero(Offset, nameof(Offset));"
                        .to_string(),
                    "OptionValidators.RequireNullableGreaterThanOrEqualZero(Length, nameof(Length));"
                        .to_string(),
                    "OptionValidators.RequireNullableGreaterThanOrEqualZero(SuffixLength, nameof(SuffixLength));"
                        .to_string(),
                    "OptionValidators.RequireSuffixExclusive(Offset, Length, SuffixLength);"
                        .to_string(),
                ]);
                parts.extend([
                    part("offset", "u64", "ulong"),
                    part("length_has_value", "u8", "byte"),
                    part("length", "u64", "ulong"),
                    part("suffix_length_has_value", "u8", "byte"),
                    part("suffix_length", "u64", "ulong"),
                ]);
                cs_writes.extend([
                    "native.Offset = (ulong)Offset;".to_string(),
                    "if (Length is { } length)\n        {\n            native.LengthHasValue = 1;\n            native.Length = (ulong)length;\n        }"
                        .to_string(),
                    "if (SuffixLength is { } suffixLength)\n        {\n            native.SuffixLengthHasValue = 1;\n            native.SuffixLength = (ulong)suffixLength;\n        }"
                        .to_string(),
                ]);
                rs_convert.push(format!(
                    "options.{key} = range(\n            self.offset,\n            (self.length_has_value != 0).then_some(self.length),\n            (self.suffix_length_has_value != 0).then_some(self.suffix_length),\n        )?;"
                ));
            }
        }
    }

    let summary = match ctx.summary(&options.doc) {
        s if s.is_empty() => format!("Options for {operation} operations."),
        s => s,
    };

    OptionsClass {
        name: options.name.clone(),
        operation: operation.to_string(),
        summary,
        properties,
        validations,
        parts,
        cs_writes,
        rs_convert,
    }
}

/// Renders rustdoc of one options struct as C# XML doc text.
struct DocContext<'a> {
    /// Fields of the struct, so code spans naming them become `cref`s.
    fields: &'a [OptionField],
}

impl DocContext<'_> {
    /// The first paragraph of a rustdoc comment as a single line of C# XML
    /// doc text.
    fn summary(&self, lines: &[String]) -> String {
        let paragraph = lines
            .iter()
            .map(|line| line.trim())
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        self.inline(&paragraph)
    }

    /// Translate inline rustdoc markup: intra-doc links, Markdown links, and
    /// code spans. Everything else is XML-escaped text.
    fn inline(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(pos) = rest.find(['[', '`']) {
            out.push_str(&xml_escape(&rest[..pos]));
            rest = &rest[pos..];

            if let Some(code) = rest.strip_prefix('`') {
                match code.find('`') {
                    Some(end) => {
                        out.push_str(&self.code(&code[..end]));
                        rest = &code[end + 1..];
                    }
                    None => {
                        out.push('`');
                        rest = code;
                    }
                }
                continue;
            }

            let Some(close) = rest.find(']') else {
                out.push('[');
                rest = &rest[1..];
                continue;
            };
            let label = &rest[1..close];
            let after = &rest[close + 1..];
            if let Some(target) = after.strip_prefix('(')
                && let Some(end) = target.find(')')
            {
                out.push_str(&format!(
                    "<see href=\"{}\">{}</see>",
                    xml_escape(&target[..end]),
                    self.inline(label)
                ));
                rest = &target[end + 1..];
            } else if let Some(target) = after.strip_prefix('[')
                && let Some(end) = target.find(']')
            {
                // A reference link points into the Rust docs; keep its text.
                out.push_str(&self.inline(label));
                rest = &target[end + 1..];
            } else if let Some(path) = label.strip_prefix('`').and_then(|l| l.strip_suffix('`')) {
                out.push_str(&self.path(path));
                rest = after;
            } else {
                out.push_str(&self.inline(label));
                rest = after;
            }
        }
        out.push_str(&xml_escape(rest));
        out
    }

    /// An intra-doc link target, e.g. `crate::Capability::read_with_version`.
    fn path(&self, path: &str) -> String {
        let path = path.strip_prefix("crate::").unwrap_or(path);
        let segments: Vec<&str> = path.split("::").collect();
        match segments.as_slice() {
            ["Capability", field] => format!(
                "<see cref=\"Capability.{}\"/>",
                heck::AsUpperCamelCase(field)
            ),
            ["ErrorKind", variant] => format!("<see cref=\"ErrorCode.{variant}\"/>"),
            ["Self", field] => self.code(field),
            [ty] if EXPOSED_OPTIONS.iter().any(|(name, _)| name == ty) => {
                format!("<see cref=\"{ty}\"/>")
            }
            [ty, field] if EXPOSED_OPTIONS.iter().any(|(name, _)| name == ty) => {
                format!("<see cref=\"{ty}.{}\"/>", heck::AsUpperCamelCase(field))
            }
            _ => format!("<c>{}</c>", xml_escape(path)),
        }
    }

    /// A code span: a sibling field becomes a `cref` to its property.
    fn code(&self, code: &str) -> String {
        if self
            .fields
            .iter()
            .any(|f| f.name == code && f.kind != OptionKind::BytesRange)
        {
            format!("<see cref=\"{}\"/>", heck::AsUpperCamelCase(code))
        } else if matches!(code, "true" | "false") {
            format!("<see langword=\"{code}\"/>")
        } else {
            format!("<c>{}</c>", xml_escape(code))
        }
    }
}

/// The `cref` targets of every `Capability` field a rustdoc comment links to,
/// in order of first mention.
fn capability_refs(lines: &[String]) -> Vec<String> {
    const MARKER: &str = "crate::Capability::";

    let mut refs = Vec::new();
    for line in lines {
        let mut rest = line.as_str();
        while let Some(pos) = rest.find(MARKER) {
            rest = &rest[pos + MARKER.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let cref = format!("Capability.{}", heck::AsUpperCamelCase(&rest[..end]));
            if !refs.contains(&cref) {
                refs.push(cref);
            }
        }
    }
    refs
}

fn enabled_service(srv: &str) -> bool {
    match srv {
        // not enabled in bindings/dotnet/Cargo.toml
        "foundationdb" | "foyer" | "ftp" | "git" | "hdfs" | "rocksdb" | "smb" => false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_doc_inline() {
        let fields = [OptionField {
            name: "if_match".to_string(),
            kind: OptionKind::OptionString,
            doc: vec![],
        }];
        let ctx = DocContext { fields: &fields };

        assert_eq!(
            ctx.inline("Check [`crate::Capability::read_with_if_match`] first."),
            "Check <see cref=\"Capability.ReadWithIfMatch\"/> first."
        );
        assert_eq!(
            ctx.inline("Fails with [`crate::ErrorKind::ConditionNotMatch`]."),
            "Fails with <see cref=\"ErrorCode.ConditionNotMatch\"/>."
        );
        assert_eq!(
            ctx.inline("Conflicts with `if_match`, default `false`, see `a<b`."),
            "Conflicts with <see cref=\"IfMatch\"/>, default <see langword=\"false\"/>, see <c>a&lt;b</c>."
        );
        assert_eq!(
            ctx.inline("See [the spec][crate::docs::specs::x] and [`crate::Reader::fetch`]."),
            "See the spec and <c>Reader::fetch</c>."
        );
        assert_eq!(
            ctx.inline("Read [MDN](https://example.com/a?b&c)."),
            "Read <see href=\"https://example.com/a?b&amp;c\">MDN</see>."
        );
    }

    #[test]
    fn test_metadata_rebuild_requires_a_setter() {
        let field = |name: &str, kind| {
            metadata_field(metadata::MetadataField {
                name: name.to_string(),
                kind,
                doc: String::new(),
            })
        };
        let fields = vec![
            field("mode", MetadataKind::Mode),
            field("content_length", MetadataKind::U64),
            field("etag", MetadataKind::OptionStr),
        ];
        let setters = vec!["etag".to_string()];
        assert!(validate_metadata_rebuild(&fields, &setters).is_ok());

        let mut with_new_accessor = fields;
        with_new_accessor.push(field("storage_class", MetadataKind::OptionStr));
        let err = validate_metadata_rebuild(&with_new_accessor, &setters)
            .expect_err("an accessor without a setter must fail");
        assert!(err.to_string().contains("storage_class"));

        let without_mode = vec![field("etag", MetadataKind::OptionStr)];
        assert!(validate_metadata_rebuild(&without_mode, &setters).is_err());
    }

    #[test]
    fn test_capability_refs() {
        let lines = vec![
            "Check [`crate::Capability::copy_can_multi`] and".to_string(),
            "[`crate::Capability::copy_multi_min_size`]; see `copy_can_multi`".to_string(),
            "again: [`crate::Capability::copy_can_multi`].".to_string(),
        ];
        assert_eq!(
            capability_refs(&lines),
            ["Capability.CopyCanMulti", "Capability.CopyMultiMinSize"]
        );
    }
}
