use crate::service;
use futures::{future::{select, Map}, FutureExt, TryFutureExt, channel::oneshot, compat::Future01CompatExt};
use std::cell::RefCell;
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use tokio::runtime::Runtime;
pub use substrate_cli::{VersionInfo, IntoExit, error};
use substrate_cli::{display_role, informant, parse_and_prepare, impl_augment_clap, ParseAndPrepare, NoCustom};
use substrate_service::{AbstractService, Roles as ServiceRoles, Configuration};
use crate::chain_spec;
use log::info;
use structopt::StructOpt;

#[derive(Debug, StructOpt, Clone)]
pub struct CustomArgs {
	#[structopt(long)]
	author: Option<String>,
	#[structopt(long)]
	threads: Option<usize>,
	#[structopt(long)]
	round: Option<u32>,
}

impl_augment_clap!(CustomArgs);

#[derive(Debug, StructOpt, Clone)]
pub enum CustomCommands {
	#[structopt(name = "export-builtin-wasm", setting = structopt::clap::AppSettings::Hidden)]
	ExportBuiltinWasm(ExportBuiltinWasmCommand),
}

impl substrate_cli::GetLogFilter for CustomCommands {
	fn get_log_filter(&self) -> Option<String> { None }
}

#[derive(Debug, StructOpt, Clone)]
pub struct ExportBuiltinWasmCommand {
	#[structopt()]
	folder: String,
}

/// Parse command line arguments into service configuration.
pub fn run<I, T, E>(args: I, exit: E, version: VersionInfo) -> error::Result<()> where
	I: IntoIterator<Item = T>,
	T: Into<std::ffi::OsString> + Clone,
	E: IntoExit,
{
	type Config<T> = Configuration<(), T>;
	match parse_and_prepare::<CustomCommands, CustomArgs, _>(&version, "kulupu-substrate", args) {
		ParseAndPrepare::Run(cmd) => cmd.run(load_spec, exit,
		|exit, _cli_args, custom_args, config: Config<_>| {
			info!("{}", version.name);
			info!("  version {}", config.full_version());
			info!("  by {}, 2019", version.author);
			info!("Chain specification: {}", config.chain_spec.name());
			info!("Node name: {}", config.name);
			info!("Roles: {}", display_role(&config));

			let runtime = Runtime::new().map_err(|e| format!("{:?}", e))?;
			match config.roles {
				ServiceRoles::LIGHT => run_until_exit(
					runtime,
				 	service::new_light(
						config,
						custom_args.author.as_ref().map(|s| s.as_str())
					)?,
					exit
				),
				_ => run_until_exit(
					runtime,
					service::new_full(
						config,
						custom_args.author.as_ref().map(|s| s.as_str()),
						custom_args.threads.unwrap_or(1),
						custom_args.round.unwrap_or(5000),
					)?,
					exit
				),
			}
		}),
		ParseAndPrepare::BuildSpec(cmd) => cmd.run::<NoCustom, _, _, _>(load_spec),
		ParseAndPrepare::ExportBlocks(cmd) => cmd.run_with_builder(|config: Config<_>|
			Ok(new_full_start!(config, None).0), load_spec, exit),
		ParseAndPrepare::ImportBlocks(cmd) => cmd.run_with_builder(|config: Config<_>|
			Ok(new_full_start!(config, None).0), load_spec, exit),
		ParseAndPrepare::PurgeChain(cmd) => cmd.run(load_spec),
		ParseAndPrepare::RevertChain(cmd) => cmd.run_with_builder(|config: Config<_>|
			Ok(new_full_start!(config, None).0), load_spec),
		ParseAndPrepare::CustomCommand(CustomCommands::ExportBuiltinWasm(cmd)) => {
			info!("Exporting builtin wasm binary to folder: {}", cmd.folder);
			let folder = PathBuf::from(cmd.folder);

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
	}?;

	Ok(())
}

fn load_spec(id: &str) -> Result<Option<chain_spec::ChainSpec>, String> {
	Ok(match chain_spec::Alternative::from(id) {
		Some(spec) => Some(spec.load()?),
		None => None,
	})
}

fn run_until_exit<T, E>(
	mut runtime: Runtime,
	service: T,
	e: E,
) -> error::Result<()>
where
	T: AbstractService,
	E: IntoExit,
{
	let (exit_send, exit) = oneshot::channel();

	let informant = informant::build(&service);

	let future = select(exit, informant)
		.map(|_| Ok(()))
		.compat();

	runtime.executor().spawn(future);

	// we eagerly drop the service so that the internal exit future is fired,
	// but we need to keep holding a reference to the global telemetry guard
	let _telemetry = service.telemetry();

	let service_res = {
		let exit = e.into_exit();
		let service = service
			.map_err(|err| error::Error::Service(err))
			.compat();
		let select = select(service, exit)
			.map(|_| Ok(()))
			.compat();
		runtime.block_on(select)
	};

	let _ = exit_send.send(());

	// TODO [andre]: timeout this future #1318

	use futures01::Future;

	let _ = runtime.shutdown_on_idle().wait();

	service_res
}

// handles ctrl-c
pub struct Exit;
impl IntoExit for Exit {
	type Exit = Map<oneshot::Receiver<()>, fn(Result<(), oneshot::Canceled>) -> ()>;
	fn into_exit(self) -> Self::Exit {
		// can't use signal directly here because CtrlC takes only `Fn`.
		let (exit_send, exit) = oneshot::channel();

		let exit_send_cell = RefCell::new(Some(exit_send));
		ctrlc::set_handler(move || {
			let exit_send = exit_send_cell.try_borrow_mut().expect("signal handler not reentrant; qed").take();
			if let Some(exit_send) = exit_send {
				exit_send.send(()).expect("Error sending exit notification");
			}
		}).expect("Error setting Ctrl-C handler");

		exit.map(drop)
	}
}
