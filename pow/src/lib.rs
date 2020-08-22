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

mod compute;

use std::sync::Arc;
use codec::{Encode, Decode};
use sp_core::{U256, H256};
use sp_api::ProvideRuntimeApi;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::{
	Block as BlockT, Header as HeaderT, UniqueSaturatedInto,
};
use sp_consensus_pow::{Seal as RawSeal, DifficultyApi};
use sc_consensus_pow::PowAlgorithm;
use sc_client_api::{blockchain::HeaderBackend, backend::AuxStore};
use sc_keystore::KeyStorePtr;
use kulupu_primitives::{Difficulty, AlgorithmApi};
use rand::{SeedableRng, thread_rng, rngs::SmallRng};
use crate::compute::{ComputeV1, ComputeV2, SealV1, SealV2, ComputeMode};

pub mod app {
	use sp_application_crypto::{app_crypto, sr25519};
	use sp_core::crypto::KeyTypeId;

	pub const ID: KeyTypeId = KeyTypeId(*b"klp2");
	app_crypto!(sr25519, ID);
}

/// Checks whether the given hash is above difficulty.
fn is_valid_hash(hash: &H256, difficulty: Difficulty) -> bool {
	let num_hash = U256::from(&hash[..]);
	let (_, overflowed) = num_hash.overflowing_mul(difficulty);

	!overflowed
}

fn key_hash<B, C>(
	client: &C,
	parent: &BlockId<B>
) -> Result<H256, sc_consensus_pow::Error<B>> where
	B: BlockT<Hash=H256>,
	C: HeaderBackend<B>,
{
	const PERIOD: u64 = 4096; // ~2.8 days
	const OFFSET: u64 = 128;  // 2 hours

	let parent_header = client.header(parent.clone())
		.map_err(|e| sc_consensus_pow::Error::Environment(
			format!("Client execution error: {:?}", e)
		))?
		.ok_or(sc_consensus_pow::Error::Environment(
			"Parent header not found".to_string()
		))?;
	let parent_number = UniqueSaturatedInto::<u64>::unique_saturated_into(*parent_header.number());

	let mut key_number = parent_number.saturating_sub(parent_number % PERIOD);
	if parent_number.saturating_sub(key_number) < OFFSET {
		key_number = key_number.saturating_sub(PERIOD);
	}

	let mut current = parent_header;
	while UniqueSaturatedInto::<u64>::unique_saturated_into(*current.number()) != key_number {
		current = client.header(BlockId::Hash(*current.parent_hash()))
			.map_err(|e| sc_consensus_pow::Error::Environment(
				format!("Client execution error: {:?}", e)
			))?
			.ok_or(sc_consensus_pow::Error::Environment(
				format!("Block with hash {:?} not found", current.hash())
			))?;
	}

	Ok(current.hash())
}

pub enum RandomXAlgorithmVersion {
	V1,
	V2,
}

pub struct RandomXAlgorithm<C> {
	client: Arc<C>,
	keystore: Option<KeyStorePtr>,
}

impl<C> RandomXAlgorithm<C> {
	pub fn new(client: Arc<C>, keystore: Option<KeyStorePtr>) -> Self {
		Self { client, keystore }
	}
}

impl<C> Clone for RandomXAlgorithm<C> {
	fn clone(&self) -> Self {
		Self {
			client: self.client.clone(),
			keystore: self.keystore.clone(),
		}
	}
}

