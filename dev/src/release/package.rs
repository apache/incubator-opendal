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

use crate::workspace_dir;
use semver::Version;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::{DocumentMut, Item, TableLike};

#[derive(Debug, Clone)]
pub struct Package {
    name: String,
    path: PathBuf,
    version: Version,
    dependencies: Vec<Package>,
    public_compat_dependencies: &'static [&'static str],
}

impl Package {
    pub fn dependencies(&self) -> &[Package] {
        &self.dependencies
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn version(&self) -> &Version {
        &self.version
    }

    pub(super) fn public_compat_dependencies(&self) -> &'static [&'static str] {
        self.public_compat_dependencies
    }

    pub(super) fn release_dependency_version(&self, crate_name: &str) -> Option<&Version> {
        self.dependencies
            .iter()
            .find(|dependency| dependency.crate_names().contains(&crate_name))
            .map(|dependency| &dependency.version)
    }

    fn with_public_compat_dependencies(mut self, dependencies: &'static [&'static str]) -> Self {
        self.public_compat_dependencies = dependencies;
        self
    }

    pub fn make_prefix(&self) -> String {
        format!(
            "apache-opendal-{}-{}-src",
            self.name.replace("/", "-"),
            self.version
        )
    }
}

fn make_package(path: &str, version: &str, dependencies: Vec<Package>) -> Package {
    let name = path.to_string();
    let path = workspace_dir().join(path);
    let version = Version::parse(version).unwrap();
    Package {
        name,
        path,
        version,
        dependencies,
        public_compat_dependencies: &[],
    }
}

/// List all packages that are ready for release.
pub fn all_packages() -> Vec<Package> {
    let core = make_package("core", "0.59.5", vec![]);

    // Integrations
    let dav_server = make_package("integrations/dav-server", "0.7.11", vec![core.clone()])
        .with_public_compat_dependencies(&["opendal-core"]);
    let object_store = make_package("integrations/object_store", "0.60.5", vec![core.clone()])
        .with_public_compat_dependencies(&["opendal", "object_store"]);
    let parquet = make_package("integrations/parquet", "0.10.5", vec![core.clone()])
        .with_public_compat_dependencies(&["opendal", "parquet"]);
    let unftp_sbe = make_package("integrations/unftp-sbe", "0.4.11", vec![core.clone()]);

    // Binaries moved to separate repositories; no longer released from this repo

    // Bindings
    let c = make_package("bindings/c", "0.47.8", vec![core.clone()]);
    let cpp = make_package("bindings/cpp", "0.45.35", vec![core.clone()]);
    let java = make_package("bindings/java", "0.50.8", vec![core.clone()]);
    let nodejs = make_package("bindings/nodejs", "0.49.13", vec![core.clone()]);
    let python = make_package("bindings/python", "0.47.12", vec![core.clone()]);
    let ruby = make_package("bindings/ruby", "0.1.16", vec![core.clone()]);
    let dotnet = make_package("bindings/dotnet", "0.3.1", vec![core.clone()]);

    vec![
        core,
        dav_server,
        object_store,
        parquet,
        unftp_sbe,
        c,
        cpp,
        java,
        nodejs,
        python,
        ruby,
        dotnet,
    ]
}

