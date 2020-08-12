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
use sp_runtime::Permill;
use sp_core::{H256, crypto::{UncheckedFrom, Ss58Codec, Ss58AddressFormat}};
use sc_consensus::LongestChain;
use sc_service::{
	error::{Error as ServiceError}, Configuration, ServiceBuilder, TaskManager, ServiceComponents
};
use sc_executor::native_executor_instance;
use sc_network::config::DummyFinalityProofRequestBuilder;
use kulupu_runtime::{self, opaque::Block, RuntimeApi, AccountId};

pub use sc_executor::NativeExecutor;

// Our native executor instance.
native_executor_instance!(
	pub Executor,
	kulupu_runtime::api::dispatch,
	kulupu_runtime::native_version,
);

/// Inherent data provider for Kulupu.
pub fn kulupu_inherent_data_providers(
	author: Option<&str>
) -> Result<sp_inherents::InherentDataProviders, ServiceError> {
	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	if !inherent_data_providers.has_provider(&sp_timestamp::INHERENT_IDENTIFIER) {
		inherent_data_providers
			.register_provider(sp_timestamp::InherentDataProvider)
			.map_err(Into::into)
			.map_err(sp_consensus::Error::InherentData)?;
	}

	if let Some(author) = author {
		if !inherent_data_providers.has_provider(&pallet_rewards::INHERENT_IDENTIFIER) {
			inherent_data_providers
				.register_provider(pallet_rewards::InherentDataProvider(
					(if author.starts_with("0x") {
						AccountId::unchecked_from(
							H256::from_str(&author[2..]).expect("Invalid author account")
						)
					} else {
						let (address, version) = AccountId::from_ss58check_with_version(author)
							.expect("Invalid author address");
						assert!(version == Ss58AddressFormat::KulupuAccount, "Invalid author version");
						address
					}.encode(), Permill::zero())
				))
				.map_err(Into::into)
				.map_err(sp_consensus::Error::InherentData)?;
		}
	}

	Ok(inherent_data_providers)
}

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
macro_rules! new_full_start {
	($config:expr, $author:expr, $check_inherents_after:expr) => {{
		let mut import_setup = None;
		let inherent_data_providers = crate::service::kulupu_inherent_data_providers($author)?;

		let builder = sc_service::ServiceBuilder::new_full::<
			kulupu_runtime::opaque::Block, kulupu_runtime::RuntimeApi, crate::service::Executor
		>($config)?
			.with_select_chain(|_config, backend| {
				Ok(sc_consensus::LongestChain::new(backend.clone()))
			})?
			.with_transaction_pool(|builder| {
				let pool_api = sc_transaction_pool::FullChainApi::new(
					builder.client().clone(),
					None,
				);
				Ok(sc_transaction_pool::BasicPool::new_full(
					builder.config().transaction_pool.clone(),
					std::sync::Arc::new(pool_api),
					builder.prometheus_registry(),
					builder.spawn_handle(),
					builder.client().clone(),
				))
			})?
			.with_import_queue(|_config, client, select_chain, _transaction_pool, spawn_task_handle, prometheus_registry| {
				let algorithm = kulupu_pow::RandomXAlgorithm::new(client.clone());

				let pow_block_import = sc_consensus_pow::PowBlockImport::new(
					client.clone(),
					client.clone(),
					algorithm.clone(),
					$check_inherents_after,
					select_chain,
					inherent_data_providers.clone(),
				);

				let import_queue = sc_consensus_pow::import_queue(
					Box::new(pow_block_import.clone()),
					None,
					None,
					algorithm.clone(),
					inherent_data_providers.clone(),
					spawn_task_handle,
					prometheus_registry,
				)?;

				import_setup = Some((pow_block_import, algorithm));

				Ok(import_queue)
			})?;

		(builder, import_setup, inherent_data_providers)
	}}
}

/// Builds a new service for a full client.
pub fn new_full(
	config: Configuration,
	author: Option<&str>,
	threads: usize,
	round: u32,
	check_inherents_after: u32,
) -> Result<TaskManager, ServiceError> {
	let role = config.role.clone();

	let (builder, mut import_setup, inherent_data_providers) =
		new_full_start!(config, author, check_inherents_after);

	let (block_import, algorithm) = import_setup.take().expect("Link Half and Block Import are present for Full Services or setup failed before. qed");

	let ServiceComponents {
		client, transaction_pool, select_chain, network, task_manager, ..
	} = builder
		.with_finality_proof_provider(|_client, _backend| {
			Ok(Arc::new(()) as _)
		})?
		.build_full()?;

	if role.is_authority() {
		for _ in 0..threads {
			let proposer = sc_basic_authorship::ProposerFactory::new(
				client.clone(),
				transaction_pool.clone(),
				None,
			);

			sc_consensus_pow::start_mine(
				Box::new(block_import.clone()),
				client.clone(),
				algorithm.clone(),
				proposer,
				None,
				round,
				network.clone(),
				std::time::Duration::new(2, 0),
				select_chain.clone(),
				inherent_data_providers.clone(),
				sp_consensus::AlwaysCanAuthor,
			);
		}
	}

	Ok(task_manager)
}

/// Builds a new service for a light client.
pub fn new_light(
	config: Configuration,
	author: Option<&str>,
	check_inherents_after: u32,
) -> Result<TaskManager, ServiceError> {
	let inherent_data_providers = kulupu_inherent_data_providers(author)?;

	ServiceBuilder::new_light::<Block, RuntimeApi, Executor>(config)?
		.with_select_chain(|_config, backend| {
			Ok(LongestChain::new(backend.clone()))
		})?
		.with_transaction_pool(|builder| {
			let fetcher = builder.fetcher()
				.ok_or_else(|| "Trying to start light transaction pool without active fetcher")?;

			let pool_api = sc_transaction_pool::LightChainApi::new(
				builder.client().clone(),
				fetcher,
			);
			let pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
				builder.config().transaction_pool.clone(),
				Arc::new(pool_api),
				builder.prometheus_registry(),
				builder.spawn_handle(),
			));
			Ok(pool)
		})?
		.with_import_queue_and_fprb(|_config, client, _backend, _fetcher, select_chain, _transaction_pool, spawn_task_handle, prometheus_registry| {
			let fprb = Box::new(DummyFinalityProofRequestBuilder::default()) as Box<_>;

			let algorithm = kulupu_pow::RandomXAlgorithm::new(client.clone());

			let pow_block_import = sc_consensus_pow::PowBlockImport::new(
				client.clone(),
				client.clone(),
				algorithm.clone(),
				check_inherents_after,
				select_chain,
				inherent_data_providers.clone(),
			);

			let import_queue = sc_consensus_pow::import_queue(
				Box::new(pow_block_import.clone()),
				None,
				None,
				algorithm.clone(),
				inherent_data_providers.clone(),
				spawn_task_handle,
				prometheus_registry,
			)?;

			Ok((import_queue, fprb))
		})?
		.with_finality_proof_provider(|_client, _backend| {
			Ok(Arc::new(()) as _)
		})?
		.build_light()
		.map(|ServiceComponents { task_manager, .. }| task_manager)
}
