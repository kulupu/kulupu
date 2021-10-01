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

use jsonrpc_derive::rpc;
use kulupu_pow::RandomXAlgorithm;
use kulupu_primitives::{AlgorithmApi, Difficulty};
use parity_scale_codec::Encode;
use parking_lot::Mutex;
use sc_client_api::{backend::AuxStore, blockchain::HeaderBackend};
use sc_consensus_pow::MiningWorker;
use serde::{Deserialize, Serialize};
use sp_consensus_pow::DifficultyApi;
use sp_core::{sr25519, H256};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

pub fn internal<E: ::std::fmt::Debug>(e: E) -> jsonrpc_core::Error {
	jsonrpc_core::Error {
		code: jsonrpc_core::ErrorCode::InternalError,
		message: "Internal error occurred".into(),
		data: Some(format!("{:?}", e).into()),
	}
}

#[derive(Serialize, Deserialize)]
pub struct Compute {
	pub key_hash: H256,
	pub pre_hash: H256,
	pub difficulty: Difficulty,
}

#[derive(Serialize, Deserialize)]
pub struct Seal {
	pub nonce: H256,
	pub signature: sr25519::Signature,
}

#[rpc]
pub trait RpcWorkApi {
	#[rpc(name = "work_getCompute")]
	fn get_compute(&self) -> Result<Compute, jsonrpc_core::Error>;
	#[rpc(name = "work_submitSeal")]
	fn submit_seal(&self, seal: Seal) -> Result<bool, jsonrpc_core::Error>;
}

pub struct RpcWork<
	Block: BlockT<Hash = H256>,
	C: HeaderBackend<Block> + AuxStore + sp_api::ProvideRuntimeApi<Block>,
	L: sc_consensus::JustificationSyncLink<Block>,
	Proof,
> where
	C::Api: DifficultyApi<Block, Difficulty> + AlgorithmApi<Block>,
{
	client: Arc<C>,
	worker: Arc<Mutex<MiningWorker<Block, RandomXAlgorithm<C>, C, L, Proof>>>,
}

impl<Block, C, L, Proof> RpcWork<Block, C, L, Proof>
where
	Block: BlockT<Hash = H256>,
	C: HeaderBackend<Block> + AuxStore + sp_api::ProvideRuntimeApi<Block> + 'static,
	C::Api: DifficultyApi<Block, Difficulty> + AlgorithmApi<Block>,
	L: sc_consensus::JustificationSyncLink<Block> + 'static,
	sp_api::TransactionFor<C, Block>: Send + 'static,
	Proof: Send + 'static,
{
	pub fn new(
		client: Arc<C>,
		worker: Arc<Mutex<MiningWorker<Block, RandomXAlgorithm<C>, C, L, Proof>>>,
	) -> Self {
		Self { client, worker }
	}
}

impl<Block, C, L, Proof> RpcWorkApi for RpcWork<Block, C, L, Proof>
where
	Block: BlockT<Hash = H256>,
	C: HeaderBackend<Block> + AuxStore + sp_api::ProvideRuntimeApi<Block> + 'static,
	C::Api: DifficultyApi<Block, Difficulty> + AlgorithmApi<Block>,
	L: sc_consensus::JustificationSyncLink<Block> + 'static,
	sp_api::TransactionFor<C, Block>: Send + 'static,
	Proof: Send + 'static,
{
	fn get_compute(&self) -> Result<Compute, jsonrpc_core::Error> {
		let metadata = self
			.worker
			.lock()
			.metadata()
			.ok_or(internal("metadata does not exist"))?;

		let key_hash =
			kulupu_pow::key_hash(self.client.as_ref(), &BlockId::Hash(metadata.best_hash))
				.map_err(|_| internal("fetch key hash failed"))?;

		let compute = Compute {
			key_hash,
			pre_hash: metadata.pre_hash,
			difficulty: metadata.difficulty,
		};

		Ok(compute)
	}

	fn submit_seal(&self, seal: Seal) -> Result<bool, jsonrpc_core::Error> {
		let metadata = self
			.worker
			.lock()
			.metadata()
			.ok_or(internal("metadata does not exist"))?;

		let raw_seal = kulupu_pow::compute::SealV2 {
			difficulty: metadata.difficulty,
			nonce: seal.nonce,
			signature: seal.signature.into(),
		}
		.encode();

		let _ = futures::executor::block_on(self.worker.lock().submit(raw_seal));

		Ok(true)
	}
}
