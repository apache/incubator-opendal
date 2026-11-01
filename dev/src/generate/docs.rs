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

//! Generate a single `services.json` that drives the website's per-service
//! configuration documentation.
//!
//! The config fields (keys, types, required, comments) are the universal
//! source of truth shared by every binding, so the data is emitted once per
//! service. On top of that we render ready-to-copy snippets for the bindings
//! whose config-passing API is a plain `String -> String` map, which is the
//! exact shape `via_iter` / `Operator.of(map)` / kwargs / `new Operator(obj)`
//! all expect. Adding a new binding is a matter of adding an entry to
//! [`BINDINGS`] plus a snippet renderer in [`render_examples`].

use crate::generate::parser::{AttrDeprecated, Config, ConfigType, Services};
use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A binding we can render copy-paste snippets for.
struct Binding {
    /// Stable id, also used in the page URL: `/services/<scheme>/<id>`.
    id: &'static str,
    /// Human readable label shown on the language tab.
    label: &'static str,
    /// Prism language id used for syntax highlighting in the code block.
    language: &'static str,
    /// Path (relative to the workspace root) of the binding crate's
    /// `Cargo.toml`, used to learn which services it enables. `None` means the
    /// binding exposes every service (this is the case for the Rust crate
    /// itself, which is the source of all services).
    cargo: Option<&'static str>,
}

const BINDINGS: &[Binding] = &[
    Binding {
        id: "rust",
        label: "Rust",
        language: "rust",
        cargo: None,
    },
    Binding {
        id: "java",
        label: "Java",
        language: "java",
        cargo: Some("bindings/java/Cargo.toml"),
    },
    Binding {
        id: "python",
        label: "Python",
        language: "python",
        cargo: Some("bindings/python/Cargo.toml"),
    },
    Binding {
        id: "nodejs",
        label: "Node.js",
        language: "javascript",
        cargo: Some("bindings/nodejs/Cargo.toml"),
    },
    Binding {
        id: "ruby",
        label: "Ruby",
        language: "ruby",
        cargo: Some("bindings/ruby/Cargo.toml"),
    },
    Binding {
        id: "go",
        label: "Go",
        language: "go",
        cargo: None,
    },
    Binding {
        id: "c",
        label: "C",
        language: "c",
        cargo: Some("bindings/c/Cargo.toml"),
    },
    Binding {
        id: "cpp",
        label: "C++",
        language: "cpp",
        cargo: Some("bindings/cpp/Cargo.toml"),
    },
];

#[derive(Serialize)]
struct DocsOutput {
    bindings: Vec<BindingMeta>,
    services: Vec<ServiceDoc>,
}

#[derive(Serialize)]
struct BindingMeta {
    id: String,
    label: String,
    language: String,
}

#[derive(Serialize)]
struct ServiceDoc {
    /// Directory name, e.g. `mini_moka`.
    name: String,
    /// The scheme users pass to a binding, e.g. `mini-moka`.
    scheme: String,
    /// Group names in display order (first appearance in the config struct).
    groups: Vec<String>,
    configs: Vec<ConfigDoc>,
    examples: Vec<ServiceExample>,
}

#[derive(Serialize)]
struct ConfigDoc {
    name: String,
    /// Friendly type name: `string`, `bool`, `integer`, `duration`, `list`, `map`.
    #[serde(rename = "type")]
    type_name: String,
    required: bool,
    /// Resolved group; defaults to [`DEFAULT_GROUP`] when the field is unmarked.
    group: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    example: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deprecated: Option<AttrDeprecated>,
    comments: String,
}

/// Group assigned to fields without an explicit `@group` marker.
const DEFAULT_GROUP: &str = "General";

/// Conventional group name for deprecated options; always sorted last.
const DEPRECATED_GROUP: &str = "Deprecated";

#[derive(Serialize)]
struct ServiceExample {
    binding: String,
    language: String,
    minimal: String,
    full: String,
}

