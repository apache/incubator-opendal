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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use prost::Message;
use prost_types::DescriptorProto;
use prost_types::FieldDescriptorProto;
use prost_types::FileDescriptorProto;
use prost_types::FileDescriptorSet;
use prost_types::MethodDescriptorProto;

const ASF_HEADER: &str = r#"// Licensed to the Apache Software Foundation (ASF) under one
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

"#;

pub fn generate(workspace_dir: PathBuf) -> Result<()> {
    let service_dir = workspace_dir.join("core/services/gcs-grpc");
    let proto_dir = service_dir.join("proto");
    let output_dir = service_dir.join("src/generated");
    let descriptor_path = output_dir.join("descriptor.bin");
    fs::create_dir_all(&output_dir)?;
    remove_generated_files(&output_dir)?;

    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .bytes(".")
        .out_dir(&output_dir)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(
            &[proto_dir.join("google/storage/v2/storage.proto")],
            std::slice::from_ref(&proto_dir),
        )
        .context("failed to generate the GCS gRPC client")?;

    let descriptor = fs::read(&descriptor_path)?;
    fs::remove_file(&descriptor_path)?;
    let upstream_descriptor = fs::read(proto_dir.join("upstream-descriptor.bin"))?;
    validate_compatibility(&descriptor, &upstream_descriptor)?;

    for entry in fs::read_dir(&output_dir)? {
        let path = entry?.path();
        if path.file_name().and_then(|v| v.to_str()) == Some("mod.rs")
            || path.extension().and_then(|v| v.to_str()) != Some("rs")
        {
            continue;
        }

        let generated = fs::read_to_string(&path)?;
        if generated.starts_with(ASF_HEADER) {
            continue;
        }
        fs::write(&path, format!("{ASF_HEADER}{generated}"))?;
    }

    Ok(())
}

