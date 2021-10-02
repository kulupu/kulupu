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

use sc_cli::RunCmd;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum Subcommand {
	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Export blocks.
	ExportBlocks(sc_cli::ExportBlocksCmd),

	/// Export the state of a given block into a chain spec.
	ExportState(sc_cli::ExportStateCmd),

	/// Import blocks.
	ImportBlocks(sc_cli::ImportBlocksCmd),

	/// Remove the whole chain.
	PurgeChain(sc_cli::PurgeChainCmd),

	/// Revert the chain to a previous state.
	Revert(sc_cli::RevertCmd),

	#[structopt(name = "export-builtin-wasm", setting = structopt::clap::AppSettings::Hidden)]
	ExportBuiltinWasm(ExportBuiltinWasmCommand),

	#[structopt(name = "import-mining-key")]
	ImportMiningKey(ImportMiningKeyCommand),

	#[structopt(name = "generate-mining-key")]
	GenerateMiningKey(GenerateMiningKeyCommand),

	/// The custom benchmark subcommmand benchmarking runtime pallets.
	#[structopt(name = "benchmark", about = "Benchmark runtime pallets.")]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),
}

#[derive(Debug, Eq, PartialEq)]
pub enum RandomxFlag {
	LargePages,
	Secure,
}

impl FromStr for RandomxFlag {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"large-pages" => Ok(Self::LargePages),
			"secure" => Ok(Self::Secure),
			_ => Err("Unknown flag".to_string()),
		}
	}
}

#[derive(Debug, StructOpt)]
pub struct Cli {
	#[structopt(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[structopt(flatten)]
	pub run: RunCmd,

	#[structopt(long)]
	pub author: Option<String>,
	#[structopt(long)]
	pub threads: Option<usize>,
	#[structopt(long)]
	pub round: Option<u32>,
	#[structopt(long)]
	pub enable_polkadot_telemetry: bool,
	#[structopt(long)]
	pub disable_weak_subjectivity: bool,
	#[structopt(long)]
	pub check_inherents_after: Option<u32>,
	#[structopt(long)]
	pub randomx_flags: Vec<RandomxFlag>,
}

#[derive(Debug, StructOpt)]
pub struct ExportBuiltinWasmCommand {
	#[structopt()]
	pub folder: String,
}

#[derive(Debug, StructOpt)]
pub struct ImportMiningKeyCommand {
	#[structopt()]
	pub suri: String,

	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub shared_params: sc_cli::SharedParams,

	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub keystore_params: sc_cli::KeystoreParams,
}

impl sc_cli::CliConfiguration for ImportMiningKeyCommand {
	fn shared_params(&self) -> &sc_cli::SharedParams {
		&self.shared_params
	}
	fn keystore_params(&self) -> Option<&sc_cli::KeystoreParams> {
		Some(&self.keystore_params)
	}
}

#[derive(Debug, StructOpt)]
pub struct GenerateMiningKeyCommand {
	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub shared_params: sc_cli::SharedParams,

	#[allow(missing_docs)]
	#[structopt(flatten)]
	pub keystore_params: sc_cli::KeystoreParams,
}

impl sc_cli::CliConfiguration for GenerateMiningKeyCommand {
	fn shared_params(&self) -> &sc_cli::SharedParams {
		&self.shared_params
	}
	fn keystore_params(&self) -> Option<&sc_cli::KeystoreParams> {
		Some(&self.keystore_params)
	}
}
