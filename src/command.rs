// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2019-2020 Wei Tang.
//
// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Kulupu. If not, see <http://www.gnu.org/licenses/>.

use std::{path::PathBuf, fs::File, io::Write};
use log::info;
use sp_core::{hexdisplay::HexDisplay, crypto::{Pair, Ss58Codec, Ss58AddressFormat}};
use sp_keystore::SyncCryptoStore;
use sc_cli::{SubstrateCli, ChainSpec, Role, RuntimeVersion};
use sc_service::{PartialComponents, config::KeystoreConfig};
use sc_keystore::LocalKeystore;
use crate::chain_spec;
use crate::cli::{Cli, Subcommand, RandomxFlag};
use crate::service;

const DEFAULT_CHECK_INHERENTS_AFTER: u32 = 152650;
const DEFAULT_ROUND: u32 = 1000;

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

	let mut randomx_config = kulupu_pow::compute::Config::new();
	if cli.randomx_flags.contains(&RandomxFlag::LargePages) {
		randomx_config.large_pages = true;
	}
	if cli.randomx_flags.contains(&RandomxFlag::Secure) {
		randomx_config.secure = true;
	}
	let _ = kulupu_pow::compute::set_global_config(randomx_config);

	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					crate::service::new_partial(&config, None, cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER), !cli.no_donate, !cli.disable_weak_subjectivity)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					crate::service::new_partial(&config, None, cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER), !cli.no_donate, !cli.disable_weak_subjectivity)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					crate::service::new_partial(&config, None, cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER), !cli.no_donate, !cli.disable_weak_subjectivity)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					crate::service::new_partial(&config, None, cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER), !cli.no_donate, !cli.disable_weak_subjectivity)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.database))
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, backend, task_manager, .. } =
					crate::service::new_partial(&config, None, cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER), !cli.no_donate, !cli.disable_weak_subjectivity)?;
				Ok((cmd.run(client, backend), task_manager))
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
					KeystoreConfig::Path { path, password } => LocalKeystore::open(
						path.clone(),
						password.clone()
					).map_err(|e| format!("Open keystore failed: {:?}", e))?,
					KeystoreConfig::InMemory => LocalKeystore::in_memory(),
				};

				let pair = kulupu_pow::app::Pair::from_string(
					&cmd.suri,
					None,
				).map_err(|e| format!("Invalid seed: {:?}", e))?;

				SyncCryptoStore::insert_unknown(
					&keystore,
					kulupu_pow::app::ID,
					&cmd.suri,
					pair.public().as_ref(),
				).map_err(|e| format!("Registering mining key failed: {:?}", e))?;

				info!("Registered one mining key (public key 0x{}).",
					  HexDisplay::from(&pair.public().as_ref()));

				Ok(())
			})
		},
		Some(Subcommand::GenerateMiningKey(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| {
				let keystore = match &config.keystore {
					KeystoreConfig::Path { path, password } => LocalKeystore::open(
						path.clone(),
						password.clone()
					).map_err(|e| format!("Open keystore failed: {:?}", e))?,
					KeystoreConfig::InMemory => LocalKeystore::in_memory(),
				};

				let (pair, phrase, _) = kulupu_pow::app::Pair::generate_with_phrase(None);

				SyncCryptoStore::insert_unknown(
					&keystore,
					kulupu_pow::app::ID,
					&phrase,
					pair.public().as_ref(),
				).map_err(|e| format!("Registering mining key failed: {:?}", e))?;

				info!("Generated one mining key.");

				println!(
					"Public key: 0x{}\nSecret seed: {}\nAddress: {}",
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
				|config| async move {
					match config.role {
						Role::Light => service::new_light(
							config,
							cli.author.as_ref().map(|s| s.as_str()),
							cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER),
							!cli.no_donate,
							!cli.disable_weak_subjectivity,
						),
						_ => service::new_full(
							config,
							cli.author.as_ref().map(|s| s.as_str()),
							cli.threads.unwrap_or(1),
							cli.round.unwrap_or(DEFAULT_ROUND),
							cli.check_inherents_after.unwrap_or(DEFAULT_CHECK_INHERENTS_AFTER),
							!cli.no_donate,
							!cli.disable_weak_subjectivity,
						)
					}
				}
			).map_err(sc_cli::Error::Service)
		},
	}
}
