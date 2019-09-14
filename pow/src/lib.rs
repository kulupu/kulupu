use core::cmp::{min, max};
use std::sync::Arc;
use primitives::{U256, H256};
use sr_primitives::generic::BlockId;
use sr_primitives::traits::{
	Block as BlockT, Header as HeaderT, ProvideRuntimeApi, UniqueSaturatedInto,
};
use client::{blockchain::HeaderBackend, backend::AuxStore};
use codec::{Encode, Decode};
use consensus_pow::{PowAux, PowAlgorithm};
use consensus_pow_primitives::{Difficulty, Seal as RawSeal, TimestampApi};

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

/// Block interval, in seconds, the network will tune its next_target for.
pub const BLOCK_TIME_SEC: u64 = 60;

/// Nominal height for standard time intervals, hour is 60 blocks
pub const HOUR_HEIGHT: u64 = 3600 / BLOCK_TIME_SEC;
/// A day is 1440 blocks
pub const DAY_HEIGHT: u64 = 24 * HOUR_HEIGHT;
/// A week is 10_080 blocks
pub const WEEK_HEIGHT: u64 = 7 * DAY_HEIGHT;
/// A year is 524_160 blocks
pub const YEAR_HEIGHT: u64 = 52 * WEEK_HEIGHT;

/// Number of blocks used to calculate difficulty adjustments
pub const DIFFICULTY_ADJUST_WINDOW: u64 = HOUR_HEIGHT;
/// Average time span of the difficulty adjustment window
pub const BLOCK_TIME_WINDOW: u64 = DIFFICULTY_ADJUST_WINDOW * BLOCK_TIME_SEC;
/// Clamp factor to use for difficulty adjustment
/// Limit value to within this factor of goal
pub const CLAMP_FACTOR: Difficulty = 2;
/// Dampening factor to use for difficulty adjustment
pub const DIFFICULTY_DAMP_FACTOR: Difficulty = 3;
/// Minimum difficulty, enforced in diff retargetting
/// avoids getting stuck when trying to increase difficulty subject to dampening
pub const MIN_DIFFICULTY: Difficulty = DIFFICULTY_DAMP_FACTOR;

fn difficulty_from_hash(
	hash: H256
) -> Difficulty {
	let mut num = U256::from(&hash[..]);
	if num == U256::max_value() {
		num = num - U256::one();
	}

	let diff = U256::max_value() / (num + U256::one());

	if diff >= U256::from(u64::max_value()) {
		u64::max_value() as u128
	} else {
		num.as_u128()
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
		current = client.header(BlockId::Hash(current.hash()))
			.map_err(|e| format!("Client execution error: {:?}", e))?
			.ok_or(format!("Block with hash {:?} not found", current.hash()))?;
	}

	Ok(current.hash())
}

/// Difficulty data from earliest to latest.
fn difficulty_data<B, C>(
	client: &C,
	parent: &BlockId<B>
) -> Result<Vec<(u64, Difficulty)>, String> where
	B: BlockT<Hash=H256>,
	C: HeaderBackend<B> + AuxStore + ProvideRuntimeApi,
	C::Api: TimestampApi<B, u64>,
{
	let needed_block_count = DIFFICULTY_ADJUST_WINDOW as usize + 1;
	let parent_header = client.header(parent.clone())
		.map_err(|e| format!("Client execution error: {:?}", e))?
		.ok_or("Parent header not found")?;
	let parent_hash = parent_header.hash();
	let parent_timestamp = client.runtime_api()
		.timestamp(&BlockId::Hash(parent_hash))
		.map_err(|e| format!("{:?}", e))?;

	let mut last_difficulty = PowAux::read(client, &parent_hash)?.difficulty;
	let mut current = Some(parent_header.hash());

	let mut ret = Vec::new();
	while ret.len() < needed_block_count {
		if let Some(hash) = current {
			let header = client.header(BlockId::Hash(hash))
				.map_err(|e| format!("Client execution error: {:?}", e))?;
			if let Some(header) = header {
				let aux = PowAux::read(client, &hash)?;
				let timestamp = client.runtime_api()
					.timestamp(&BlockId::Hash(hash))
					.map_err(|e| format!("{:?}", e))?;

				ret.push((timestamp, aux.difficulty));
				last_difficulty = aux.difficulty;
				current = Some(header.parent_hash().clone());
			} else {
				let last_timestamp = ret.last().map(|v| v.0).unwrap_or(parent_timestamp);

				ret.push((last_timestamp.saturating_sub(BLOCK_TIME_SEC), last_difficulty));
				current = None;
			}
		} else {
			let last_timestamp = ret.last().map(|v| v.0).unwrap_or(parent_timestamp);

			ret.push((last_timestamp.saturating_sub(BLOCK_TIME_SEC), last_difficulty));
			current = None;
		}
	}

	ret.reverse();

	Ok(ret)
}

/// Move value linearly toward a goal
pub fn damp(actual: Difficulty, goal: Difficulty, damp_factor: Difficulty) -> Difficulty {
	(actual + (damp_factor - 1) * goal) / damp_factor
}

/// limit value to be within some factor from a goal
pub fn clamp(actual: Difficulty, goal: Difficulty, clamp_factor: Difficulty) -> Difficulty {
	max(goal / clamp_factor, min(actual, goal * clamp_factor))
}

pub struct RandomXAlgorithm<C> {
	client: Arc<C>,
}

impl<B: BlockT<Hash=H256>, C> PowAlgorithm<B> for RandomXAlgorithm<C> where
	C: HeaderBackend<B> + AuxStore + ProvideRuntimeApi,
	C::Api: TimestampApi<B, u64>,
{
	fn difficulty(&self, parent: &BlockId<B>) -> Result<Difficulty, String> {
		// Create vector of difficulty data running from earliest
		// to latest, and pad with simulated pre-genesis data to allow earlier
		// adjustment if there isn't enough window data length will be
		// DIFFICULTY_ADJUST_WINDOW + 1 (for initial block time bound)
		let diff_data = difficulty_data(self.client.as_ref(), parent)?;

		// Get the timestamp delta across the window
		let ts_delta = diff_data[DIFFICULTY_ADJUST_WINDOW as usize].0 - diff_data[0].0;

		// Get the difficulty sum of the last DIFFICULTY_ADJUST_WINDOW elements
		let diff_sum: Difficulty = diff_data
			.iter()
			.skip(1)
			.map(|v| v.1)
			.sum();

		// adjust time delta toward goal subject to dampening and clamping
		let adj_ts = clamp(
			damp(ts_delta as u128, BLOCK_TIME_WINDOW as u128, DIFFICULTY_DAMP_FACTOR),
			BLOCK_TIME_WINDOW as u128,
			CLAMP_FACTOR,
		);

		// minimum difficulty avoids getting stuck due to dampening
		let difficulty = max(MIN_DIFFICULTY, diff_sum * BLOCK_TIME_SEC as u128 / adj_ts);

		Ok(difficulty)
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

		if difficulty_from_hash(seal.work) < difficulty {
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

			if difficulty_from_hash(seal.work) >= difficulty {
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
