// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2020 Wei Tang.
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

//! Era information recording.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
#[cfg(feature = "std")]
use serde::{Serialize, Deserialize};
use sp_std::prelude::*;
use frame_support::{decl_storage, decl_module};

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode)]
pub struct Era<H> {
	/// Genesis block hash of the era.
	pub genesis_block_hash: H,
	/// Final block hash.
	pub final_block_hash: H,
	/// Final state root.
	pub final_state_root: H,
}

pub trait Trait: frame_system::Trait { }

decl_storage! {
	trait Store for Module<T: Trait> as Eras {
		/// Past eras.
		pub PastEras get(fn past_eras) config(past_eras): Vec<Era<T::Hash>>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin { }
}
