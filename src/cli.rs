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

use sc_cli::RunCmd;
use structopt::StructOpt;

#[derive(Debug, StructOpt, Clone)]
pub enum Subcommand {
	#[structopt(flatten)]
	Base(sc_cli::Subcommand),

	#[structopt(name = "export-builtin-wasm", setting = structopt::clap::AppSettings::Hidden)]
	ExportBuiltinWasm(ExportBuiltinWasmCommand),
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
}

#[derive(Debug, StructOpt, Clone)]
pub struct ExportBuiltinWasmCommand {
	#[structopt()]
	pub folder: String,
}
