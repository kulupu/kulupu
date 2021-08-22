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

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::path::PathBuf;
use std::sync::Arc;
use std::str::FromStr;
use std::time::Duration;
use std::thread;
use parking_lot::Mutex;
use codec::Encode;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use sp_core::{H256, Pair, crypto::{UncheckedFrom, Ss58Codec, Ss58AddressFormat}};
use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use sc_service::{error::{Error as ServiceError}, Configuration, TaskManager};
use sc_client_api::backend::RemoteBackend;
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_executor::NativeElseWasmExecutor;
use sc_consensus::DefaultImportQueue;
use kulupu_runtime::{self, opaque::Block, RuntimeApi};
use kulupu_pow::Error as PowError;
use kulupu_pow::compute::Error as ComputeError;
use kulupu_pow::compute::RandomxError;
use async_trait::async_trait;
use log::*;

// Our native executor instance.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		kulupu_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		kulupu_runtime::native_version()
	}
}

pub fn decode_author(
	author: Option<&str>, keystore: SyncCryptoStorePtr, keystore_path: Option<PathBuf>,
) -> Result<kulupu_pow::app::Public, String> {
	if let Some(author) = author {
		if author.starts_with("0x") {
			Ok(kulupu_pow::app::Public::unchecked_from(
				H256::from_str(&author[2..]).map_err(|_| "Invalid author account".to_string())?
			).into())
		} else {
			let (address, version) = kulupu_pow::app::Public::from_ss58check_with_version(author)
				.map_err(|_| "Invalid author address".to_string())?;
			if version != Ss58AddressFormat::KulupuAccount {
				return Err("Invalid author version".to_string())
			}
			Ok(address)
		}
	} else {
		info!("The node is configured for mining, but no author key is provided.");

		let (pair, phrase, _) = kulupu_pow::app::Pair::generate_with_phrase(None);

		SyncCryptoStore::insert_unknown(
			&*keystore.as_ref(),
			kulupu_pow::app::ID,
			&phrase,
			pair.public().as_ref(),
		).map_err(|e| format!("Registering mining key failed: {:?}", e))?;

		info!("Generated a mining key with address: {}", pair.public().to_ss58check_with_version(Ss58AddressFormat::KulupuAccount));

		match keystore_path {
			Some(path) => info!("You can go to {:?} to find the seed phrase of the mining key.", path),
			None => warn!("Keystore is not local. This means that your mining key will be lost when exiting the program. This should only happen if you are in dev mode."),
		}

		Ok(pair.public())
	}
}

type FullClient =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

pub struct CreateInherentDataProviders;

#[async_trait]
impl sp_inherents::CreateInherentDataProviders<Block, ()> for CreateInherentDataProviders {
	type InherentDataProviders = sp_timestamp::InherentDataProvider;

	async fn create_inherent_data_providers(
		&self,
		_parent: <Block as BlockT>::Hash,
		_extra_args: (),
	) -> Result<Self::InherentDataProviders, Box<dyn std::error::Error + Send + Sync>> {
		Ok(sp_timestamp::InherentDataProvider::from_system_time())
	}
}

type PowBlockImport = sc_consensus_pow::PowBlockImport<
	Block,
	kulupu_pow::weak_sub::WeakSubjectiveBlockImport<
		Block,
		Arc<FullClient>,
		FullClient,
		FullSelectChain,
		kulupu_pow::RandomXAlgorithm<FullClient>,
		kulupu_pow::weak_sub::ExponentialWeakSubjectiveAlgorithm
	>,
	FullClient,
	FullSelectChain,
	kulupu_pow::RandomXAlgorithm<FullClient>,
	sp_consensus::AlwaysCanAuthor,
	CreateInherentDataProviders,
>;

pub fn new_partial(
	config: &Configuration,
	check_inherents_after: u32,
	donate: bool,
	enable_weak_subjectivity: bool,
) -> Result<sc_service::PartialComponents<
	FullClient, FullBackend, FullSelectChain,
	DefaultImportQueue<Block, FullClient>,
	sc_transaction_pool::FullPool<Block, FullClient>,
	(
		PowBlockImport,
		Option<Telemetry>,
	),
>, ServiceError> {
	let telemetry = config.telemetry_endpoints.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts(
			&config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry
		.map(|(worker, telemetry)| {
			task_manager.spawn_handle().spawn("telemetry", worker.run());
			telemetry
		});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let algorithm = kulupu_pow::RandomXAlgorithm::new(client.clone());

	let weak_sub_block_import = kulupu_pow::weak_sub::WeakSubjectiveBlockImport::new(
		client.clone(),
		client.clone(),
		algorithm.clone(),
		kulupu_pow::weak_sub::ExponentialWeakSubjectiveAlgorithm(30, 1.1),
		select_chain.clone(),
		enable_weak_subjectivity,
	);

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		weak_sub_block_import,
		client.clone(),
		algorithm.clone(),
		check_inherents_after,
		select_chain.clone(),
		CreateInherentDataProviders,
		sp_consensus::AlwaysCanAuthor,
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		algorithm.clone(),
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
	)?;

	Ok(sc_service::PartialComponents {
		client, backend, task_manager, import_queue, keystore_container,
		select_chain, transaction_pool,
		other: (pow_block_import, telemetry),
	})
}