pub(super) fn inventory_versions(
    inventory: &str,
) -> anyhow::Result<std::collections::BTreeMap<String, Version>> {
    let matcher = regex::Regex::new(r#"make_package\("([^"]+)", "([^"]+)""#)?;
    matcher
        .captures_iter(inventory)
        .map(|c| Ok((c[1].to_string(), Version::parse(&c[2])?)))
        .collect()
}

/// Prepare inventory and dependency versions together before compatibility validation.
pub(super) fn prepare_versions(
    packages: &mut [Package],
    baseline: &str,
    patch: bool,
    breaking: &[String],
) -> anyhow::Result<()> {
    let baseline = inventory_versions(baseline)?;
    for name in breaking {
        anyhow::ensure!(
            packages.iter().any(|p| p.name() == name),
            "unknown breaking package: {name}"
        );
    }
    let mut targets = std::collections::BTreeMap::new();
    for package in packages.iter() {
        let target = match baseline.get(package.name()) {
            Some(previous) => {
                anyhow::ensure!(
                    previous.pre.is_empty() && previous.build.is_empty(),
                    "baseline package must be a final version"
                );
                let mut next = previous.clone();
                if breaking.iter().any(|name| name == package.name()) {
                    next = super::bump::next_incompatible_version(previous);
                } else if patch {
                    next.patch = next
                        .patch
                        .checked_add(1)
                        .ok_or_else(|| anyhow::anyhow!("patch version overflow"))?;
                }
                std::cmp::max(next, package.version.clone())
            }
            None => package.version.clone(),
        };
        targets.insert(package.name.clone(), target);
    }
    apply_versions(packages, &targets);
    Ok(())
}

pub(super) fn apply_versions(
    packages: &mut [Package],
    targets: &std::collections::BTreeMap<String, Version>,
) {
    for package in packages {
        if let Some(version) = targets.get(package.name()) {
            package.version = version.clone();
        }
        apply_versions(&mut package.dependencies, targets);
    }
}

pub(super) fn render_inventory(packages: &[Package]) -> anyhow::Result<String> {
    let matcher = regex::Regex::new(r#"make_package\("([^"]+)", "([^"]+)""#)?;
    let targets = packages
        .iter()
        .map(|p| (p.name(), p.version()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let inventory = std::fs::read_to_string(workspace_dir().join("dev/src/release/package.rs"))?;
    Ok(matcher
        .replace_all(&inventory, |c: &regex::Captures<'_>| {
            format!("make_package(\"{}\", \"{}\"", &c[1], targets[&c[1]])
        })
        .into_owned())
}

pub fn update_package_version(package: &Package) -> bool {
    match package.name.as_str() {
        "core" => update_cargo_version(&package.path, &package.version, package.dependencies()),
        "integrations/dav-server" => {
            update_cargo_version(&package.path, &package.version, package.dependencies())
        }
        "integrations/object_store" => {
            update_cargo_version(&package.path, &package.version, package.dependencies())
        }
        "integrations/parquet" => {
            update_cargo_version(&package.path, &package.version, package.dependencies())
        }
        "integrations/unftp-sbe" => {
            update_cargo_version(&package.path, &package.version, package.dependencies())
        }

        "bindings/c" => false,   // C bindings has no version to update
        "bindings/cpp" => false, // C++ bindings has no version to update
        "bindings/lua" => false, // Lua bindings has no version to update

        "bindings/python" => update_cargo_version(&package.path, &package.version, &[]),
        "bindings/ruby" => {
            update_cargo_version(&package.path, &package.version, package.dependencies())
        }
        "bindings/java" => update_maven_version(&package.path, &package.version),
        "bindings/nodejs" => update_nodejs_version(&package.path, &package.version),
        "bindings/dotnet" => update_dotnet_version(&package.path, &package.version),

        name => panic!("unknown package: {name}"),
    }
}

fn update_cargo_version(path: &Path, version: &Version, dependencies: &[Package]) -> bool {
    let manifest_path = path.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap();
    let mut manifest = DocumentMut::from_str(manifest.as_str()).unwrap();
    let mut updated = update_manifest_version(&mut manifest, version, &manifest_path);

    for dependency in dependencies {
        updated |= update_dependency_version(&mut manifest, path, dependency);
    }

    if updated {
        std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    }

    if is_core_workspace_root(path) {
        updated |= sync_workspace_internal_dependency_versions(path, version);
    }

    updated
}

fn update_manifest_version(manifest: &mut DocumentMut, version: &Version, path: &Path) -> bool {
    if let Some(old_version) = manifest["package"]["version"]
        .as_str()
        .map(Version::parse)
        .transpose()
        .unwrap()
    {
        if &old_version != version {
            manifest["package"]["version"] = toml_edit::value(version.to_string());
            println!(
                "updating version for package: {} from {} to {}",
                path.display(),
                old_version,
                version
            );
            return true;
        }

        return false;
    }

    if let Some(old_version) = manifest["workspace"]["package"]["version"]
        .as_str()
        .map(Version::parse)
        .transpose()
        .unwrap()
    {
        if &old_version != version {
            manifest["workspace"]["package"]["version"] = toml_edit::value(version.to_string());
            println!(
                "updating version for package: {} from {} to {}",
                path.display(),
                old_version,
                version
            );
            return true;
        }

        return false;
    }

    panic!("missing version for package: {}", path.display());
}

fn update_dependency_version(
    manifest: &mut DocumentMut,
    manifest_dir: &Path,
    dependency: &Package,
) -> bool {
    update_dependency_version_in_table(manifest.as_table_mut(), manifest_dir, dependency)
}

fn update_dependency_version_in_table(
    table: &mut dyn TableLike,
    manifest_dir: &Path,
    dependency: &Package,
) -> bool {
    let mut updated = false;

    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(dependencies) = table.get_mut(table_name).and_then(Item::as_table_like_mut) else {
            continue;
        };

        updated |=
            update_dependency_version_in_dependencies(dependencies, manifest_dir, dependency);
    }

    let Some(targets) = table.get_mut("target").and_then(Item::as_table_like_mut) else {
        return updated;
    };

    for (_, target) in targets.iter_mut() {
        let Some(target) = target.as_table_like_mut() else {
            continue;
        };
        updated |= update_dependency_version_in_table(target, manifest_dir, dependency);
    }

    updated
}

fn update_dependency_version_in_dependencies(
    dependencies: &mut dyn TableLike,
    manifest_dir: &Path,
    dependency: &Package,
) -> bool {
    let mut updated = false;

    for (crate_name, entry) in dependencies.iter_mut() {
        let Some(entry) = entry.as_table_like_mut() else {
            continue;
        };
        let Some(path) = entry.get("path").and_then(Item::as_str) else {
            continue;
        };
        if !path_points_into(manifest_dir, path, &dependency.path) {
            continue;
        }
        let Some(value) = entry.get_mut("version") else {
            continue;
        };

        let old_version = match value.as_str().map(Version::parse) {
            Some(Ok(version)) => version,
            _ => continue,
        };

        if old_version == dependency.version {
            continue;
        }

        *value = toml_edit::value(dependency.version.to_string());
        println!(
            "updating dependency version for crate: {} from {} to {}",
            crate_name, old_version, dependency.version
        );
        updated = true;
    }

    updated
}

fn is_core_workspace_root(path: &Path) -> bool {
    path == workspace_dir().join("core")
}

fn sync_workspace_internal_dependency_versions(workspace_root: &Path, version: &Version) -> bool {
    let workspace_root = normalize_path(workspace_root);
    let mut updated = false;

    for entry in ignore::Walk::new(&workspace_root) {
        let entry = entry.unwrap();
        if entry.file_name() != "Cargo.toml" {
            continue;
        }

        let manifest_path = entry.path();
        let manifest_dir = manifest_path.parent().unwrap();
        let manifest = std::fs::read_to_string(manifest_path).unwrap();
        let mut manifest = DocumentMut::from_str(&manifest).unwrap();

        let mut manifest_updated = sync_workspace_internal_dependency_versions_in_table(
            manifest.as_table_mut(),
            manifest_dir,
            &workspace_root,
            version,
        );
        manifest_updated |= sync_workspace_package_version(&mut manifest, version);
        if manifest_updated {
            std::fs::write(manifest_path, manifest.to_string()).unwrap();
            updated = true;
        }
    }

    updated
}

fn sync_workspace_package_version(manifest: &mut DocumentMut, version: &Version) -> bool {
    let Some(package) = manifest.get("package").and_then(Item::as_table_like) else {
        return false;
    };
    let Some(name) = package.get("name").and_then(Item::as_str) else {
        return false;
    };
    if !name.starts_with("opendal") {
        return false;
    }
    let Some(old_version) = manifest["package"]["version"]
        .as_str()
        .map(Version::parse)
        .transpose()
        .unwrap()
    else {
        return false;
    };
    if &old_version == version {
        return false;
    }

    manifest["package"]["version"] = toml_edit::value(version.to_string());
    true
}

fn sync_workspace_internal_dependency_versions_in_table(
    table: &mut dyn TableLike,
    manifest_dir: &Path,
    workspace_root: &Path,
    version: &Version,
) -> bool {
    let mut updated = false;

    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(dependencies) = table.get_mut(table_name).and_then(Item::as_table_like_mut) else {
            continue;
        };

        updated |= sync_workspace_internal_dependency_versions_in_dependencies(
            dependencies,
            manifest_dir,
            workspace_root,
            version,
        );
    }

    let Some(targets) = table.get_mut("target").and_then(Item::as_table_like_mut) else {
        return updated;
    };

    for (_, target) in targets.iter_mut() {
        let Some(target) = target.as_table_like_mut() else {
            continue;
        };
        updated |= sync_workspace_internal_dependency_versions_in_table(
            target,
            manifest_dir,
            workspace_root,
            version,
        );
    }

    updated
}

fn sync_workspace_internal_dependency_versions_in_dependencies(
    dependencies: &mut dyn TableLike,
    manifest_dir: &Path,
    workspace_root: &Path,
    version: &Version,
) -> bool {
    let mut updated = false;

    for (_, dependency) in dependencies.iter_mut() {
        let Some(dependency) = dependency.as_table_like_mut() else {
            continue;
        };
        let Some(path) = dependency.get("path").and_then(Item::as_str) else {
            continue;
        };
        if !path_points_into(manifest_dir, path, workspace_root) {
            continue;
        }
        let Some(value) = dependency.get_mut("version") else {
            continue;
        };

        let old_version = match value.as_str() {
            Some(version) => match Version::parse(version) {
                Ok(version) => version,
                Err(_) => continue,
            },
            None => continue,
        };

        if &old_version == version {
            continue;
        }

        *value = toml_edit::value(version.to_string());
        updated = true;
    }

    updated
}

fn path_points_into(manifest_dir: &Path, raw_path: &str, workspace_root: &Path) -> bool {
    normalize_path(&manifest_dir.join(raw_path)).starts_with(normalize_path(workspace_root))
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| lexical_normalize(path))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }

    normalized
}