impl<B: BlockT<Hash=H256>, C> PowAlgorithm<B> for RandomXAlgorithm<C> where
	C: HeaderBackend<B> + AuxStore + ProvideRuntimeApi<B>,
	C::Api: DifficultyApi<B, Difficulty> + AlgorithmApi<B>,
{
	type Difficulty = Difficulty;

	fn difficulty(&self, parent: H256) -> Result<Difficulty, sc_consensus_pow::Error<B>> {
		let difficulty = self.client.runtime_api().difficulty(&BlockId::Hash(parent))
			.map_err(|e| sc_consensus_pow::Error::Environment(
				format!("Fetching difficulty from runtime failed: {:?}", e)
			));

		difficulty
	}

	fn verify(
		&self,
		parent: &BlockId<B>,
		pre_hash: &H256,
		pre_digest: Option<&[u8]>,
		seal: &RawSeal,
		difficulty: Difficulty,
	) -> Result<bool, sc_consensus_pow::Error<B>> {
		let version_raw = self.client.runtime_api().identifier(parent)
			.map_err(|e| sc_consensus_pow::Error::Environment(
				format!("Fetching identifier from runtime failed: {:?}", e))
			)?;

		let version = match version_raw {
			kulupu_primitives::ALGORITHM_IDENTIFIER_V1 => RandomXAlgorithmVersion::V1,
			kulupu_primitives::ALGORITHM_IDENTIFIER_V2 => RandomXAlgorithmVersion::V2,
			_ => return Err(sc_consensus_pow::Error::<B>::Other(
				"Unknown algorithm identifier".to_string(),
			)),
		};

		let key_hash = key_hash(self.client.as_ref(), parent)?;

		match version {
			RandomXAlgorithmVersion::V1 => {
				let seal = match SealV1::decode(&mut &seal[..]) {
					Ok(seal) => seal,
					Err(_) => return Ok(false),
				};

				let compute = ComputeV1 {
					key_hash,
					difficulty,
					pre_hash: *pre_hash,
					nonce: seal.nonce,
				};

				// No pre-digest check is needed for V1 algorithm.

				let (computed_seal, computed_work) = compute.seal_and_work(ComputeMode::Sync);

				if computed_seal != seal {
					return Ok(false)
				}

				if !is_valid_hash(&computed_work, difficulty) {
					return Ok(false)
				}

				Ok(true)
			},
			RandomXAlgorithmVersion::V2 => {
				let seal = match SealV2::decode(&mut &seal[..]) {
					Ok(seal) => seal,
					Err(_) => return Ok(false),
				};

				let compute = ComputeV2 {
					key_hash,
					difficulty,
					pre_hash: *pre_hash,
					nonce: seal.nonce,
				};

				let pre_digest = match pre_digest {
					Some(pre_digest) => pre_digest,
					None => return Ok(false),
				};

				let author = match app::Public::decode(&mut &pre_digest[..]) {
					Ok(author) => author,
					Err(_) => return Ok(false),
				};

				if !compute.verify(&seal.signature, &author) {
					return Ok(false)
				}

				let (computed_seal, computed_work) = compute.seal_and_work(
					seal.signature.clone(),
					ComputeMode::Sync,
				);

				if computed_seal != seal {
					return Ok(false)
				}

				if !is_valid_hash(&computed_work, difficulty) {
					return Ok(false)
				}

				Ok(true)
			},
		}
	}

	fn mine(
		&self,
		parent: &BlockId<B>,
		pre_hash: &H256,
		pre_digest: Option<&[u8]>,
		difficulty: Difficulty,
		round: u32,
	) -> Result<Option<RawSeal>, sc_consensus_pow::Error<B>> {
		let version_raw = self.client.runtime_api().identifier(parent)
			.map_err(|e| sc_consensus_pow::Error::Environment(
				format!("Fetching identifier from runtime failed: {:?}", e))
			)?;

		let version = match version_raw {
			kulupu_primitives::ALGORITHM_IDENTIFIER_V1 => RandomXAlgorithmVersion::V1,
			kulupu_primitives::ALGORITHM_IDENTIFIER_V2 => RandomXAlgorithmVersion::V2,
			_ => return Err(sc_consensus_pow::Error::<B>::Other(
				"Unknown algorithm identifier".to_string()
			)),
		};

		let mut rng = SmallRng::from_rng(&mut thread_rng())
			.map_err(|e| sc_consensus_pow::Error::Environment(
				format!("Initialize RNG failed for mining: {:?}", e)
			))?;
		let key_hash = key_hash(self.client.as_ref(), parent)?;

		match version {
			RandomXAlgorithmVersion::V1 => {
				for _ in 0..round {
					let nonce = H256::random_using(&mut rng);

					let compute = ComputeV1 {
						key_hash,
						difficulty,
						pre_hash: *pre_hash,
						nonce,
					};

					let (seal, work) = compute.seal_and_work(ComputeMode::Mining);

					if is_valid_hash(&work, difficulty) {
						return Ok(Some(seal.encode()))
					}
				}

				Ok(None)
			},
			RandomXAlgorithmVersion::V2 => {
				let pre_digest = pre_digest.ok_or(sc_consensus_pow::Error::<B>::Other(
					"Unable to mine: v2 pre-digest not set".to_string(),
				))?;

				let author = app::Public::decode(&mut &pre_digest[..]).map_err(|_| {
					sc_consensus_pow::Error::<B>::Other(
						"Unable to mine: v2 author pre-digest decoding failed".to_string(),
					)
				})?;

				let pair = self.keystore.as_ref().ok_or(sc_consensus_pow::Error::<B>::Other(
					"Unable to mine: v2 keystore not set".to_string(),
				))?.read().key_pair::<app::Pair>(
					&author,
				).map_err(|_| sc_consensus_pow::Error::<B>::Other(
					"Unable to mine: v2 fetch pair from author failed".to_string(),
				))?;

				for _ in 0..round {
					let nonce = H256::random_using(&mut rng);

					let compute = ComputeV2 {
						key_hash,
						difficulty,
						pre_hash: *pre_hash,
						nonce,
					};

					let signature = compute.sign(&pair);
					let (seal, work) = compute.seal_and_work(signature, ComputeMode::Mining);

					if is_valid_hash(&work, difficulty) {
						return Ok(Some(seal.encode()))
					}
				}

				Ok(None)
			},
		}
	}
}