/// Builds a new service for a full client.
pub fn new_full(
	config: Configuration,
	author: Option<&str>,
	threads: usize,
	round: u32,
	check_inherents_after: u32,
	donate: bool,
	enable_weak_subjectivity: bool,
) -> Result<TaskManager, ServiceError> {
	let sc_service::PartialComponents {
		client, backend, mut task_manager, import_queue, keystore_container,
		select_chain, transaction_pool,
		other: (pow_block_import, mut telemetry),
	} = new_partial(&config, check_inherents_after, donate, enable_weak_subjectivity)?;

	let (network, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: None,
			block_announce_validator_builder: None,
			warp_sync: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config, task_manager.spawn_handle(), client.clone(), network.clone(),
		);
	}

	let role = config.role.clone();
	let prometheus_registry = config.prometheus_registry().cloned();

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
			};

			Ok(crate::rpc::create_full(deps))
		})
	};

	let keystore_path = config.keystore.path().map(|p| p.to_owned());

	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		rpc_extensions_builder: rpc_extensions_builder,
		on_demand: None,
		remote_blockchain: None,
		backend, system_rpc_tx, config,
		telemetry: telemetry.as_mut(),
	})?;

	if role.is_authority() {
		let author = decode_author(author, keystore_container.sync_keystore(), keystore_path)?;
		let algorithm = kulupu_pow::RandomXAlgorithm::new(
			client.clone(),
		);

		let proposer = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let (worker, worker_task) = sc_consensus_pow::start_mining_worker(
			Box::new(pow_block_import.clone()),
			client.clone(),
			select_chain.clone(),
			algorithm,
			proposer,
			network.clone(),
			network.clone(),
			Some(author.encode()),
			CreateInherentDataProviders,
			Duration::new(10, 0),
			Duration::new(10, 0),
			sp_consensus::AlwaysCanAuthor,
		);
		task_manager.spawn_handle().spawn_blocking("pow", worker_task);

		let stats = Arc::new(Mutex::new(kulupu_pow::Stats::new()));

		for _ in 0..threads {
			if let Some(keystore) = keystore_container.local_keystore() {
				let worker = worker.clone();
				let client = client.clone();
				let stats = stats.clone();

				thread::spawn(move || {
					loop {
						let metadata = worker.lock().metadata();
						if let Some(metadata) = metadata {
							match kulupu_pow::mine(
								client.as_ref(),
								&keystore,
								&BlockId::Hash(metadata.best_hash),
								&metadata.pre_hash,
								metadata.pre_runtime.as_ref().map(|v| &v[..]),
								metadata.difficulty,
								round,
								&stats
							) {
								Ok(Some(seal)) => {
									let mut worker = worker.lock();
									let current_metadata = worker.metadata();
									if current_metadata == Some(metadata) {
										let _ = futures::executor::block_on(worker.submit(seal));
									}
								},
								Ok(None) => (),
								Err(PowError::Compute(ComputeError::CacheNotAvailable)) => {
									thread::sleep(Duration::new(1, 0));
								},
								Err(PowError::Compute(ComputeError::Randomx(err @ RandomxError::CacheAllocationFailed))) => {
									warn!("Mining failed: {}", err.description());
									thread::sleep(Duration::new(10, 0));
								},
								Err(err) => {
									warn!("Mining failed: {:?}", err);
								},
							}
						} else {
							thread::sleep(Duration::new(1, 0));
						}
					}
				});
			} else {
				warn!("Local keystore is not available");
			}
		}
	}

	network_starter.start_network();
	Ok(task_manager)
}

/// Builds a new service for a light client.
pub fn new_light(
	config: Configuration,
	check_inherents_after: u32,
	donate: bool,
	enable_weak_subjectivity: bool,
) -> Result<TaskManager, ServiceError> {
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
	);

	let (client, backend, keystore_container, mut task_manager, on_demand) =
		sc_service::new_light_parts::<Block, RuntimeApi, _>(
			&config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;

	let mut telemetry = telemetry
		.map(|(worker, telemetry)| {
			task_manager.spawn_handle().spawn("telemetry", worker.run());
			telemetry
		});


	let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
		on_demand.clone(),
	));

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let algorithm = kulupu_pow::RandomXAlgorithm::new(client.clone());

	let weak_sub_block_import = kulupu_pow::weak_sub::WeakSubjectiveBlockImport::new(
		client.clone(),
		client.clone(),
		algorithm.clone(),
		kulupu_pow::weak_sub::ExponentialWeakSubjectiveAlgorithm(30, 1.1),
		select_chain.clone(),
		enable_weak_subjectivity,
	);

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		weak_sub_block_import,
		client.clone(),
		algorithm.clone(),
		check_inherents_after,
		select_chain.clone(),
		CreateInherentDataProviders,
		sp_consensus::AlwaysCanAuthor,
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		algorithm.clone(),
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
	)?;

	let (network, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: Some(on_demand.clone()),
			block_announce_validator_builder: None,
			warp_sync: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config, task_manager.spawn_handle(), client.clone(), network.clone(),
		);
	}

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		remote_blockchain: Some(backend.remote_blockchain()),
		transaction_pool,
		task_manager: &mut task_manager,
		on_demand: Some(on_demand),
		rpc_extensions_builder: Box::new(|_, _| Ok(())),
		config,
		client,
		keystore: keystore_container.sync_keystore(),
		backend,
		network,
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	 })?;

	 network_starter.start_network();

	 Ok(task_manager)
}
