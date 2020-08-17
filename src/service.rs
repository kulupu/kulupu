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

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::sync::Arc;
use std::str::FromStr;
use codec::Encode;
use sp_runtime::{Perbill, traits::Bounded};
use sp_core::{H256, crypto::{UncheckedFrom, Ss58Codec, Ss58AddressFormat}};
use sc_service::{error::{Error as ServiceError}, Configuration, TaskManager};
use sc_executor::native_executor_instance;
use sc_client_api::backend::RemoteBackend;
use kulupu_runtime::{self, opaque::Block, RuntimeApi, AccountId};
use log::*;

pub use sc_executor::NativeExecutor;

// Our native executor instance.
native_executor_instance!(
	pub Executor,
	kulupu_runtime::api::dispatch,
	kulupu_runtime::native_version,
);

pub fn decode_author(
	author: Option<&str>,
) -> Option<kulupu_pow::app::Public> {
	author.map(|author| {
		if author.starts_with("0x") {
			kulupu_pow::app::Public::unchecked_from(
				H256::from_str(&author[2..]).expect("Invalid author account")
			).into()
		} else {
			let (address, version) = kulupu_pow::app::Public::from_ss58check_with_version(author)
				.expect("Invalid author address");
			assert!(version == Ss58AddressFormat::KulupuAccount, "Invalid author version");
			address
		}
	})
}

type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Inherent data provider for Kulupu.
pub fn kulupu_inherent_data_providers(
	author: Option<kulupu_pow::app::Public>, donate: bool,
) -> Result<sp_inherents::InherentDataProviders, ServiceError> {
	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	if !inherent_data_providers.has_provider(&sp_timestamp::INHERENT_IDENTIFIER) {
		inherent_data_providers
			.register_provider(sp_timestamp::InherentDataProvider)
			.map_err(Into::into)
			.map_err(sp_consensus::Error::InherentData)?;
	}

	if let Some(author) = author {
		let encoded_author = author.encode();

		if !inherent_data_providers.has_provider(&pallet_rewards::INHERENT_IDENTIFIER_V0) {
			inherent_data_providers
				.register_provider(pallet_rewards::InherentDataProviderV0(
					encoded_author.clone(),
				))
				.map_err(Into::into)
				.map_err(sp_consensus::Error::InherentData)?;
		}

		if !inherent_data_providers.has_provider(&pallet_rewards::INHERENT_IDENTIFIER) {
			inherent_data_providers
				.register_provider(pallet_rewards::InherentDataProvider(
					(encoded_author, if donate { Perbill::max_value() } else { Perbill::zero() })
				))
				.map_err(Into::into)
				.map_err(sp_consensus::Error::InherentData)?;
		}
	}

	Ok(inherent_data_providers)
}

pub fn new_partial(
	config: &Configuration,
	author: Option<&str>,
	check_inherents_after: u32,
	donate: bool,
) -> Result<sc_service::PartialComponents<
	FullClient, FullBackend, FullSelectChain,
	sp_consensus::DefaultImportQueue<Block, FullClient>,
	sc_transaction_pool::FullPool<Block, FullClient>,
	sc_consensus_pow::PowBlockImport<Block, Arc<FullClient>, FullClient, FullSelectChain, kulupu_pow::RandomXAlgorithm<FullClient>>,