fn remove_generated_files(output_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(output_dir)? {
        let path = entry?.path();
        if path.file_name().and_then(|v| v.to_str()) != Some("mod.rs")
            && path.extension().and_then(|v| v.to_str()) == Some("rs")
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn validate_compatibility(descriptor: &[u8], upstream_descriptor: &[u8]) -> Result<()> {
    let descriptor = FileDescriptorSet::decode(descriptor)?;
    let upstream_descriptor = FileDescriptorSet::decode(upstream_descriptor)?;
    let file = storage_file(&descriptor)?;
    let upstream_file = storage_file(&upstream_descriptor)?;

    let service = file
        .service
        .iter()
        .find(|service| service.name.as_deref() == Some("Storage"))
        .ok_or_else(|| anyhow!("generated descriptor does not define Storage"))?;
    let upstream_service = upstream_file
        .service
        .iter()
        .find(|service| service.name.as_deref() == Some("Storage"))
        .ok_or_else(|| anyhow!("upstream descriptor does not define Storage"))?;

    const RETAINED_METHODS: [&str; 9] = [
        "CancelResumableWrite",
        "DeleteObject",
        "GetObject",
        "ListObjects",
        "QueryWriteStatus",
        "ReadObject",
        "RewriteObject",
        "StartResumableWrite",
        "WriteObject",
    ];
    let mut method_names = service
        .method
        .iter()
        .map(|method| method.name.as_deref().unwrap_or_default())
        .collect::<Vec<_>>();
    method_names.sort_unstable();
    if method_names != RETAINED_METHODS {
        bail!("generated Storage methods do not match the retained method set");
    }
    for method in &service.method {
        let name = method.name.as_deref().unwrap_or_default();
        let upstream_method = upstream_service
            .method
            .iter()
            .find(|candidate| candidate.name.as_deref() == Some(name))
            .ok_or_else(|| anyhow!("upstream Storage does not define {name}"))?;
        validate_method(method, upstream_method)?;
    }

    let package = file.package.as_deref().unwrap_or_default();
    let upstream_package = upstream_file.package.as_deref().unwrap_or_default();
    if package != upstream_package {
        bail!("generated package {package} does not match upstream package {upstream_package}");
    }
    let messages = collect_messages(file);
    let upstream_messages = collect_messages(upstream_file);
    for (name, message) in messages {
        let upstream_message = upstream_messages
            .get(&name)
            .ok_or_else(|| anyhow!("upstream descriptor does not define {name}"))?;
        validate_message(&name, message, upstream_message)?;
    }

    Ok(())
}

fn storage_file(descriptor: &FileDescriptorSet) -> Result<&FileDescriptorProto> {
    descriptor
        .file
        .iter()
        .find(|file| file.name.as_deref() == Some("google/storage/v2/storage.proto"))
        .ok_or_else(|| anyhow!("descriptor does not contain google/storage/v2/storage.proto"))
}

fn validate_method(method: &MethodDescriptorProto, upstream: &MethodDescriptorProto) -> Result<()> {
    let name = method.name.as_deref().unwrap_or_default();
    if method.input_type != upstream.input_type
        || method.output_type != upstream.output_type
        || method.client_streaming.unwrap_or(false) != upstream.client_streaming.unwrap_or(false)
        || method.server_streaming.unwrap_or(false) != upstream.server_streaming.unwrap_or(false)
    {
        bail!("method {name} is incompatible with the upstream descriptor");
    }
    Ok(())
}

fn collect_messages(file: &FileDescriptorProto) -> BTreeMap<String, &DescriptorProto> {
    let mut messages = BTreeMap::new();
    let package = file.package.as_deref().unwrap_or_default();
    for message in &file.message_type {
        collect_message(package, message, &mut messages);
    }
    messages
}

fn collect_message<'a>(
    parent: &str,
    message: &'a DescriptorProto,
    messages: &mut BTreeMap<String, &'a DescriptorProto>,
) {
    let name = message.name.as_deref().unwrap_or_default();
    let full_name = format!("{parent}.{name}");
    messages.insert(full_name.clone(), message);
    for nested in &message.nested_type {
        collect_message(&full_name, nested, messages);
    }
}

fn validate_message(
    name: &str,
    message: &DescriptorProto,
    upstream: &DescriptorProto,
) -> Result<()> {
    if message
        .options
        .as_ref()
        .and_then(|options| options.map_entry)
        != upstream
            .options
            .as_ref()
            .and_then(|options| options.map_entry)
    {
        bail!("message {name} has an incompatible map entry declaration");
    }
    for field in &message.field {
        let number = field.number.unwrap_or_default();
        let upstream_field = upstream
            .field
            .iter()
            .find(|candidate| candidate.number == field.number)
            .ok_or_else(|| anyhow!("upstream message {name} does not define field {number}"))?;
        validate_field(name, message, field, upstream, upstream_field)?;
    }
    Ok(())
}

fn validate_field(
    message_name: &str,
    message: &DescriptorProto,
    field: &FieldDescriptorProto,
    upstream_message: &DescriptorProto,
    upstream: &FieldDescriptorProto,
) -> Result<()> {
    let field_name = field.name.as_deref().unwrap_or_default();
    if field.name != upstream.name
        || field.label != upstream.label
        || field.r#type != upstream.r#type
        || field.type_name != upstream.type_name
        || field.proto3_optional.unwrap_or(false) != upstream.proto3_optional.unwrap_or(false)
        || oneof_name(message, field) != oneof_name(upstream_message, upstream)
    {
        bail!("field {message_name}.{field_name} is incompatible with the upstream descriptor");
    }
    Ok(())
}

fn oneof_name<'a>(message: &'a DescriptorProto, field: &FieldDescriptorProto) -> Option<&'a str> {
    field
        .oneof_index
        .and_then(|index| message.oneof_decl.get(index as usize))
        .and_then(|oneof| oneof.name.as_deref())
}
