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

use crate::{find_command, workspace_dir};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha512};
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

mod bump;
mod package;

pub fn update_version(
    baseline: Option<&str>,
    patch: bool,
    sync: bool,
    breaking: &[String],
    report: Option<&Path>,
) -> anyhow::Result<()> {
    let baseline = baseline
        .map(str::to_owned)
        .map_or_else(bump::latest_final_release_tag, Ok)?;
    let mut packages = package::all_packages();
    let configured = packages.clone();
    let mut previous = std::collections::BTreeMap::new();
    if patch || sync {
        let mut command = find_command("git", workspace_dir());
        let output = command
            .args(["show", &format!("{baseline}:dev/src/release/package.rs")])
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "failed to read baseline package inventory"
        );
        previous = package::inventory_versions(std::str::from_utf8(&output.stdout)?)?;
        package::prepare_versions(
            &mut packages,
            std::str::from_utf8(&output.stdout)?,
            patch,
            breaking,
        )?;
    }
    let dependency_reasons = if patch {
        bump::prepare_public_dependencies(&mut packages, &baseline)?
    } else {
        std::collections::BTreeMap::new()
    };
    bump::validate_release_versions(&packages, Some(&baseline))?;
    if let Some(path) = report {
        let decisions = packages
            .iter()
            .zip(&configured)
            .map(|(target, configured)| {
                serde_json::json!({
                    "package": target.name(),
                    "previous": previous.get(target.name()).map(ToString::to_string),
                    "configured": configured.version().to_string(),
                    "target": target.version().to_string(),
                    "breaking": breaking.iter().any(|name| name == target.name()),
                    "public_dependency_reason": dependency_reasons.get(target.name()),
                })
            })
            .collect::<Vec<_>>();
        std::fs::write(path, serde_json::to_string_pretty(&decisions)?)?;
    }
    if patch || sync {
        std::fs::write(
            workspace_dir().join("dev/src/release/package.rs"),
            package::render_inventory(&packages)?,
        )?;
    }

    let mut updated = false;
    for package in packages {
        updated |= package::update_package_version(&package);
    }
    if !updated {
        println!("all package versions are up-to-date");
    }
    Ok(())
}

pub fn print_packages() -> anyhow::Result<()> {
    let packages = package::all_packages();
    let inventory = packages
        .iter()
        .map(|p| {
            serde_json::json!({
                "path": p.name(), "version": p.version().to_string(),
                "dependencies": p.dependencies().iter().map(|d| d.name()).collect::<Vec<_>>(),
                "public_dependencies": p.public_compat_dependencies(),
            })
        })
        .collect::<Vec<_>>();
    println!("{}", serde_json::to_string_pretty(&inventory)?);
    Ok(())
}

pub fn archive_package(sign: bool) -> anyhow::Result<()> {
    std::fs::create_dir_all(workspace_dir().join("dist"))?;

    let workspace_dir = workspace_dir();
    let dist_dir = workspace_dir.join("dist");

    let packages = package::all_packages();
    for package in packages {
        let mut cmd = find_command("git", &workspace_dir);
        // Package symlinks point at these root files, so every archive ships them.
        cmd.args([
            "ls-files",
            "--stage",
            "-z",
            "--",
            "LICENSE",
            "NOTICE",
            "CHANGELOG.md",
        ]);
        cmd.arg(package.name());
        for dep in package.dependencies() {
            cmd.arg(dep.name());
        }
        let output = cmd.output()?;
        anyhow::ensure!(output.status.success(), "git ls-files failed");
        let files = archive_entries(&output.stdout)?;
        archive_and_checksum(&package, &files)?;

        if !sign {
            continue;
        }

        let filename = format!("{}.tar.gz", package.make_prefix());

        println!("Generate signature for package: {}", package.name());
        let mut cmd = find_command("gpg", &dist_dir);
        cmd.args([
            "--yes",
            "--armor",
            "--output",
            format!("{filename}.asc").as_str(),
            "--detach-sign",
            filename.as_str(),
        ]);
        anyhow::ensure!(cmd.status()?.success(), "failed to sign {filename}");

        println!("Check signature for package: {}", package.name());
        let mut cmd = find_command("gpg", &dist_dir);
        cmd.args([
            "--verify",
            format!("{filename}.asc").as_str(),
            filename.as_str(),
        ]);
        anyhow::ensure!(cmd.status()?.success(), "failed to verify {filename}");
    }

    Ok(())
}

