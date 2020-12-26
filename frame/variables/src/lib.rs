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

//! Variable storage pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_core::H256;
use frame_support::{
	decl_module, decl_storage, decl_event,
};
use frame_system::ensure_root;

pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as Eras {
		///	u32 storage values.
		pub U32s: map hasher(opaque_blake2_256) H256 => Option<u32>;
		/// u64 storage values.
		pub U64s: map hasher(opaque_blake2_256) H256 => Option<u64>;
	}
}

decl_event! {
	pub enum Event {
		/// U32 value changed.
		U32Changed(H256, u32),
		/// U64 value changed.
		U64Changed(H256, u64),
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 0]
		fn set_u32(origin, key: H256, value: u32) {
			ensure_root(origin)?;

			U32s::insert(key, value);
			Self::deposit_event(Event::U32Changed(key, value));
		}

		#[weight = 0]
		fn set_u64(origin, key: H256, value: u64) {
			ensure_root(origin)?;

			U64s::insert(key, value);
			Self::deposit_event(Event::U64Changed(key, value));
		}
	}
}
