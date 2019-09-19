#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};

pub use substrate_primitives::U256;

#[derive(Default, Encode, Decode, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Difficulty(pub U256);

impl pow_primitives::TotalDifficulty for Difficulty {
	fn add(&mut self, other: Self) {
		let ret = self.0.saturating_add(other.0);
		*self = Difficulty(ret);
	}
}

/// Block interval, in seconds, the network will tune its next_target for.
pub const BLOCK_TIME_SEC: u64 = 60;
/// Block time interval in milliseconds.
pub const BLOCK_TIME_MSEC: u128 = BLOCK_TIME_SEC as u128 * 1000;

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
/// Average time span of the difficulty adjustment window in seconds.
pub const BLOCK_TIME_WINDOW_SEC: u64 = DIFFICULTY_ADJUST_WINDOW * BLOCK_TIME_SEC;
/// Average time span of the difficulty adjustment window in milliseconds.
pub const BLOCK_TIME_WINDOW_MSEC: u128 = DIFFICULTY_ADJUST_WINDOW as u128 * BLOCK_TIME_MSEC;
/// Clamp factor to use for difficulty adjustment
/// Limit value to within this factor of goal
pub const CLAMP_FACTOR: u128 = 2;
/// Dampening factor to use for difficulty adjustment
pub const DIFFICULTY_DAMP_FACTOR: u128 = 3;
/// Minimum difficulty, enforced in diff retargetting
/// avoids getting stuck when trying to increase difficulty subject to dampening
pub const MIN_DIFFICULTY: u128 = DIFFICULTY_DAMP_FACTOR;
/// Maximum difficulty.
pub const MAX_DIFFICULTY: u128 = u128::max_value();