pub fn generate(workspace_dir: PathBuf, services: Services) -> Result<()> {
    // Resolve, per binding, the set of schemes it enables.
    let support: Vec<Option<HashSet<String>>> = BINDINGS
        .iter()
        .map(|b| match b.cargo {
            None => Ok(None),
            Some(rel) => supported_schemes(&workspace_dir, rel).map(Some),
        })
        .collect::<Result<_>>()?;

    let mut service_docs = Vec::with_capacity(services.len());
    for (name, service) in services {
        let scheme = name.replace('_', "-");

        // Preserve source order: the order fields appear in the config struct is
        // the author's logical order (root, bucket, endpoint, credentials, ...),
        // which reads far better than an alphabetical dump.
        let configs = &service.config;

        // Group names in first-appearance order, across all fields (the table
        // shows deprecated fields too, so they participate in group ordering).
        let mut groups: Vec<String> = Vec::new();
        for c in configs {
            let g = c.group.clone().unwrap_or_else(|| DEFAULT_GROUP.to_string());
            if !groups.contains(&g) {
                groups.push(g);
            }
        }
        // Deprecated options are noise for newcomers; always show them last.
        if let Some(pos) = groups.iter().position(|g| g == DEPRECATED_GROUP) {
            let g = groups.remove(pos);
            groups.push(g);
        }

        // Fields usable in a snippet: not deprecated.
        let fields: Vec<Field> = configs
            .iter()
            .filter(|c| c.deprecated.is_none())
            .map(Field::from)
            .collect();
        let grouped = group_fields(&fields, &groups);
        let required: Vec<&Field> = fields.iter().filter(|f| f.required).collect();

        let mut examples = Vec::new();
        for (binding, support) in BINDINGS.iter().zip(support.iter()) {
            let enabled = match support {
                None => true,
                Some(set) => set.contains(&scheme),
            };
            if !enabled {
                continue;
            }
            let (minimal, full) = render_examples(binding.id, &scheme, &required, &grouped);
            examples.push(ServiceExample {
                binding: binding.id.to_string(),
                language: binding.language.to_string(),
                minimal,
                full,
            });
        }

        service_docs.push(ServiceDoc {
            name,
            scheme,
            groups,
            configs: configs.iter().map(ConfigDoc::from).collect(),
            examples,
        });
    }

    service_docs.sort_by(|a, b| a.scheme.cmp(&b.scheme));

    let output = DocsOutput {
        bindings: BINDINGS
            .iter()
            .map(|b| BindingMeta {
                id: b.id.to_string(),
                label: b.label.to_string(),
                language: b.language.to_string(),
            })
            .collect(),
        services: service_docs,
    };

    let path = workspace_dir.join("website/data/services.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&output)?;
    fs::write(&path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    log::info!(
        "generated {} with {} services",
        path.display(),
        output.services.len()
    );
    Ok(())
}

/// Collect the schemes a binding crate enables by scanning every `services-*`
/// token in its `Cargo.toml`. Bindings declare services in two places: their
/// own `[features]` table (Python, Node.js) or the `opendal` dependency's
/// `features` list (Java), so a textual scan over non-comment text covers both
/// layouts and naturally skips `# "services-foo"` lines that are turned off.
/// Feature names are kebab-case and map directly to a scheme once the
/// `services-` prefix is stripped.
fn supported_schemes(workspace: &Path, cargo_rel: &str) -> Result<HashSet<String>> {
    let path = workspace.join(cargo_rel);
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;

    let re = Regex::new(r"services-([a-z0-9-]+)").expect("valid regex");
    let mut set = HashSet::new();
    for line in content.lines() {
        // Cargo.toml comments run from `#` to end of line; service names never
        // contain `#`, so cutting there is enough to ignore disabled services.
        let code = line.split('#').next().unwrap_or(line);
        for cap in re.captures_iter(code) {
            set.insert(cap[1].to_string());
        }
    }
    Ok(set)
}

/// A single config field reduced to what the renderers need.
struct Field<'a> {
    name: &'a str,
    value: ConfigType,
    required: bool,
    comments: &'a str,
    group: &'a str,
    default: Option<&'a str>,
    example: Option<&'a str>,
}

impl<'a> From<&'a Config> for Field<'a> {
    fn from(c: &'a Config) -> Self {
        Field {
            name: &c.name,
            value: c.value,
            required: !c.optional,
            comments: &c.comments,
            group: c.group.as_deref().unwrap_or(DEFAULT_GROUP),
            default: c.default_value.as_deref(),
            example: c.example.as_deref(),
        }
    }
}

