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

use super::package::Package;
use crate::{find_command, workspace_dir};
use anyhow::{Context, bail};
use semver::{Op, Version, VersionReq};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use toml_edit::{DocumentMut, Item};

pub fn validate_release_versions(
    packages: &[Package],
    baseline: Option<&str>,
) -> anyhow::Result<()> {
    let baseline = match baseline {
        Some(tag) => {
            let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))?;
            anyhow::ensure!(
                version.pre.is_empty() && version.build.is_empty(),
                "baseline must be a final release"
            );
            tag.to_string()
        }
        None => latest_final_release_tag()?,
    };
    let mut violations = Vec::new();

    for package in packages {
        if package.public_compat_dependencies().is_empty() {
            continue;
        }

        if let Some(violation) = validate_package(package, &baseline)? {
            violations.push(violation);
        }
    }

    if !violations.is_empty() {
        let details = violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        bail!("release version validation failed against {baseline}:\n{details}");
    }

    println!("release version targets are compatible with changes since {baseline}");
    Ok(())
}

/// Raise integration versions when their public dependencies cross compatibility lines.
pub(super) fn prepare_public_dependencies(
    packages: &mut [Package],
    baseline: &str,
) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
    let mut reasons = std::collections::BTreeMap::new();
    loop {
        let mut targets = std::collections::BTreeMap::new();
        for package in packages.iter() {
            if !package.public_compat_dependencies().is_empty()
                && let Some(violation) = validate_package(package, baseline)?
            {
                reasons.insert(
                    package.name().to_string(),
                    violation
                        .changes
                        .iter()
                        .map(|change| {
                            format!("{}: {} -> {}", change.name, change.previous, change.target)
                        })
                        .collect::<Vec<_>>()
                        .join("; "),
                );
                targets.insert(package.name().to_string(), violation.required);
            }
        }
        if targets.is_empty() {
            return Ok(reasons);
        }
        super::package::apply_versions(packages, &targets);
    }
}

fn validate_package(package: &Package, baseline: &str) -> anyhow::Result<Option<Violation>> {
    let baseline_manifest = manifest_at(baseline, package)?;
    let current_manifest = CargoManifest::read(package.path().join("Cargo.toml"))?;
    let baseline_package_version = baseline_manifest.package_version()?;
    let mut changes = Vec::new();

    for dependency in package.public_compat_dependencies() {
        let previous = baseline_manifest.dependency_version(dependency)?;
        // Managed dependency constraints have not been written to this manifest yet.
        let target = match package.release_dependency_version(dependency) {
            Some(version) => version.clone(),
            None => current_manifest.dependency_version(dependency)?,
        };

        if !same_cargo_compatibility_line(&previous, &target) {
            changes.push(DependencyChange {
                name: dependency.to_string(),
                previous,
                target,
            });
        }
    }

    Ok(validate_target_version(
        package.name(),
        baseline_package_version,
        package.version().clone(),
        changes,
    ))
}

pub(super) fn latest_final_release_tag() -> anyhow::Result<String> {
    let mut cmd = find_command("git", workspace_dir());
    cmd.args([
        "describe",
        "--tags",
        "--abbrev=0",
        "--match",
        "v[0-9]*",
        "--exclude",
        "*-*",
        "HEAD",
    ]);
    let output = cmd
        .output()
        .context("failed to find the latest final release tag")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to find the latest final release tag; fetch the repository tags before running update-version: {}",
            stderr.trim()
        );
    }

    let tag = String::from_utf8(output.stdout)
        .context("latest final release tag is not valid UTF-8")?
        .trim()
        .to_string();
    let version = tag.strip_prefix('v').unwrap_or(&tag);
    let version = Version::parse(version)
        .with_context(|| format!("release tag {tag} is not a semantic version"))?;
    if !version.pre.is_empty() {
        bail!("release tag {tag} is not a final release");
    }

    Ok(tag)
}

fn manifest_at(reference: &str, package: &Package) -> anyhow::Result<CargoManifest> {
    let manifest = format!("{reference}:{}/Cargo.toml", package.name());
    let mut cmd = find_command("git", workspace_dir());
    cmd.args(["show", &manifest]);
    let output = cmd
        .output()
        .with_context(|| format!("failed to read {manifest}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to read {manifest}: {}", stderr.trim());
    }

    let content = String::from_utf8(output.stdout)
        .with_context(|| format!("{manifest} is not valid UTF-8"))?;
    CargoManifest::parse(&content, &manifest)
}

struct CargoManifest {
    document: DocumentMut,
    source: String,
}

impl CargoManifest {
    fn read(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::parse(&content, &path.display().to_string())
    }

    fn parse(content: &str, source: &str) -> anyhow::Result<Self> {
        let document =
            DocumentMut::from_str(content).with_context(|| format!("failed to parse {source}"))?;
        Ok(Self {
            document,
            source: source.to_string(),
        })
    }

    fn package_version(&self) -> anyhow::Result<Version> {
        let version = self.document["package"]["version"]
            .as_str()
            .with_context(|| format!("missing package.version in {}", self.source))?;
        Version::parse(version)
            .with_context(|| format!("invalid package.version in {}: {version}", self.source))
    }

    fn dependency_version(&self, name: &str) -> anyhow::Result<Version> {
        let dependencies = self
            .document
            .get("dependencies")
            .and_then(Item::as_table_like)
            .with_context(|| format!("missing dependencies in {}", self.source))?;
        let dependency = dependencies.get(name).with_context(|| {
            format!(
                "missing public compatibility dependency {name} in {}",
                self.source
            )
        })?;
        let requirement = dependency.as_str().or_else(|| {
            dependency
                .as_table_like()
                .and_then(|dependency| dependency.get("version"))
                .and_then(Item::as_str)
        });
        let requirement = requirement
            .with_context(|| format!("missing version for dependency {name} in {}", self.source))?;

        dependency_requirement_version(requirement).with_context(|| {
            format!(
                "unsupported version requirement for dependency {name} in {}: {requirement}",
                self.source
            )
        })
    }
}

