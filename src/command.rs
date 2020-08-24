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
use sp_core::{hexdisplay::HexDisplay, crypto::{Pair, Ss58Codec, Ss58AddressFormat}};
use sc_cli::{SubstrateCli, ChainSpec, Role, RuntimeVersion};
use sc_service::{PartialComponents, config::KeystoreConfig};
use sc_keystore::Store as Keystore;
use crate::chain_spec;
use crate::cli::{Cli, Subcommand};
use crate::service;

const DEFAULT_CHECK_INHERENTS_AFTER: u32 = 152650;

/// URL for the telemetry server. Disabled by default.
pub const POLKADOT_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Kulupu".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/kulupu/kulupu/issues".into()
	}

	fn copyright_start_year() -> i32 {
		2019
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		Ok(match id {
			"" | "kulupu" | "mainnet" => Box::new(chain_spec::mainnet_config()),
			"local" => Box::new(chain_spec::local_testnet_config()?),
			"dev" => Box::new(chain_spec::development_config()?),
			"breaknet4" => Box::new(chain_spec::breaknet4_config()),
			path => Box::new(chain_spec::ChainSpec::from_json_file(
				std::path::PathBuf::from(path),
			)?),
		})
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&kulupu_runtime::VERSION
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let mut cli = Cli::from_args();
	if cli.enable_polkadot_telemetry {
		cli.run.telemetry_endpoints.push((POLKADOT_TELEMETRY_URL.to_string(), 0));
	}

	match &cli.subcommand {
		Some(Subcommand::Base(subcommand)) => {
			let runner = cli.create_runner(subcommand)?;
			runner.run_subcommand(subcommand, |config| {
				let PartialComponents { client, backend, task_manager, import_queue, .. } =
					crate::service::new_partial(&config, None, cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER), !cli.no_donate)?;
				Ok((client, backend, import_queue, task_manager))
			})
		},
		Some(Subcommand::ExportBuiltinWasm(cmd)) => {
			let wasm_binary_bloaty = kulupu_runtime::WASM_BINARY_BLOATY.ok_or("Wasm binary not available".to_string())?;
			let wasm_binary = kulupu_runtime::WASM_BINARY.ok_or("Compact Wasm binary not available".to_string())?;

			info!("Exporting builtin wasm binary to folder: {}", cmd.folder);

			let folder = PathBuf::from(cmd.folder.clone());
			{
				let mut path = folder.clone();
				path.push("kulupu_runtime.compact.wasm");
				let mut file = File::create(path)?;
				file.write_all(&wasm_binary)?;
				file.flush()?;
			}

			{
				let mut path = folder.clone();
				path.push("kulupu_runtime.wasm");
				let mut file = File::create(path)?;
				file.write_all(&wasm_binary_bloaty)?;
				file.flush()?;
			}

			Ok(())
		},
		Some(Subcommand::ImportMiningKey(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| {
				let keystore = match &config.keystore {
					KeystoreConfig::Path { path, password } => Keystore::open(
						path.clone(),
						password.clone()
					).map_err(|e| format!("Open keystore failed: {:?}", e))?,
					KeystoreConfig::InMemory => Keystore::new_in_memory(),
				};

				let pair = keystore.write().insert::<kulupu_pow::app::Pair>(&cmd.suri)
					.map_err(|e| format!("Registering mining key failed: {:?}", e))?;
				info!("Registered one mining key.\n\nPublic key: 0x{}",
					  HexDisplay::from(&pair.public().as_ref()));

				Ok(())
			})
		},
		Some(Subcommand::GenerateMiningKey(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| {
				let keystore = match &config.keystore {
					KeystoreConfig::Path { path, password } => Keystore::open(
						path.clone(),
						password.clone()
					).map_err(|e| format!("Open keystore failed: {:?}", e))?,
					KeystoreConfig::InMemory => Keystore::new_in_memory(),
				};

				let (_, phrase, _) = kulupu_pow::app::Pair::generate_with_phrase(None);

				let pair = keystore.write().insert::<kulupu_pow::app::Pair>(&phrase)
					.map_err(|e| format!("Generating mining key failed: {:?}", e))?;

				info!(
					"Generated one mining key.\n\nPublic key: 0x{}\nSecret seed: {}\nAddress: {}",
					HexDisplay::from(&pair.public().as_ref()),
					phrase,
					pair.public().to_ss58check_with_version(Ss58AddressFormat::KulupuAccount),
				);

				Ok(())
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			if cfg!(feature = "runtime-benchmarks") {
				let runner = cli.create_runner(cmd)?;

				runner.sync_run(|config| cmd.run::<kulupu_runtime::Block, service::Executor>(config))
			} else {
				Err("Benchmarking wasn't enabled when building the node. \
				You can enable it with `--features runtime-benchmarks`.".into())
			}
		},
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(
				|config| match config.role {
					Role::Light => service::new_light(
						config,
						cli.author.as_ref().map(|s| s.as_str()),
						cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER),
						!cli.no_donate,
					),
					_ => service::new_full(
						config,
						cli.author.as_ref().map(|s| s.as_str()),
						cli.threads.unwrap_or(1),
						cli.round.unwrap_or(5000),
						cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER),
						!cli.no_donate,
					)
				}
			)
		},
	}
}