impl From<&Config> for ConfigDoc {
    fn from(c: &Config) -> Self {
        ConfigDoc {
            name: c.name.clone(),
            type_name: friendly_type(c.value).to_string(),
            required: !c.optional,
            group: c.group.clone().unwrap_or_else(|| DEFAULT_GROUP.to_string()),
            default: c.default_value.clone(),
            example: c.example.clone(),
            deprecated: c.deprecated.clone(),
            comments: c.comments.clone(),
        }
    }
}

/// Cluster fields by group, in the given group order, dropping empty groups.
fn group_fields<'a>(
    fields: &'a [Field<'a>],
    groups: &'a [String],
) -> Vec<(&'a str, Vec<&'a Field<'a>>)> {
    groups
        .iter()
        .map(|g| {
            let items: Vec<&Field> = fields.iter().filter(|f| f.group == g).collect();
            (g.as_str(), items)
        })
        .filter(|(_, items)| !items.is_empty())
        .collect()
}

fn friendly_type(ty: ConfigType) -> &'static str {
    match ty {
        ConfigType::Bool => "bool",
        ConfigType::String => "string",
        ConfigType::Duration => "duration",
        ConfigType::Usize
        | ConfigType::U64
        | ConfigType::I64
        | ConfigType::U32
        | ConfigType::U16 => "integer",
        ConfigType::Vec => "list",
        ConfigType::HashMap => "map",
    }
}

/// Example string value for a field. Prefers an explicit `@default`; otherwise
/// falls back to a per-type placeholder. Every binding here passes config as a
/// `String -> String` map, so even non-string types are quoted strings that
/// OpenDAL's `ConfigDeserializer` parses (e.g. `"true"`, `"10s"`, `"a,b"`).
fn value_of<'a>(f: &Field<'a>) -> &'a str {
    let placeholder = match f.value {
        ConfigType::Bool => "true",
        ConfigType::String | ConfigType::HashMap => "...",
        ConfigType::Duration => "10s",
        ConfigType::Usize
        | ConfigType::U64
        | ConfigType::I64
        | ConfigType::U32
        | ConfigType::U16 => "1000",
        ConfigType::Vec => "value1,value2",
    };
    f.default.or(f.example).unwrap_or(placeholder)
}

/// Per-binding syntax used by the generic snippet builder.
struct Syntax {
    /// Import lines plus the opening of the call, ready for fields to follow.
    open: String,
    /// Text closing the call.
    close: String,
    /// Indentation for field lines.
    indent: &'static str,
    /// Line-comment marker, e.g. `# ` or `// `.
    comment: &'static str,
    /// Render a single `key = value` assignment (no indent, no comment marker).
    assign: fn(&Field) -> String,
}

/// Build the `(minimal, full)` snippet pair for one binding.
///
/// `minimal` lists only the required fields. `full` walks every group in order,
/// emitting a group header (when there is more than one group), each field's
/// doc comment, and the assignment — uncommented when required, commented out
/// otherwise so the block is a copy-and-trim reference.
fn build(required: &[&Field], grouped: &[(&str, Vec<&Field>)], s: &Syntax) -> (String, String) {
    let doc_prefix = format!("{}{}", s.indent, s.comment);

    let comment_block = |comments: &str, out: &mut String| {
        for line in comments.lines() {
            if line.trim().is_empty() {
                out.push_str(doc_prefix.trim_end());
            } else {
                out.push_str(&doc_prefix);
                out.push_str(line);
            }
            out.push('\n');
        }
    };

    let mut minimal = s.open.clone();
    for f in required {
        minimal.push_str(&format!("{}{}\n", s.indent, (s.assign)(f)));
    }
    minimal.push_str(&s.close);

    let multi = grouped.len() > 1;
    let mut full = s.open.clone();
    for (group, fields) in grouped {
        if multi {
            full.push_str(&format!("{}{}--- {group} ---\n", s.indent, s.comment));
        }
        let mut config_options = vec![];
        for field in fields {
            let mut field_comment = String::new();
            comment_block(field.comments, &mut field_comment);
            let assign = (s.assign)(field);
            // field have block indententation
            if field.required {
                config_options.push(format!("{field_comment}{}{assign}", s.indent));
            } else {
                config_options.push(format!("{field_comment}{}{}{assign}", s.indent, s.comment));
            }
        }

        if !config_options.is_empty() {
            full.push_str(config_options.join("\n\n").trim_end());
            full.push('\n');
        }
    }
    full.push_str(&s.close);

    (minimal, full)
}

