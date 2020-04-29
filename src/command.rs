// Copyright 2019-2020 Wei Tang.
// This file is part of Kulupu.

// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Kulupu.  If not, see <http://www.gnu.org/licenses/>.

use std::{path::PathBuf, fs::File, io::Write};
use log::info;
use sc_cli::SubstrateCli;
use crate::chain_spec;
use crate::cli::{Cli, Subcommand};
use crate::service;

impl SubstrateCli for Cli {
	fn impl_name() -> &'static str {
		"Kulupu Node"
	}

	fn impl_version() -> &'static str {
		env!("SUBSTRATE_CLI_IMPL_VERSION")
	}

	fn description() -> &'static str {
		env!("CARGO_PKG_DESCRIPTION")
	}

	fn author() -> &'static str {
		env!("CARGO_PKG_AUTHORS")
	}

	fn support_url() -> &'static str {
		"https://github.com/kulupu/kulupu/issues"
	}

	fn copyright_start_year() -> i32 {
		2019
	}

	fn executable_name() -> &'static str {
		env!("CARGO_PKG_NAME")
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"" | "kulupu" | "mainnet" => Box::new(chain_spec::mainnet_config()),
			"local" => Box::new(chain_spec::local_testnet_config()),
			"dev" => Box::new(chain_spec::development_config()),
			path => Box::new(chain_spec::ChainSpec::from_json_file(
				std::path::PathBuf::from(path),
			)?),
		})
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::Base(subcommand)) => {
			let runner = cli.create_runner(subcommand)?;
			runner.run_subcommand(subcommand, |config| Ok(new_full_start!(config, None).0))
		},
		Some(Subcommand::ExportBuiltinWasm(cmd)) => {
			info!("Exporting builtin wasm binary to folder: {}", cmd.folder);
			let folder = PathBuf::from(cmd.folder.clone());

			{
				let mut path = folder.clone();
				path.push("kulupu_runtime.compact.wasm");
				let mut file = File::create(path)?;
				file.write_all(&kulupu_runtime::WASM_BINARY)?;
				file.flush()?;
			}

			{
				let mut path = folder.clone();
				path.push("kulupu_runtime.wasm");
				let mut file = File::create(path)?;
				file.write_all(&kulupu_runtime::WASM_BINARY_BLOATY)?;
				file.flush()?;
			}

			Ok(())
		},
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node(
				|config| service::new_light(
					config,
					cli.author.as_ref().map(|s| s.as_str())
				),
				|config| service::new_full(
					config,
					cli.author.as_ref().map(|s| s.as_str()),
					cli.threads.unwrap_or(1),
					cli.round.unwrap_or(5000),
				),
				kulupu_runtime::VERSION
			)
		},
	}
}