>, ServiceError> {
	let inherent_data_providers = crate::service::kulupu_inherent_data_providers(author, donate)?;

	let (client, backend, keystore, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
	let client = Arc::new(client);

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let algorithm = kulupu_pow::RandomXAlgorithm::new(client.clone(), None);

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		client.clone(),
		client.clone(),
		algorithm.clone(),
		check_inherents_after,
		Some(select_chain.clone()),
		inherent_data_providers.clone(),
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		None,
		algorithm.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_handle(),
		config.prometheus_registry(),
	)?;

	Ok(sc_service::PartialComponents {
		client, backend, task_manager, import_queue, keystore, select_chain, transaction_pool,
		inherent_data_providers,
		other: pow_block_import,
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
	register_key: Option<&str>,
) -> Result<TaskManager, ServiceError> {
	let sc_service::PartialComponents {
		client, backend, mut task_manager, import_queue, keystore, select_chain, transaction_pool,
		inherent_data_providers, other: pow_block_import,
	} = new_partial(&config, author, check_inherents_after, donate)?;

	let (network, network_status_sinks, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: None,
			block_announce_validator_builder: None,
			finality_proof_request_builder: None,
			finality_proof_provider: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config, backend.clone(), task_manager.spawn_handle(), client.clone(), network.clone(),
		);
	}

	let role = config.role.clone();
	let telemetry_connection_sinks = sc_service::TelemetryConnectionSinks::default();

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
			};

			crate::rpc::create_full(deps)
		})
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: keystore.clone(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		telemetry_connection_sinks: telemetry_connection_sinks.clone(),
		rpc_extensions_builder: rpc_extensions_builder,
		on_demand: None,
		remote_blockchain: None,
		backend, network_status_sinks, system_rpc_tx, config,
	})?;

	if let Some(suri) = register_key {
		match keystore.write().insert::<kulupu_pow::app::Pair>(suri) {
			Ok(_) => info!("Registered one key"),
			Err(e) => warn!("Registering key failed: {:?}", e),
		}
	}

	if role.is_authority() {
		let author = decode_author(author);
		let algorithm = kulupu_pow::RandomXAlgorithm::new(
			client.clone(),
			Some(keystore.clone()),
		);

		for _ in 0..threads {
			let proposer = sc_basic_authorship::ProposerFactory::new(
				client.clone(),
				transaction_pool.clone(),
				None,
			);

			sc_consensus_pow::start_mine(
				Box::new(pow_block_import.clone()),
				client.clone(),
				algorithm.clone(),
				proposer,
				author.clone().map(|a| a.encode()),
				round,
				network.clone(),
				std::time::Duration::new(2, 0),
				Some(select_chain.clone()),
				inherent_data_providers.clone(),
				sp_consensus::AlwaysCanAuthor,
			);
		}
	}

	network_starter.start_network();
	Ok(task_manager)
}

/// Builds a new service for a light client.
pub fn new_light(
	config: Configuration,
	author: Option<&str>,
	check_inherents_after: u32,
	donate: bool,
) -> Result<TaskManager, ServiceError> {
	let (client, backend, keystore, mut task_manager, on_demand) =
		sc_service::new_light_parts::<Block, RuntimeApi, Executor>(&config)?;

	let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
		on_demand.clone(),
	));

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let inherent_data_providers = kulupu_inherent_data_providers(author, donate)?;

	let algorithm = kulupu_pow::RandomXAlgorithm::new(client.clone());

	let pow_block_import = sc_consensus_pow::PowBlockImport::new(
		client.clone(),
		client.clone(),
		algorithm.clone(),
		check_inherents_after,
		Some(select_chain),
		inherent_data_providers.clone(),
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		None,
		algorithm.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_handle(),
		config.prometheus_registry(),
	)?;

	let (network, network_status_sinks, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: Some(on_demand.clone()),
			block_announce_validator_builder: None,
			finality_proof_request_builder: None,
			finality_proof_provider: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config, backend.clone(), task_manager.spawn_handle(), client.clone(), network.clone(),
		);
	}

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		remote_blockchain: Some(backend.remote_blockchain()),
		transaction_pool,
		task_manager: &mut task_manager,
		on_demand: Some(on_demand),
		rpc_extensions_builder: Box::new(|_| ()),
		telemetry_connection_sinks: sc_service::TelemetryConnectionSinks::default(),
		config,
		client,
		keystore,
		backend,
		network,
		network_status_sinks,
		system_rpc_tx,
	 })?;

	 network_starter.start_network();

	 Ok(task_manager)
}
