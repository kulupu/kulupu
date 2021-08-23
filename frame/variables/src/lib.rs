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

use frame_support::{decl_event, decl_module, decl_storage};
use frame_system::ensure_root;
use sp_std::vec::Vec;

pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
	trait Store for Module<T: Config> as Eras {
		///	u32 storage values.
		pub U32s: map hasher(blake2_128_concat) Vec<u8> => Option<u32>;
		/// u64 storage values.
		pub U64s: map hasher(blake2_128_concat) Vec<u8> => Option<u64>;
		/// U128 storage values.
		pub U128s: map hasher(blake2_128_concat) Vec<u8> => Option<u128>;
		/// Bool storage values.
		pub Bools: map hasher(blake2_128_concat) Vec<u8> => Option<bool>;
	}
}

decl_event! {
	pub enum Event {
		/// U32 value changed.
		U32Changed(Vec<u8>, u32),
		/// U64 value changed.
		U64Changed(Vec<u8>, u64),
		/// U128 value changed.
		U128Changed(Vec<u8>, u128),
		/// Bool value changed.
		BoolChanged(Vec<u8>, bool),
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 0]
		fn set_u32(origin, key: Vec<u8>, value: u32) {
			ensure_root(origin)?;

			U32s::insert(key.clone(), value);
			Self::deposit_event(Event::U32Changed(key, value));
		}

		#[weight = 0]
		fn set_u64(origin, key: Vec<u8>, value: u64) {
			ensure_root(origin)?;

			U64s::insert(key.clone(), value);
			Self::deposit_event(Event::U64Changed(key, value));
		}

		#[weight = 0]
		fn set_u128(origin, key: Vec<u8>, value: u128) {
			ensure_root(origin)?;

			U128s::insert(key.clone(), value);
			Self::deposit_event(Event::U128Changed(key, value));
		}

		#[weight = 0]
		fn set_bool(origin, key: Vec<u8>, value: bool) {
			ensure_root(origin)?;

			Bools::insert(key.clone(), value);
			Self::deposit_event(Event::BoolChanged(key, value));
		}
	}
}
