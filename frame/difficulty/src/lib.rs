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

//! Difficulty adjustment module.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use sp_std::cmp::{min, max};
use sp_core::U256;
use sp_runtime::traits::UniqueSaturatedInto;
use frame_support::{decl_storage, decl_module};
use kulupu_primitives::{
	DIFFICULTY_ADJUST_WINDOW, BLOCK_TIME_MSEC, BLOCK_TIME_WINDOW_MSEC,
	DIFFICULTY_DAMP_FACTOR, CLAMP_FACTOR, MIN_DIFFICULTY, MAX_DIFFICULTY,
	Difficulty,
};

#[derive(Encode, Decode, Clone, Copy, Eq, PartialEq, Debug)]
pub struct DifficultyAndTimestamp<M> {
	pub difficulty: Difficulty,
	pub timestamp: M,
}

/// Move value linearly toward a goal
pub fn damp(actual: u128, goal: u128, damp_factor: u128) -> u128 {
	(actual + (damp_factor - 1) * goal) / damp_factor
}

/// limit value to be within some factor from a goal
pub fn clamp(actual: u128, goal: u128, clamp_factor: u128) -> u128 {
	max(goal / clamp_factor, min(actual, goal * clamp_factor))
}

pub trait Trait: pallet_timestamp::Trait { }

decl_storage! {
	trait Store for Module<T: Trait> as Difficulty {
		/// Past difficulties and timestamps, from earliest to latest.
		PastDifficultiesAndTimestamps:
		[Option<DifficultyAndTimestamp<T::Moment>>; 60]
			= [None; DIFFICULTY_ADJUST_WINDOW as usize];
		/// Current difficulty.
		pub CurrentDifficulty get(fn difficulty) build(|config: &GenesisConfig| {
			config.initial_difficulty
		}): Difficulty;
		/// Initial difficulty.
		pub InitialDifficulty config(initial_difficulty): Difficulty;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn on_finalize(_n: T::BlockNumber) {
			let mut data = PastDifficultiesAndTimestamps::<T>::get();

			for i in 1..data.len() {
				data[i - 1] = data[i];
			}

			data[data.len() - 1] = Some(DifficultyAndTimestamp {
				timestamp: <pallet_timestamp::Module<T>>::get(),
				difficulty: Self::difficulty(),
			});

			let mut ts_delta = 0;
			for i in 1..(DIFFICULTY_ADJUST_WINDOW as usize) {
				let prev: Option<u128> = data[i - 1].map(|d| d.timestamp.unique_saturated_into());
				let cur: Option<u128> = data[i].map(|d| d.timestamp.unique_saturated_into());

				let delta = match (prev, cur) {
					(Some(prev), Some(cur)) => cur.saturating_sub(prev),
					_ => BLOCK_TIME_MSEC.into(),
				};
				ts_delta += delta;
			}

			if ts_delta == 0 {
				ts_delta = 1;
			}

			let mut diff_sum = U256::zero();
			for i in 0..(DIFFICULTY_ADJUST_WINDOW as usize) {
				let diff = match data[i].map(|d| d.difficulty) {
					Some(diff) => diff,
					None => InitialDifficulty::get(),
				};
				diff_sum += diff;
			}

			if diff_sum < U256::from(MIN_DIFFICULTY) {
				diff_sum = U256::from(MIN_DIFFICULTY);
			}

			// adjust time delta toward goal subject to dampening and clamping
			let adj_ts = clamp(
				damp(ts_delta, BLOCK_TIME_WINDOW_MSEC as u128, DIFFICULTY_DAMP_FACTOR),
				BLOCK_TIME_WINDOW_MSEC as u128,
				CLAMP_FACTOR,
			);

			// minimum difficulty avoids getting stuck due to dampening
			let difficulty = min(U256::from(MAX_DIFFICULTY),
								 max(U256::from(MIN_DIFFICULTY),
									 diff_sum * U256::from(BLOCK_TIME_MSEC) / U256::from(adj_ts)));

			<PastDifficultiesAndTimestamps<T>>::put(data);
			<CurrentDifficulty>::put(difficulty);
		}
	}
}
