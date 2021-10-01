// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2020 Wei Tang.
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

#![warn(missing_docs)]

use std::sync::Arc;

use kulupu_pow::RandomXAlgorithm;
use kulupu_primitives::{AlgorithmApi, Difficulty};
use kulupu_runtime::{opaque::Block, AccountId, Balance, BlockNumber, Hash, Index};
use parking_lot::Mutex;
use sc_client_api::backend::AuxStore;
use sc_consensus_pow::MiningWorker;
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus_pow::DifficultyApi;

/// Full client dependencies.
pub struct FullDeps<
	C: HeaderBackend<Block> + AuxStore + sp_api::ProvideRuntimeApi<Block>,
	L: sc_consensus::JustificationSyncLink<Block>,
	P,
	Proof,
> where
	C::Api: DifficultyApi<Block, Difficulty> + AlgorithmApi<Block>,
{
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// Mining worker.
	pub mining_worker: Arc<Mutex<MiningWorker<Block, RandomXAlgorithm<C>, C, L, Proof>>>,
}

/// Instantiate all full RPC extensions.
pub fn create_full<C, L, P, Proof>(
	deps: FullDeps<C, L, P, Proof>,
) -> jsonrpc_core::IoHandler<sc_rpc::Metadata>
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + AuxStore,
	C: Send + Sync + 'static,
	L: sc_consensus::JustificationSyncLink<Block> + 'static,
	sp_api::TransactionFor<C, Block>: Send + 'static,
	Proof: Send + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_contracts_rpc::ContractsRuntimeApi<Block, AccountId, Balance, BlockNumber, Hash>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: DifficultyApi<Block, Difficulty> + AlgorithmApi<Block>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + 'static,
{
	use kulupu_rpc_work::{RpcWork, RpcWorkApi};
	use pallet_contracts_rpc::{Contracts, ContractsApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
	use substrate_frame_rpc_system::{FullSystem, SystemApi};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		pool,
		deny_unsafe,
		mining_worker,
	} = deps;

	io.extend_with(SystemApi::to_delegate(FullSystem::new(
		client.clone(),
		pool,
		deny_unsafe,
	)));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	io.extend_with(ContractsApi::to_delegate(Contracts::new(client.clone())));
	io.extend_with(RpcWorkApi::to_delegate(RpcWork::new(
		client.clone(),
		mining_worker,
	)));

	io
}
