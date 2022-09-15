// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use anyhow::Result;
use clap::builder::PathBufValueParser;
use clap::App;
use clap::AppSettings;
use clap::Arg;

pub fn main() -> Result<()> {
    let _ = cli("ocp").get_matches();
    println!("got ocp");
    Ok(())
}

pub(crate) fn cli(name: &str) -> App<'static> {
    let app = App::new(name)
        .version("0.10.0")
        .about("copy")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::new("source_file")
                .required(true)
                .value_parser(PathBufValueParser::new()),
        )
        .arg(
            Arg::new("target_file")
                .required(true)
                .value_parser(PathBufValueParser::new()),
        );

    app
}