fn archive_and_checksum(package: &package::Package, files: &[ArchiveEntry]) -> anyhow::Result<()> {
    println!("Archiving package: {}", package.name());

    let prefix = package.make_prefix();
    let filename = format!("{prefix}.tar.gz");
    let tarball = workspace_dir().join("dist").join(&filename);

    write_archive(&workspace_dir(), &tarball, &prefix, files)?;

    {
        let tarball = std::fs::File::open(&tarball)?;
        let mut reader = BufReader::new(tarball);
        let mut hasher = Sha512::new();
        let mut buf = [0; 8 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let digest = hasher.finalize();
        let checksum_lines = format!("{}  {filename}", format_digest_hex(digest));

        let checksum = workspace_dir()
            .join("dist")
            .join(format!("{filename}.sha512"));
        std::fs::write(checksum, checksum_lines)?;
    }

    Ok(())
}

struct ArchiveEntry {
    path: String,
    mode: u32,
}

fn archive_entries(index: &[u8]) -> anyhow::Result<Vec<ArchiveEntry>> {
    let mut entries = Vec::new();
    for entry in index
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry)?;
        let (metadata, path) = entry
            .split_once('\t')
            .ok_or_else(|| anyhow::anyhow!("invalid Git index entry"))?;
        let fields = metadata.split_whitespace().collect::<Vec<_>>();
        anyhow::ensure!(
            fields.len() == 3 && fields[2] == "0",
            "unmerged Git index entry: {path}"
        );
        let mode = u32::from_str_radix(fields[0], 8)?;
        anyhow::ensure!(
            matches!(mode, 0o100644 | 0o100755 | 0o120000),
            "unsupported Git mode for {path}"
        );
        entries.push(ArchiveEntry {
            path: path.to_owned(),
            mode,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn write_archive(
    root: &Path,
    output: &Path,
    prefix: &str,
    files: &[ArchiveEntry],
) -> anyhow::Result<()> {
    let encoder = GzEncoder::new(std::fs::File::create(output)?, Compression::default());
    let mut tar = tar::Builder::new(encoder);
    for entry in files {
        // Git owns executable bits; checkout timestamps, owners and umask must not
        // affect the release bytes. Gzip's default header has no timestamp/name.
        let mut header = tar::Header::new_gnu();
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        let path = root.join(&entry.path);
        let archive_path = format!("{prefix}/{}", entry.path);
        if entry.mode == 0o120000 {
            header.set_mode(0o777);
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            tar.append_link(&mut header, archive_path, std::fs::read_link(path)?)?;
        } else {
            header.set_mode(entry.mode & 0o777);
            header.set_entry_type(tar::EntryType::Regular);
            let file = std::fs::File::open(path)?;
            header.set_size(file.metadata()?.len());
            tar.append_data(&mut header, archive_path, file)?;
        }
    }
    tar.into_inner()?.finish()?;
    Ok(())
}

fn format_digest_hex(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    let digest = digest.as_ref();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String must succeed");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::FileTimes;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn archive_ignores_checkout_metadata() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("opendal-archive-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let file = root.join("script");
        std::fs::write(&file, b"#!/bin/sh\necho reproducible\n")?;
        let entries = archive_entries(b"100755 abc 0\tscript\0")?;
        for (name, seconds) in [("a.tar.gz", 1_700_000_000), ("b.tar.gz", 1_800_000_000)] {
            std::fs::File::open(&file)?.set_times(
                FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds)),
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(
                    &file,
                    std::fs::Permissions::from_mode(if name.starts_with('a') {
                        0o600
                    } else {
                        0o755
                    }),
                )?;
            }
            write_archive(&root, &root.join(name), "source", &entries)?;
        }
        assert_eq!(
            std::fs::read(root.join("a.tar.gz"))?,
            std::fs::read(root.join("b.tar.gz"))?
        );
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(std::fs::File::open(
            root.join("a.tar.gz"),
        )?));
        let entry = archive.entries()?.next().unwrap()?;
        assert_eq!(entry.header().mode()?, 0o755);
        assert_eq!(entry.header().mtime()?, 0);
        assert_eq!(entry.header().uid()?, 0);
        assert_eq!(entry.header().gid()?, 0);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn archive_preserves_symlinks_without_following_them() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!("opendal-symlink-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        std::os::unix::fs::symlink("missing-target", root.join("link"))?;
        let entries = archive_entries(b"120000 abc 0\tlink\0")?;
        write_archive(&root, &root.join("source.tar.gz"), "source", &entries)?;
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(std::fs::File::open(
            root.join("source.tar.gz"),
        )?));
        let entry = archive.entries()?.next().unwrap()?;
        assert!(entry.header().entry_type().is_symlink());
        assert_eq!(entry.link_name()?.unwrap(), Path::new("missing-target"));
        assert_eq!(entry.size(), 0);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn index_paths_are_sorted_and_conflicts_rejected() -> anyhow::Result<()> {
        let entries = archive_entries(b"100644 a 0\tz\x00100644 b 0\ta\tname\0")?;
        assert_eq!(entries[0].path, "a\tname");
        assert!(archive_entries(b"100644 a 1\tconflict\0").is_err());
        assert!(archive_entries(b"160000 a 0\tsubmodule\0").is_err());
        Ok(())
    }
}
