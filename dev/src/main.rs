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

mod generate;
mod release;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap()
}

fn workspace_dir() -> PathBuf {
    manifest_dir().join("..").canonicalize().unwrap()
}

fn find_command(cmd: &str, cwd: impl AsRef<Path>) -> StdCommand {
    match which::which(cmd) {
        Ok(exe) => {
            let mut cmd = StdCommand::new(exe);
            cmd.current_dir(cwd);
            cmd
        }
        Err(err) => {
            panic!("{cmd} not found: {err}");
        }
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cmd {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate all services configs for opendal.
    Generate {
        #[arg(short, long)]
        language: String,
    },
    /// Update the version of all packages.
    UpdateVersion {
        /// Compare versions against the last successful release.
        #[arg(long)]
        baseline: Option<String>,
        /// Prepare at least a patch increment from the baseline for every package.
        #[arg(long, conflicts_with = "sync")]
        patch: bool,
        /// Raise package versions to the baseline without adding another patch.
        #[arg(long)]
        sync: bool,
        /// Packages with declared breaking changes; repeat for each package.
        #[arg(long, requires = "patch")]
        breaking: Vec<String>,
        /// Write the version decisions as JSON for the candidate release plan.
        #[arg(long, requires = "patch")]
        report: Option<PathBuf>,
    },
    /// Create all the release artifacts.
    Release {
        /// Build source archives without invoking GPG.
        #[arg(long)]
        unsigned: bool,
    },
    /// Print the source package inventory as JSON.
    ReleasePackages,
}

fn main() -> anyhow::Result<()> {
    logforth::starter_log::stderr().apply();

    match Cmd::parse().command {
        Commands::Generate { language } => generate::run(&language),
        Commands::UpdateVersion {
            baseline,
            patch,
            sync,
            breaking,
            report,
        } => release::update_version(
            baseline.as_deref(),
            patch,
            sync,
            &breaking,
            report.as_deref(),
        ),
        Commands::Release { unsigned } => release::archive_package(!unsigned),
        Commands::ReleasePackages => release::print_packages(),
    }
}
