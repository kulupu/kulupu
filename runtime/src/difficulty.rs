use core::cmp::{min, max};
use pow_primitives::Difficulty;
use sr_primitives::traits::UniqueSaturatedInto;
use support::{decl_storage, decl_module, storage::StorageValue};
use codec::{Encode, Decode};
use kulupu_primitives::{
	DIFFICULTY_ADJUST_WINDOW, BLOCK_TIME_SEC, BLOCK_TIME_WINDOW,
	DIFFICULTY_DAMP_FACTOR, CLAMP_FACTOR, MIN_DIFFICULTY,
};

#[derive(Encode, Decode, Clone, Copy, Eq, PartialEq)]
pub struct DifficultyAndTimestamp<M> {
	pub difficulty: Difficulty,
	pub timestamp: M,
}

/// Move value linearly toward a goal
pub fn damp(actual: Difficulty, goal: Difficulty, damp_factor: Difficulty) -> Difficulty {
	(actual + (damp_factor - 1) * goal) / damp_factor
}

/// limit value to be within some factor from a goal
pub fn clamp(actual: Difficulty, goal: Difficulty, clamp_factor: Difficulty) -> Difficulty {
	max(goal / clamp_factor, min(actual, goal * clamp_factor))
}

pub trait Trait: timestamp::Trait { }

decl_storage! {
	trait Store for Module<T: Trait> as Difficulty {
		/// Past difficulties and timestamps, from earliest to latest.
		pub PastDifficultiesAndTimestamps
			get(past_difficulties_and_timestamps):
		[Option<DifficultyAndTimestamp<T::Moment>>; DIFFICULTY_ADJUST_WINDOW as usize]
			= [None; DIFFICULTY_ADJUST_WINDOW as usize];
		/// Initial difficulty.
		pub InitialDifficulty config(initial_difficulty): Difficulty;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn on_finalize(_n: T::BlockNumber) {
			let mut data = Self::past_difficulties_and_timestamps();

			for i in 1..data.len() {
				data[i - 1] = data[i];
			}

			data[data.len() - 1] = Some(DifficultyAndTimestamp {
				timestamp: <timestamp::Module<T>>::get(),
				difficulty: Self::difficulty(),
			});

			<PastDifficultiesAndTimestamps<T>>::put(data);
		}
	}
}

impl<T: Trait> Module<T> {
	/// Get target difficulty for the next block.
	pub fn difficulty() -> Difficulty {
		let data = Self::past_difficulties_and_timestamps();

		let mut ts_delta = 0;
		for i in 1..(DIFFICULTY_ADJUST_WINDOW as usize) {
			let prev: Option<u128> = data[i - 1].map(|d| d.timestamp.unique_saturated_into());
			let cur: Option<u128> = data[i].map(|d| d.timestamp.unique_saturated_into());

			let delta = match (prev, cur) {
				(Some(prev), Some(cur)) => cur.saturating_sub(prev),
				_ => BLOCK_TIME_SEC.into(),
			};
			ts_delta += delta;
		}

		let mut diff_sum = 0;
		for i in 0..(DIFFICULTY_ADJUST_WINDOW as usize) {
			let diff = match data[i].map(|d| d.difficulty) {
				Some(diff) => diff,
				None => InitialDifficulty::get(),
			};
			diff_sum += diff;
		}

		// adjust time delta toward goal subject to dampening and clamping
		let adj_ts = clamp(
			damp(ts_delta, BLOCK_TIME_WINDOW as u128, DIFFICULTY_DAMP_FACTOR),
			BLOCK_TIME_WINDOW as u128,
			CLAMP_FACTOR,
		);

		// minimum difficulty avoids getting stuck due to dampening
		let difficulty = max(MIN_DIFFICULTY, diff_sum * BLOCK_TIME_SEC as u128 / adj_ts);

		difficulty
	}
}
