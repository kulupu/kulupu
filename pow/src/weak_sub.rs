// This file is part of Substrate.

// Copyright (C) 2017-2020 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::{
	sync::Arc, collections::HashMap, marker::PhantomData, fmt::Debug,
};
use sc_client_api::{BlockOf, AuxStore};
use sp_api::ProvideRuntimeApi;
use sp_core::U256;
use sp_runtime::{traits::{Block as BlockT, Header as HeaderT}};
use sp_blockchain::{
	well_known_cache_keys::Id as CacheKeyId, HeaderMetadata,
};
use sp_consensus::{
	ImportResult, BlockImportParams, BlockCheckParams, Error as ConsensusError, BlockImport,
	SelectChain, ForkChoiceStrategy,
};
use sc_consensus_pow::{PowAlgorithm, PowAux};
use log::*;

/// Parameters passed to decision function of whether to block the reorg.
pub struct WeakSubjectiveParams {
	/// Total difficulty of the best block.
	pub best_total_difficulty: U256,
	/// Total difficulty of the common ancestor.
	pub common_total_difficulty: U256,
	/// Total difficulty of the new block to be imported.
	pub new_total_difficulty: U256,
	/// Retracted block length if the reorg happens.
	pub retracted_len: usize,
}

/// Deccision of weak subjectivity.
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum WeakSubjectiveDecision {
	/// Block the reorg.
	BlockReorg,
	/// Continue the normal import.
	Continue,
}

/// Algorithm used for the decision function of weak subjectivity.
pub trait WeakSubjectiveAlgorithm {
	/// Decide based on the weak subjectivity parameters of whether to block the import.
	fn weak_subjective_decide(
		&self,
		params: WeakSubjectiveParams,
	) -> WeakSubjectiveDecision;
}

/// Exponential weak subjectivity algorithm for U256 difficulty type.
#[derive(Clone, Debug)]
pub struct ExponentialWeakSubjectiveAlgorithm(pub usize, pub f64);

impl WeakSubjectiveAlgorithm for ExponentialWeakSubjectiveAlgorithm {
	fn weak_subjective_decide(
		&self,
		params: WeakSubjectiveParams,
	) -> WeakSubjectiveDecision {
		if params.retracted_len <= self.0 {
			return WeakSubjectiveDecision::Continue
		}

		let mut best_diff = params.best_total_difficulty
			.saturating_sub(params.common_total_difficulty);
		let mut new_diff = params.new_total_difficulty
			.saturating_sub(params.common_total_difficulty);

		while best_diff > U256::from(u128::max_value()) ||
			new_diff > U256::from(u128::max_value())
		{
			best_diff /= U256::from(2);
			new_diff /= U256::from(2);
		}

		let left = (new_diff.as_u128() as f64) / (best_diff.as_u128() as f64);
		let right = self.1.powi(params.retracted_len.saturating_sub(self.0) as i32);

		if left > right {
			WeakSubjectiveDecision::Continue
		} else {
			WeakSubjectiveDecision::BlockReorg
		}
	}
}

/// Block import for weak subjectivity. It must be combined with a PoW block import.
pub struct WeakSubjectiveBlockImport<B: BlockT, I, C, S, Pow, Reorg> {
	inner: I,
	client: Arc<C>,
	select_chain: S,
	pow_algorithm: Pow,
	reorg_algorithm: Reorg,
	enabled: bool,
	_marker: PhantomData<B>,
}

impl<B: BlockT, I: Clone, C, S: Clone, Pow: Clone, Reorg: Clone> Clone
	for WeakSubjectiveBlockImport<B, I, C, S, Pow, Reorg>
{
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
			client: self.client.clone(),
			select_chain: self.select_chain.clone(),
			pow_algorithm: self.pow_algorithm.clone(),
			reorg_algorithm: self.reorg_algorithm.clone(),
			enabled: self.enabled.clone(),
			_marker: PhantomData,
		}
	}
}

impl<B, I, C, S, Pow, Reorg> WeakSubjectiveBlockImport<B, I, C, S, Pow, Reorg> where
	B: BlockT,
	I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + HeaderMetadata<B> + BlockOf + AuxStore + Send + Sync,
	C::Error: Debug,
	S: SelectChain<B>,
	Pow: PowAlgorithm<B, Difficulty=U256>,
	Reorg: WeakSubjectiveAlgorithm,
{
	/// Create a new block import for weak subjectivity.
	pub fn new(
		inner: I,
		client: Arc<C>,
		pow_algorithm: Pow,
		reorg_algorithm: Reorg,
		select_chain: S,
		enabled: bool,
	) -> Self {
		Self {
			inner,
			client,
			pow_algorithm,
			reorg_algorithm,
			select_chain,
			enabled,
			_marker: PhantomData,
		}
	}
}

