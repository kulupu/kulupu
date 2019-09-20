//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::sync::Arc;
use std::time::Duration;
use substrate_client::LongestChain;
use futures::prelude::*;
use kulupu_runtime::{self, GenesisConfig, opaque::Block, RuntimeApi};
use substrate_service::{error::{Error as ServiceError}, AbstractService, Configuration, ServiceBuilder};
use transaction_pool::{self, txpool::{Pool as TransactionPool}};
use inherents::InherentDataProviders;
use network::{config::DummyFinalityProofRequestBuilder, construct_simple_protocol};
use substrate_executor::native_executor_instance;
pub use substrate_executor::NativeExecutor;

// Our native executor instance.
native_executor_instance!(
	pub Executor,
	kulupu_runtime::api::dispatch,
	kulupu_runtime::native_version,
);

construct_simple_protocol! {
	/// Demo protocol attachment for substrate.
	pub struct NodeProtocol where Block = Block { }
}

pub fn kulupu_inherent_data_providers() -> Result<inherents::InherentDataProviders, ServiceError> {
	let mut inherent_data_providers = inherents::InherentDataProviders::new();

	if !inherent_data_providers.has_provider(&srml_timestamp::INHERENT_IDENTIFIER) {
		inherent_data_providers
			.register_provider(srml_timestamp::InherentDataProvider)
			.map_err(Into::into)
			.map_err(consensus_common::Error::InherentData)?;
	}

	if !inherent_data_providers.has_provider(&srml_anyupgrade::INHERENT_IDENTIFIER) {
		inherent_data_providers
			.register_provider(srml_anyupgrade::InherentDataProvider(Default::default()))
			.map_err(Into::into)
			.map_err(consensus_common::Error::InherentData)?;
	}

	Ok(inherent_data_providers)
}

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
macro_rules! new_full_start {
	($config:expr) => {{
		let inherent_data_providers = crate::service::kulupu_inherent_data_providers()?;

		let builder = substrate_service::ServiceBuilder::new_full::<
			kulupu_runtime::opaque::Block, kulupu_runtime::RuntimeApi, crate::service::Executor
		>($config)?
			.with_select_chain(|_config, backend| {
				Ok(substrate_client::LongestChain::new(backend.clone()))
			})?
			.with_transaction_pool(|config, client|
				Ok(transaction_pool::txpool::Pool::new(config, transaction_pool::ChainApi::new(client)))
			)?
			.with_import_queue(|config, client, select_chain, transaction_pool| {
				let import_queue = consensus_pow::import_queue(
					Box::new(client.clone()),
					client.clone(),
					kulupu_pow::RandomXAlgorithm::new(client.clone()),
					inherent_data_providers.clone(),
				)?;

				Ok(import_queue)
			})?;

		(builder, inherent_data_providers)
	}}
}

/// Builds a new service for a full client.
pub fn new_full<C: Send + Default + 'static>(config: Configuration<C, GenesisConfig>)
	-> Result<impl AbstractService, ServiceError>
{
	let is_authority = config.roles.is_authority();
	let name = config.name.clone();
	let force_authoring = config.force_authoring;

	let (builder, inherent_data_providers) = new_full_start!(config);

	let service = builder
		.with_network_protocol(|_| Ok(NodeProtocol::new()))?
		.with_finality_proof_provider(|client, backend| {
			Ok(Arc::new(()) as _)
		})?
		.build()?;

	if is_authority {
		let proposer = basic_authorship::ProposerFactory {
			client: service.client(),
			transaction_pool: service.transaction_pool(),
		};

		consensus_pow::start_mine(
			Box::new(service.client().clone()),
			service.client(),
			kulupu_pow::RandomXAlgorithm::new(service.client()),
			proposer,
			None,
			500,
			inherent_data_providers.clone(),
		);
	}

	Ok(service)
}

/// Builds a new service for a light client.
pub fn new_light<C: Send + Default + 'static>(config: Configuration<C, GenesisConfig>)
	-> Result<impl AbstractService, ServiceError>
{
	let inherent_data_providers = kulupu_inherent_data_providers()?;

	ServiceBuilder::new_light::<Block, RuntimeApi, Executor>(config)?
		.with_select_chain(|_config, backend| {
			Ok(LongestChain::new(backend.clone()))
		})?
		.with_transaction_pool(|config, client|
			Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client)))
		)?
		.with_import_queue_and_fprb(|_config, client, backend, fetcher, _select_chain, transaction_pool| {
			let fprb = Box::new(DummyFinalityProofRequestBuilder::default()) as Box<_>;
			let import_queue = consensus_pow::import_queue(
				Box::new(client.clone()),
				client.clone(),
				kulupu_pow::RandomXAlgorithm::new(client.clone()),
				inherent_data_providers.clone(),
			)?;

			Ok((import_queue, fprb))
		})?
		.with_finality_proof_provider(|client, backend| {
			Ok(Arc::new(()) as _)
		})?
		.with_network_protocol(|_| Ok(NodeProtocol::new()))?
		.build()
}