fn render_examples(
    binding: &str,
    scheme: &str,
    required: &[&Field],
    grouped: &[(&str, Vec<&Field>)],
) -> (String, String) {
    let syntax = match binding {
        "rust" => Syntax {
            open: format!("use opendal::Operator;\n\nlet operator = Operator::via_iter(\"{scheme}\", [\n"),
            close: "])?;".to_string(),
            indent: "    ",
            comment: "// ",
            assign: |f| {
                format!(
                    "(\"{}\".to_string(), \"{}\".to_string()),",
                    f.name,
                    value_of(f)
                )
            },
        },
        "python" => Syntax {
            open: format!("import opendal\n\noperator = opendal.Operator(\n    \"{scheme}\",\n"),
            close: ")".to_string(),
            indent: "    ",
            comment: "# ",
            assign: |f| format!("{}=\"{}\",", f.name, value_of(f)),
        },
        "nodejs" => Syntax {
            open: format!(
                "import {{ Operator }} from \"opendal\";\n\nconst operator = new Operator(\"{scheme}\", {{\n"
            ),
            close: "});".to_string(),
            indent: "  ",
            comment: "// ",
            assign: |f| format!("{}: \"{}\",", f.name, value_of(f)),
        },
        "java" => Syntax {
            open: "import java.util.HashMap;\nimport java.util.Map;\nimport org.apache.opendal.Operator;\n\nMap<String, String> config = new HashMap<>();\n".to_string(),
            close: format!("Operator operator = Operator.of(\"{scheme}\", config);"),
            indent: "",
            comment: "// ",
            assign: |f| format!("config.put(\"{}\", \"{}\");", f.name, value_of(f)),
        },
        "go" => Syntax {
            open: format!(
                "import (\n	\"github.com/apache/opendal-go-services/{scheme}\"\n	opendal \"github.com/apache/opendal/bindings/go\"\n)\n\noperator, err := opendal.NewOperator({scheme}.Scheme, opendal.OperatorOptions{{\n",
            ),
            close: "})".to_string(),
            indent: "	",
            comment: "// ",
            assign: |f| format!("\"{}\": \"{}\",", f.name, value_of(f)),
        },
        "c" => Syntax {
            open: "#include \"opendal.h\"\n\nopendal_operator_options *options = opendal_operator_options_new();\n".to_string(),
            close: format!(
                "opendal_result_operator_new result = opendal_operator_new(\"{scheme}\", options);\nopendal_operator *operator = result.op;\n\n/* ... use operator ... */\n\nopendal_operator_free(operator);\nopendal_operator_options_free(options);"
            ),
            indent: "",
            comment: "// ",
            assign: |f| {
                format!(
                    "opendal_operator_options_set(options, \"{}\", \"{}\");",
                    f.name,
                    value_of(f)
                )
            },
        },
        "cpp" => Syntax {
            open: "#include \"opendal.hpp\"\n\nstd::unordered_map<std::string, std::string> config{\n".to_string(),
            close: format!("}};\nopendal::Operator operator(\"{scheme}\", config);"),
            indent: "    ",
            comment: "// ",
            assign: |f| format!("{{\"{}\", \"{}\"}},", f.name, value_of(f)),
        },
        "ruby" => Syntax {
            open: format!(
                "require \"opendal\"\n\noperator = OpenDal::Operator.new(\"{scheme}\", {{\n"
            ),
            close: "})".to_string(),
            indent: "  ",
            comment: "# ",
            assign: |f| format!("\"{}\" => \"{}\",", f.name, value_of(f)),
        },
        other => unreachable!("no renderer for binding {other}"),
    };

    build(required, grouped, &syntax)
}