fn dependency_requirement_version(requirement: &str) -> anyhow::Result<Version> {
    let requirement = VersionReq::parse(requirement)?;
    let [comparator] = requirement.comparators.as_slice() else {
        bail!("expected one version comparator");
    };
    if !matches!(comparator.op, Op::Caret | Op::Exact | Op::Tilde) {
        bail!("expected a caret, exact, or tilde version comparator");
    }

    let mut version = Version::new(
        comparator.major,
        comparator.minor.unwrap_or(0),
        comparator.patch.unwrap_or(0),
    );
    version.pre = comparator.pre.clone();
    Ok(version)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DependencyChange {
    name: String,
    previous: Version,
    target: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Violation {
    package: String,
    baseline: Version,
    target: Version,
    required: Version,
    changes: Vec<DependencyChange>,
}

impl Display for Violation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  {} must use an incompatible version bump: configured {}, expected at least {} from baseline {}",
            self.package, self.target, self.required, self.baseline
        )?;
        for change in &self.changes {
            writeln!(
                f,
                "    - {} changed from {} to {}",
                change.name, change.previous, change.target
            )?;
        }
        Ok(())
    }
}

fn validate_target_version(
    package: &str,
    baseline: Version,
    target: Version,
    changes: Vec<DependencyChange>,
) -> Option<Violation> {
    if changes.is_empty() {
        return None;
    }

    let required = next_incompatible_version(&baseline);
    if target >= required {
        return None;
    }

    Some(Violation {
        package: package.to_string(),
        baseline,
        target,
        required,
        changes,
    })
}

fn same_cargo_compatibility_line(previous: &Version, target: &Version) -> bool {
    if !previous.pre.is_empty() || !target.pre.is_empty() {
        return previous == target;
    }
    if previous.major != target.major {
        return false;
    }
    if previous.major != 0 {
        return true;
    }
    if previous.minor != target.minor {
        return false;
    }
    if previous.minor != 0 {
        return true;
    }
    previous.patch == target.patch
}

pub(super) fn next_incompatible_version(version: &Version) -> Version {
    if version.major != 0 {
        Version::new(version.major + 1, 0, 0)
    } else if version.minor != 0 {
        Version::new(0, version.minor + 1, 0)
    } else {
        Version::new(0, 0, version.patch + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(value: &str) -> Version {
        Version::parse(value).unwrap()
    }

    fn change(name: &str, previous: &str, target: &str) -> DependencyChange {
        DependencyChange {
            name: name.to_string(),
            previous: version(previous),
            target: version(target),
        }
    }

    #[test]
    fn object_store_incompatible_upgrade_requires_integration_bump() {
        let violation = validate_target_version(
            "integrations/object_store",
            version("0.58.0"),
            version("0.58.1"),
            vec![change("object_store", "0.13.1", "0.14.1")],
        )
        .unwrap();

        assert_eq!(violation.required, version("0.59.0"));
    }

    #[test]
    fn parquet_incompatible_upgrade_accepts_integration_bump() {
        let violation = validate_target_version(
            "integrations/parquet",
            version("0.8.2"),
            version("0.9.0"),
            vec![change("parquet", "58.0.0", "59.0.0")],
        );

        assert!(violation.is_none());
    }

    #[test]
    fn opendal_incompatible_upgrade_requires_parquet_bump() {
        let violation = validate_target_version(
            "integrations/parquet",
            version("0.9.1"),
            version("0.9.2"),
            vec![change("opendal", "0.58.2", "0.59.0")],
        )
        .unwrap();

        assert_eq!(violation.required, version("0.10.0"));
    }

    #[test]
    fn compatible_dependency_upgrade_does_not_require_incompatible_bump() {
        assert!(same_cargo_compatibility_line(
            &version("0.58.1"),
            &version("0.58.2")
        ));
        assert!(same_cargo_compatibility_line(
            &version("1.2.0"),
            &version("1.9.0")
        ));
        assert!(!same_cargo_compatibility_line(
            &version("0.0.1"),
            &version("0.0.2")
        ));
        assert_eq!(
            next_incompatible_version(&version("0.58.1")),
            version("0.59.0")
        );
        assert_eq!(
            next_incompatible_version(&version("0.0.1")),
            version("0.0.2")
        );
        assert_eq!(
            next_incompatible_version(&version("1.2.3")),
            version("2.0.0")
        );
    }

    #[test]
    fn parses_cargo_dependency_requirements() {
        assert_eq!(
            dependency_requirement_version("0.14.1").unwrap(),
            version("0.14.1")
        );
        assert_eq!(
            dependency_requirement_version("59.0").unwrap(),
            version("59.0.0")
        );
    }

    #[test]
    fn reads_inline_dependency_versions() {
        let manifest = CargoManifest::parse(
            r#"
[package]
name = "parquet_opendal"
version = "0.9.0"

[dependencies]
opendal = { version = "0.58.1", path = "../../core" }
parquet = { version = "59.0", default-features = false }
"#,
            "Cargo.toml",
        )
        .unwrap();

        assert_eq!(manifest.package_version().unwrap(), version("0.9.0"));
        assert_eq!(
            manifest.dependency_version("opendal").unwrap(),
            version("0.58.1")
        );
        assert_eq!(
            manifest.dependency_version("parquet").unwrap(),
            version("59.0.0")
        );
    }
}
