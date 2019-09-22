//! Kulupu CLI library.

#![warn(missing_docs)]
#![warn(unused_extern_crates)]

mod chain_spec;
#[macro_use]
mod service;
mod cli;

pub use substrate_cli::{VersionInfo, IntoExit, error};

fn main() {
	let version = VersionInfo {
		name: "Kulupu",
		commit: env!("VERGEN_SHA_SHORT"),
		version: env!("CARGO_PKG_VERSION"),
		executable_name: "kulupu",
		author: "Wei Tang <hi@that.world>",
		description: "Kulupu node implementation",
		support_url: "https://github.com/kulupu/kulupu/issues",
	};

	if let Err(e) = cli::run(::std::env::args(), cli::Exit, version) {
		eprintln!("Fatal error: {}\n\n{:?}", e, e);
		std::process::exit(1)
	}
}
