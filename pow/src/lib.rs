use std::sync::Arc;
use std::cell::RefCell;
use primitives::{U256, H256};
use sr_primitives::generic::BlockId;
use sr_primitives::traits::{
	Block as BlockT, Header as HeaderT, ProvideRuntimeApi, UniqueSaturatedInto,
};
use client::{blockchain::HeaderBackend, backend::AuxStore};
use codec::{Encode, Decode};
use consensus_pow::PowAlgorithm;
use consensus_pow_primitives::{Seal as RawSeal, DifficultyApi};
use kulupu_primitives::{Difficulty, AlgorithmApi, DAY_HEIGHT, HOUR_HEIGHT};
use lru_cache::LruCache;
use rand::{SeedableRng, thread_rng, rngs::SmallRng};
use log::*;

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Seal {
	pub difficulty: Difficulty,
	pub work: H256,
	pub nonce: H256,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Calculation {
	pub difficulty: Difficulty,
	pub pre_hash: H256,
	pub nonce: H256,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Compute {
	pub key_hash: H256,
	pub pre_hash: H256,
	pub difficulty: Difficulty,
	pub nonce: H256,
}

thread_local!(static MACHINES: RefCell<LruCache<H256, randomx::FullVM>> = RefCell::new(LruCache::new(3)));

impl Compute {
	pub fn compute(self) -> Seal {
		MACHINES.with(|m| {
			let mut ms = m.borrow_mut();
			let calculation = Calculation {
				difficulty: self.difficulty,
				pre_hash: self.pre_hash,
				nonce: self.nonce,
			};

			let work = if let Some(vm) = ms.get_mut(&self.key_hash) {
				vm.calculate(&calculation.encode()[..])
			} else {
				let mut vm = randomx::FullVM::new(&self.key_hash[..]);
				let work = vm.calculate(&calculation.encode()[..]);
				ms.insert(self.key_hash, vm);
				work
			};

			Seal {
				nonce: self.nonce,
				difficulty: self.difficulty,
				work: H256::from(work),
			}
		})
	}
}

fn is_valid_hash(hash: &H256, difficulty: Difficulty) -> bool {
	let num_hash = U256::from(&hash[..]);
	let (_, overflowed) = num_hash.overflowing_mul(difficulty);

	!overflowed
}

fn key_hash<B, C>(
	client: &C,
	parent: &BlockId<B>
) -> Result<H256, String> where
	B: BlockT<Hash=H256>,
	C: HeaderBackend<B>,
{
	const PERIOD: u64 = 2 * DAY_HEIGHT;
	const OFFSET: u64 = 2 * HOUR_HEIGHT;

	let parent_header = client.header(parent.clone())
		.map_err(|e| format!("Client execution error: {:?}", e))?
		.ok_or("Parent header not found")?;
	let parent_number = UniqueSaturatedInto::<u64>::unique_saturated_into(*parent_header.number());

	let mut key_number = parent_number.saturating_sub(parent_number % PERIOD);
	if parent_number.saturating_sub(key_number) < OFFSET {
		key_number = key_number.saturating_sub(PERIOD);
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
	C::Api: DifficultyApi<B, Difficulty> + AlgorithmApi<B>,
{
	type Difficulty = Difficulty;

	fn difficulty(&self, parent: &BlockId<B>) -> Result<Difficulty, String> {
		let difficulty = self.client.runtime_api().difficulty(parent)
			.map_err(|e| format!("Fetching difficulty from runtime failed: {:?}", e));

		info!("Next block's difficulty: {:?}", difficulty);
		difficulty
	}

	fn verify(
		&self,
		parent: &BlockId<B>,
		pre_hash: &H256,
		seal: &RawSeal,
		difficulty: Difficulty,
	) -> Result<bool, String> {
		assert_eq!(self.client.runtime_api().identifier(parent)
				   .map_err(|e| format!("Fetching identifier from runtime failed: {:?}", e))?,
				   kulupu_primitives::ALGORITHM_IDENTIFIER);

		let key_hash = key_hash(self.client.as_ref(), parent)?;

		let seal = match Seal::decode(&mut &seal[..]) {
			Ok(seal) => seal,
			Err(_) => return Ok(false),
		};

		if !is_valid_hash(&seal.work, difficulty) {
			return Ok(false)
		}

		let compute = Compute {
			key_hash,
			difficulty,
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
		difficulty: Difficulty,
		round: u32,
	) -> Result<Option<RawSeal>, String> {
		let mut rng = SmallRng::from_rng(&mut thread_rng())
			.map_err(|e| format!("Initialize RNG failed for mining: {:?}", e))?;
		let key_hash = key_hash(self.client.as_ref(), parent)?;

		for _ in 0..round {
			let nonce = H256::random_using(&mut rng);

			let compute = Compute {
				key_hash,
				difficulty,
				pre_hash: *pre_hash,
				nonce,
			};

			let seal = compute.compute();

			if is_valid_hash(&seal.work, difficulty) {
				return Ok(Some(seal.encode()))
			}
		}

		Ok(None)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use kulupu_primitives::{H256, U256};

	#[test]
	fn randomx_len() {
		assert_eq!(randomx::HASH_SIZE, 32);
	}

	#[test]
	fn randomx_collision() {
		let mut compute = Compute {
			key_hash: H256::from([210, 164, 216, 149, 3, 68, 116, 1, 239, 110, 111, 48, 180, 102, 53, 180, 91, 84, 242, 90, 101, 12, 71, 70, 75, 83, 17, 249, 214, 253, 71, 89]),
			pre_hash: H256::default(),
			difficulty: U256::default(),
			nonce: H256::default(),
		};
		let hash1 = compute.clone().compute();
		U256::one().to_big_endian(&mut compute.nonce[..]);
		let hash2 = compute.compute();
		assert!(hash1 != hash2);
	}
}
