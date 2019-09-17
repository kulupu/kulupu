mod compact;

use std::sync::Arc;
use primitives::{U256, H256};
use sr_primitives::generic::BlockId;
use sr_primitives::traits::{
	Block as BlockT, Header as HeaderT, ProvideRuntimeApi, UniqueSaturatedInto,
};
use client::{blockchain::HeaderBackend, backend::AuxStore};
use codec::{Encode, Decode};
use consensus_pow::PowAlgorithm;
use consensus_pow_primitives::{Difficulty, Seal as RawSeal, DifficultyApi};
use kulupu_primitives::{DAY_HEIGHT, HOUR_HEIGHT};
use compact::Compact;
use log::*;

#[derive(Clone, PartialEq, Eq, Encode, Decode)]
pub struct Seal {
	pub nonce: H256,
	pub work: H256,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Compute {
	pub key_hash: H256,
	pub pre_hash: H256,
	pub nonce: H256,
}

impl Compute {
	pub fn compute(self) -> Seal {
		let mut vm = randomx::VM::new(&self.key_hash[..]);
		let work = vm.calculate(&(self.pre_hash, self.nonce).encode()[..]);

		Seal {
			nonce: self.nonce,
			work: H256::from(work),
		}
	}
}

fn key_hash<B, C>(
	client: &C,
	parent: &BlockId<B>
) -> Result<H256, String> where
	B: BlockT<Hash=H256>,
	C: HeaderBackend<B>,
{
	let parent_header = client.header(parent.clone())
		.map_err(|e| format!("Client execution error: {:?}", e))?
		.ok_or("Parent header not found")?;
	let parent_number = UniqueSaturatedInto::<u64>::unique_saturated_into(*parent_header.number());

	let mut key_number = parent_number.saturating_sub(parent_number % DAY_HEIGHT);
	if parent_number.saturating_sub(key_number) < 2 * HOUR_HEIGHT {
		key_number = key_number.saturating_sub(DAY_HEIGHT);
	}

	let mut current = parent_header;
	while UniqueSaturatedInto::<u64>::unique_saturated_into(*current.number()) != key_number {
		current = client.header(BlockId::Hash(*current.parent_hash()))
			.map_err(|e| format!("Client execution error: {:?}", e))?
			.ok_or(format!("Block with hash {:?} not found", current.hash()))?;
	}

	Ok(current.hash())
}

pub struct RandomXAlgorithm<C> {
	client: Arc<C>,
}

impl<C> RandomXAlgorithm<C> {
	pub fn new(client: Arc<C>) -> Self {
		Self { client }
	}
}

impl<B: BlockT<Hash=H256>, C> PowAlgorithm<B> for RandomXAlgorithm<C> where
	C: HeaderBackend<B> + AuxStore + ProvideRuntimeApi,
	C::Api: DifficultyApi<B>,
{
	fn difficulty(&self, parent: &BlockId<B>) -> Result<Difficulty, String> {
		self.client.runtime_api().difficulty(parent)
			.map_err(|e| format!("Fetching difficulty from runtime failed: {:?}", e))
	}

	fn verify(
		&self,
		parent: &BlockId<B>,
		pre_hash: &H256,
		seal: &RawSeal,
		difficulty: Difficulty,
	) -> Result<bool, String> {
		let key_hash = key_hash(self.client.as_ref(), parent)?;

		let seal = match Seal::decode(&mut &seal[..]) {
			Ok(seal) => seal,
			Err(_) => return Ok(false),
		};

		let compact = Compact::from(difficulty);
		if !compact.verify(seal.work) {
			return Ok(false)
		}

		let compute = Compute {
			key_hash,
			pre_hash: *pre_hash,
			nonce: seal.nonce,
		};

		if compute.compute() != seal {
			return Ok(false)
		}

		Ok(true)
	}

	fn mine(
		&self,
		parent: &BlockId<B>,
		pre_hash: &H256,
		seed: &H256,
		difficulty: Difficulty,
		round: u32,
	) -> Result<Option<RawSeal>, String> {
		let key_hash = key_hash(self.client.as_ref(), parent)?;

		for i in 0..round {
			let nonce = {
				let mut ret = H256::default();
				(U256::from(&seed[..]) + U256::from(i)).to_big_endian(&mut ret[..]);
				ret
			};

			let compute = Compute {
				key_hash,
				pre_hash: *pre_hash,
				nonce,
			};

			let seal = compute.compute();

			let compact = Compact::from(difficulty);
			if !compact.verify(seal.work) {
				return Ok(Some(seal.encode()))
			}
		}

		Ok(None)
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn randomx_len() {
		assert_eq!(randomx::HASH_SIZE, 32);
	}
}
