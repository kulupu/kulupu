// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2020 Wei Tang.
// Copyright (c) 2020 Shawn Tabrizi.
//
// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Kulupu. If not, see <http://www.gnu.org/licenses/>.

//! Weights for rewards
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0
//! DATE: 2020-10-19, STEPS: [50], REPEAT: 20, LOW RANGE: [], HIGH RANGE: []

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{Weight, constants::RocksDbWeight as DbWeight};

impl crate::WeightInfo for () {
	fn on_initialize() -> Weight {
		(48_264_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn on_finalize() -> Weight {
		(164_338_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn note_author_prefs() -> Weight {
		(7_001_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_reward() -> Weight {
		(24_184_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_taxation() -> Weight {
		(22_786_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_curve(l: u32, ) -> Weight {
		(19_503_000 as Weight)
			.saturating_add((12_000 as Weight).saturating_mul(l as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn unlock() -> Weight {
		(58_289_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}