impl<B, I, C, S, Pow, Reorg> BlockImport<B> for WeakSubjectiveBlockImport<B, I, C, S, Pow, Reorg> where
	B: BlockT,
	I: BlockImport<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + HeaderMetadata<B> + BlockOf + AuxStore + Send + Sync,
	C::Error: Debug,
	S: SelectChain<B>,
	Pow: PowAlgorithm<B, Difficulty=U256>,
	Reorg: WeakSubjectiveAlgorithm,
{
	type Error = ConsensusError;
	type Transaction = sp_api::TransactionFor<C, B>;

	fn check_block(
		&mut self,
		block: BlockCheckParams<B>,
	) -> Result<ImportResult, Self::Error> {
		self.inner.check_block(block).map_err(Into::into)
	}

	fn import_block(
		&mut self,
		mut block: BlockImportParams<B, Self::Transaction>,
		new_cache: HashMap<CacheKeyId, Vec<u8>>,
	) -> Result<ImportResult, Self::Error> {
		if self.enabled {
			let best_header = self.select_chain.best_chain()
				.map_err(|e| format!("Fetch best chain failed via select chain: {:?}", e))?;
			let best_hash = best_header.hash();

			let parent_hash = *block.header.parent_hash();
			let route_from_best = sp_blockchain::tree_route(
				self.client.as_ref(),
				best_hash,
				parent_hash,
			).map_err(|e| format!("Find route from best failed: {:?}", e))?;

			let retracted_len = route_from_best.retracted().len();

			let best_difficulty_aux = PowAux::<U256>::read::<_, B>(
				self.client.as_ref(),
				&best_hash,
			)?;
			let parent_difficulty_aux = PowAux::<U256>::read::<_, B>(
				self.client.as_ref(),
				&parent_hash,
			)?;
			let common_difficulty_aux = PowAux::<U256>::read::<_, B>(
				self.client.as_ref(),
				&route_from_best.common_block().hash,
			)?;

			let best_total_difficulty = best_difficulty_aux.total_difficulty;
			let common_total_difficulty = common_difficulty_aux.total_difficulty;
			let new_total_difficulty = parent_difficulty_aux.total_difficulty +
				self.pow_algorithm.difficulty(parent_hash)?;

			let params = WeakSubjectiveParams {
				best_total_difficulty,
				common_total_difficulty,
				new_total_difficulty,
				retracted_len,
			};

			match self.reorg_algorithm.weak_subjective_decide(params) {
				WeakSubjectiveDecision::BlockReorg => {
					warn!(
						target: "kulupu-pow",
						"Weak subjectivity blocked a deep chain reorg. Retracted len: {}, current head total difficulty: {}, reorg total difficulty: {}",
						retracted_len,
						best_total_difficulty,
						new_total_difficulty,
					);
					block.fork_choice = Some(ForkChoiceStrategy::Custom(false));
				},
				WeakSubjectiveDecision::Continue => (),
			}
		}

		self.inner.import_block(block, new_cache).map_err(Into::into)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use WeakSubjectiveDecision::*;

	fn check(best_diff: U256, new_diff: U256, retracted_len: usize, decision: WeakSubjectiveDecision) {
		let algorithm = ExponentialWeakSubjectiveAlgorithm(30, 1.1);
		let params = WeakSubjectiveParams {
			best_total_difficulty: best_diff + U256::from(1000),
			common_total_difficulty: U256::from(1000),
			new_total_difficulty: new_diff + U256::from(1000),
			retracted_len,
		};

		assert_eq!(decision, algorithm.weak_subjective_decide(params));
	}

	#[test]
	fn less_than_30_block_should_not_be_affected() {
		check(U256::from(7000), U256::from(8000), 20, Continue);
		check(U256::from(7000), U256::from(7001), 30, Continue);
	}

	#[test]
	fn more_than_30_block_should_be_panelized() {
		check(U256::from(7000), U256::from(7001), 31, BlockReorg);
		check(U256::from(7000), U256::from(8000), 31, Continue);
		check(U256::from(7000), U256::from(8000), 40, BlockReorg);
	}
}
