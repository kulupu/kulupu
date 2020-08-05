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

//! Kulupu primitive constants and types.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_api::decl_runtime_apis;

pub type Difficulty = sp_core::U256;

/// Block interval, in seconds, the network will tune its next_target for.
pub const BLOCK_TIME_SEC: u64 = 60;
/// Block time interval in milliseconds.
pub const BLOCK_TIME: u64 = BLOCK_TIME_SEC * 1000;

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

/// Value of 1 KLP.
pub const DOLLARS: u128 = 1_000_000_000_000;
/// Value of cents relative to KLP.
pub const CENTS: u128 = DOLLARS / 100;
/// Value of millicents relative to KLP.
pub const MILLICENTS: u128 = CENTS / 1_000;
/// Value of microcents relative to RLP.
pub const MICROCENTS: u128 = MILLICENTS / 1_000;

pub const fn deposit(items: u32, bytes: u32) -> u128 {
	items as u128 * 2 * DOLLARS + (bytes as u128) * 10 * MILLICENTS
}

/// Block number of one hour.
pub const HOURS: u32 = 60;
/// Block number of one day.
pub const DAYS: u32 = 24 * HOURS;

pub const ALGORITHM_IDENTIFIER: [u8; 8] = *b"randomx1";

decl_runtime_apis! {
	pub trait AlgorithmApi {
		fn identifier() -> [u8; 8];
	}
}