impl Package {
    fn crate_names(&self) -> &'static [&'static str] {
        match self.name.as_str() {
            "core" => &["opendal", "opendal-core"],
            _ => &[],
        }
    }
}

fn update_maven_version(path: &Path, version: &Version) -> bool {
    let path = path.join("pom.xml");
    let manifest = std::fs::read_to_string(&path).unwrap();

    let old_version_matcher = r#"<version>\d+\.\d+\.\d+</version> <!-- update version number -->"#;
    let new_version_string = format!("<version>{version}</version> <!-- update version number -->");

    let new_manifest = regex::Regex::new(old_version_matcher)
        .unwrap()
        .replace_all(manifest.as_str(), new_version_string.as_str());
    if manifest != new_manifest {
        std::fs::write(&path, new_manifest.as_bytes()).unwrap();
        println!(
            "updating version for package: {} to {}",
            path.display(),
            version
        );
        true
    } else {
        false
    }
}

fn update_nodejs_version(path: &Path, version: &Version) -> bool {
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path.join("package.json")).unwrap()).unwrap();
    let previous = manifest["version"].as_str().unwrap();
    let loader = path.join("generated.js");
    let source = std::fs::read_to_string(&loader).unwrap();
    let updated_source = source.replace(previous, &version.to_string());
    let mut updated = source != updated_source;
    if updated {
        std::fs::write(loader, updated_source).unwrap();
    }

    for entry in ignore::Walk::new(path) {
        let entry = entry.unwrap();
        if entry.file_name() != "package.json" {
            continue;
        }
        // Ignore theme/package.json
        if entry.path().to_str().unwrap().contains("theme") {
            continue;
        }
        let manifest = std::fs::read_to_string(entry.path()).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();

        let value = manifest.pointer_mut("/version").unwrap();
        let old_version = Version::parse(value.as_str().unwrap()).unwrap();
        if &old_version != version {
            *value = serde_json::Value::String(version.to_string());
            let mut new_manifest = serde_json::to_string_pretty(&manifest).unwrap();
            new_manifest.push('\n');
            std::fs::write(entry.path(), new_manifest).unwrap();
            println!(
                "updating version for package: {} from {} to {}",
                entry.path().display(),
                old_version,
                version
            );
            updated = true;
        }
    }

    updated
}

