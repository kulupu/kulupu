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
use kulupu_primitives::{Difficulty, DAY_HEIGHT, HOUR_HEIGHT};
use lru_cache::LruCache;
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

thread_local!(static MACHINES: RefCell<LruCache<H256, randomx::VM>> = RefCell::new(LruCache::new(3)));

impl Compute {
	pub fn compute(self) -> Seal {
		MACHINES.with(|m| {
			let mut ms = m.borrow_mut();

			let work = if let Some(vm) = ms.get_mut(&self.key_hash) {
				vm.calculate(&(self.pre_hash, self.nonce).encode()[..])
			} else {
				let mut vm = randomx::VM::new(&self.key_hash[..]);
				let work = vm.calculate(&(self.pre_hash, self.nonce).encode()[..]);
				ms.insert(self.key_hash, vm);
				work
			};

			Seal {
				nonce: self.nonce,
				work: H256::from(work),
			}
		})
	}
}

fn is_valid_hash(hash: &H256, difficulty: Difficulty) -> bool {
	let num_hash = U256::from(&hash[..]);
	let (_, overflowed) = num_hash.overflowing_mul(difficulty.0);

	!overflowed
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
	C::Api: DifficultyApi<B, Difficulty>,
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

			if is_valid_hash(&seal.work, difficulty) {
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