fn update_dotnet_version(path: &Path, version: &Version) -> bool {
    let path = path.join("OpenDAL").join("OpenDAL.csproj");
    let manifest = std::fs::read_to_string(&path).unwrap();
    let matcher = regex::Regex::new(r"<VersionPrefix>(\d+\.\d+\.\d+)</VersionPrefix>").unwrap();
    let old_version = matcher
        .captures(&manifest)
        .and_then(|captures| captures.get(1))
        .map(|matched| Version::parse(matched.as_str()).unwrap())
        .unwrap_or_else(|| panic!("missing VersionPrefix for package: {}", path.display()));

    if old_version == *version {
        return false;
    }

    let new_version_string = format!("<VersionPrefix>{version}</VersionPrefix>");
    let new_manifest = matcher.replace(&manifest, new_version_string.as_str());
    std::fs::write(&path, new_manifest.as_bytes()).unwrap();
    println!(
        "updating version for package: {} from {} to {}",
        path.display(),
        old_version,
        version
    );

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_versions_preserve_reviewed_versions_and_update_dependencies() {
        let mut packages = all_packages();
        let old = packages[0].version.clone();
        packages[1].version.major += 1;
        let reviewed = packages[1].version.clone();
        let baseline = include_str!("package.rs");
        prepare_versions(&mut packages, baseline, true, &[]).unwrap();
        let inventory = render_inventory(&packages).unwrap();
        assert_eq!(packages[0].version.patch, old.patch + 1);
        assert_eq!(packages[1].version, reviewed);
        assert_eq!(packages[1].dependencies[0].version, packages[0].version);
        assert!(inventory.contains(&format!("\"core\", \"{}\"", packages[0].version)));
        // Retrying from the same published baseline must not consume another patch.
        prepare_versions(&mut packages, baseline, true, &[]).unwrap();
        assert_eq!(packages[0].version.patch, old.patch + 1);
        let mut new_packages = all_packages();
        prepare_versions(&mut new_packages, "", true, &[]).unwrap();
        assert_eq!(new_packages[0].version, old);
    }

    #[test]
    fn breaking_versions_are_scoped_idempotent_and_preserve_higher_targets() {
        let mut packages = all_packages();
        let baseline = include_str!("package.rs");
        let breaking = vec![
            "core".to_string(),
            "bindings/java".to_string(),
            "core".to_string(),
        ];
        let old = inventory_versions(baseline).unwrap();
        let reviewed = Version::new(packages[1].version.major + 1, 0, 0);
        packages[1].version = reviewed.clone();
        for _ in 0..2 {
            prepare_versions(&mut packages, baseline, true, &breaking).unwrap();
            assert_eq!(
                packages[1].release_dependency_version("opendal-core"),
                Some(packages[0].version())
            );
            for package in &packages {
                let previous = &old[package.name()];
                let expected = if breaking.iter().any(|name| name == package.name()) {
                    if previous.major == 0 {
                        Version::new(0, previous.minor + 1, 0)
                    } else {
                        Version::new(previous.major + 1, 0, 0)
                    }
                } else if package.name() == "integrations/dav-server" {
                    reviewed.clone()
                } else {
                    Version::new(previous.major, previous.minor, previous.patch + 1)
                };
                assert_eq!(package.version, expected, "{}", package.name());
                for dependency in &package.dependencies {
                    assert_eq!(dependency.version, packages[0].version);
                }
            }
        }
        assert!(
            prepare_versions(&mut packages, baseline, true, &["bindings/go".to_string()]).is_err()
        );
    }

    #[test]
    fn stable_breaking_release_increments_major() {
        let mut packages = all_packages();
        let baseline = include_str!("package.rs").replace(
            &format!("\"core\", \"{}\"", packages[0].version),
            "\"core\", \"1.2.3\"",
        );
        packages[0].version = Version::new(1, 2, 3);
        prepare_versions(&mut packages, &baseline, true, &["core".to_string()]).unwrap();
        assert_eq!(packages[0].version, Version::new(2, 0, 0));
    }

    #[test]
    fn sync_versions_preserve_main_and_use_released_dependency_versions() {
        let mut packages = all_packages();
        let released = packages[0].version.clone();
        packages[0].version = Version::new(0, 0, 0);
        packages[1].version.major += 1;
        let main_version = packages[1].version.clone();
        prepare_versions(&mut packages, include_str!("package.rs"), false, &[]).unwrap();
        assert_eq!(packages[0].version, released);
        assert_eq!(packages[1].version, main_version);
        assert_eq!(packages[1].dependencies[0].version, released);
        prepare_versions(&mut packages, include_str!("package.rs"), false, &[]).unwrap();
        assert_eq!(packages[0].version, released);
        let mut new_packages = all_packages();
        prepare_versions(&mut new_packages, "", false, &[]).unwrap();
        assert_eq!(new_packages[0].version, released);
    }

    #[test]
    fn node_loaders_follow_package_version() {
        let root = temp_test_dir("node-loaders");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("package.json"), r#"{"version":"1.2.3"}"#).unwrap();
        std::fs::write(root.join("generated.js"), "expectedVersion = '1.2.3'").unwrap();
        assert!(update_nodejs_version(&root, &Version::new(1, 2, 4)));
        assert_eq!(
            std::fs::read_to_string(root.join("generated.js")).unwrap(),
            "expectedVersion = '1.2.4'"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("odev-package-{name}-{nanos}"))
    }

    fn release_package(name: &str) -> Package {
        all_packages()
            .into_iter()
            .find(|package| package.name() == name)
            .unwrap()
    }

    fn cargo_manifest_version(package: &Package) -> Version {
        let manifest_path = package.path.join("Cargo.toml");
        let manifest = std::fs::read_to_string(manifest_path).unwrap();
        let manifest = DocumentMut::from_str(&manifest).unwrap();

        manifest["package"]["version"]
            .as_str()
            .map(Version::parse)
            .transpose()
            .unwrap()
            .unwrap()
    }

    fn dotnet_project_version(package: &Package) -> Version {
        let manifest_path = package.path.join("OpenDAL").join("OpenDAL.csproj");
        let manifest = std::fs::read_to_string(manifest_path).unwrap();
        let matcher = regex::Regex::new(r"<VersionPrefix>(\d+\.\d+\.\d+)</VersionPrefix>").unwrap();
        matcher
            .captures(&manifest)
            .and_then(|captures| captures.get(1))
            .map(|matched| Version::parse(matched.as_str()).unwrap())
            .unwrap()
    }

    #[test]
    fn parquet_release_version_matches_manifest() {
        let parquet = release_package("integrations/parquet");

        assert_eq!(parquet.version, cargo_manifest_version(&parquet));
    }

    #[test]
    fn integrations_track_public_compatibility_dependencies() {
        let core = release_package("core");
        let dav = release_package("integrations/dav-server");
        let object_store = release_package("integrations/object_store");
        let parquet = release_package("integrations/parquet");

        assert_eq!(dav.public_compat_dependencies(), ["opendal-core"]);
        assert_eq!(
            object_store.public_compat_dependencies(),
            ["opendal", "object_store"]
        );
        assert_eq!(parquet.public_compat_dependencies(), ["opendal", "parquet"]);
        assert_eq!(
            object_store.release_dependency_version("opendal"),
            Some(core.version())
        );
        assert_eq!(
            parquet.release_dependency_version("opendal"),
            Some(core.version())
        );
    }

    #[test]
    fn ruby_release_version_matches_manifest() {
        let ruby = release_package("bindings/ruby");

        assert_eq!(ruby.version, cargo_manifest_version(&ruby));
    }

    #[test]
    fn dotnet_release_version_matches_project() {
        let dotnet = release_package("bindings/dotnet");

        assert_eq!(dotnet.version, dotnet_project_version(&dotnet));
    }

    #[test]
    fn update_manifest_version_supports_workspace_package_version() {
        let mut manifest = DocumentMut::from_str(
            r#"
[workspace.package]
version = "0.55.0"

[package]
name = "opendal"
version = { workspace = true }
"#,
        )
        .unwrap();

        let updated = update_manifest_version(
            &mut manifest,
            &Version::parse("0.56.0").unwrap(),
            Path::new("core/Cargo.toml"),
        );

        assert!(updated);
        assert_eq!(
            manifest["workspace"]["package"]["version"].as_str(),
            Some("0.56.0")
        );
        assert!(!manifest["package"]["version"].is_str());
    }

    #[test]
    fn update_dependency_version_only_touches_release_managed_core_constraints() {
        let dir = temp_test_dir("dependency-sync");
        std::fs::create_dir_all(&dir).unwrap();

        let manifest_path = dir.join("Cargo.toml");
        std::fs::write(
            &manifest_path,
            r#"[package]
name = "demo"
version = "0.8.0"

[dependencies]
opendal = { version = "0.55.0", path = "../../core" }
opendal-core = { version = "0.55.0", path = "../../core/core" }
serde = "1"

[dev-dependencies]
opendal = { version = "0.55.0", path = "../../core", features = ["services-memory"] }

[build-dependencies]
opendal = { version = ">=0", path = "../../core" }

[target.'cfg(unix)'.dependencies]
opendal = { version = "0.55.0", path = "../../core" }

[target.'cfg(windows)'.dependencies]
opendal = { version = "0.55.0", path = "../core" }
"#,
        )
        .unwrap();

        let dependency = Package {
            name: "core".to_string(),
            path: dir.join("../../core"),
            version: Version::parse("0.56.0").unwrap(),
            dependencies: vec![],
            public_compat_dependencies: &[],
        };
        assert!(path_points_into(
            dir.as_path(),
            "../../core",
            &dependency.path
        ));

        let updated = update_cargo_version(
            dir.as_path(),
            &Version::parse("0.8.0").unwrap(),
            &[dependency],
        );
        assert!(updated);

        let manifest = std::fs::read_to_string(&manifest_path).unwrap();
        let manifest = DocumentMut::from_str(&manifest).unwrap();

        assert_eq!(
            manifest["dependencies"]["opendal"]["version"].as_str(),
            Some("0.56.0")
        );
        assert_eq!(
            manifest["dependencies"]["opendal-core"]["version"].as_str(),
            Some("0.56.0")
        );
        assert_eq!(
            manifest["dev-dependencies"]["opendal"]["version"].as_str(),
            Some("0.56.0")
        );
        assert_eq!(
            manifest["target"]["cfg(unix)"]["dependencies"]["opendal"]["version"].as_str(),
            Some("0.56.0")
        );
        assert_eq!(
            manifest["build-dependencies"]["opendal"]["version"].as_str(),
            Some(">=0")
        );
        assert_eq!(
            manifest["target"]["cfg(windows)"]["dependencies"]["opendal"]["version"].as_str(),
            Some("0.55.0")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn update_dotnet_version_updates_version_prefix() {
        let dir = temp_test_dir("dotnet-version");
        let project_dir = dir.join("OpenDAL");
        std::fs::create_dir_all(&project_dir).unwrap();

        let project_path = project_dir.join("OpenDAL.csproj");
        std::fs::write(
            &project_path,
            r#"<Project Sdk="Microsoft.NET.Sdk">
    <PropertyGroup>
        <VersionPrefix>0.1.0</VersionPrefix>
        <AssemblyVersion>$(VersionPrefix)</AssemblyVersion>
        <FileVersion>$(VersionPrefix)</FileVersion>
    </PropertyGroup>
</Project>"#,
        )
        .unwrap();

        let updated = update_dotnet_version(&dir, &Version::parse("0.2.0").unwrap());
        assert!(updated);

        let manifest = std::fs::read_to_string(&project_path).unwrap();
        assert!(manifest.contains("<VersionPrefix>0.2.0</VersionPrefix>"));
        assert!(manifest.contains("<AssemblyVersion>$(VersionPrefix)</AssemblyVersion>"));
        assert!(manifest.contains("<FileVersion>$(VersionPrefix)</FileVersion>"));
        assert!(!update_dotnet_version(
            &dir,
            &Version::parse("0.2.0").unwrap()
        ));

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sync_workspace_internal_dependency_versions_updates_workspace_path_dependencies() {
        let dir = temp_test_dir("workspace-sync");
        let crate_dir = dir.join("crate-a");
        let dep_dir = dir.join("dep-b");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::create_dir_all(&dep_dir).unwrap();

        std::fs::write(
            dir.join("Cargo.toml"),
            r#"[workspace]
members = ["crate-a", "dep-b"]

[workspace.package]
version = "0.56.0"
"#,
        )
        .unwrap();
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            r#"[package]
name = "crate-a"
version = "0.56.0"

[dependencies]
dep-b = { path = "../dep-b", version = "0.55.0" }
serde = "1"
"#,
        )
        .unwrap();
        std::fs::write(
            dep_dir.join("Cargo.toml"),
            r#"[package]
name = "dep-b"
version = "0.56.0"
"#,
        )
        .unwrap();

        let updated = sync_workspace_internal_dependency_versions(
            dir.as_path(),
            &Version::parse("0.56.0").unwrap(),
        );
        assert!(updated);

        let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
        let manifest = DocumentMut::from_str(&manifest).unwrap();
        assert_eq!(
            manifest["dependencies"]["dep-b"]["version"].as_str(),
            Some("0.56.0")
        );
        assert_eq!(manifest["dependencies"]["serde"].as_str(), Some("1"));

        let dep_manifest = std::fs::read_to_string(dep_dir.join("Cargo.toml")).unwrap();
        let dep_manifest = DocumentMut::from_str(&dep_manifest).unwrap();
        assert_eq!(dep_manifest["package"]["version"].as_str(), Some("0.56.0"));

        std::fs::remove_dir_all(dir).unwrap();
    }
}
