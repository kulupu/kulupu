// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
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

//! Set the block reward with a reward curve.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use sp_std::prelude::*;
use sp_runtime::traits::Zero;
use frame_support::{decl_storage, decl_module, decl_error, decl_event, ensure, weights::Weight};
use frame_support::traits::{Currency, LockableCurrency, Get, EnsureOrigin};
use pallet_rewards::SetReward;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;
	/// An implementation of on-chain currency.
	type Currency: LockableCurrency<Self::AccountId>;
	/// How often to check to update the reward curve.
	type UpdateFrequency: Get<Self::BlockNumber>;
	/// The origin that can set the reward curve.
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
	/// Handler for setting a new reward.
	type SetReward: SetReward<BalanceOf<Self>>;
}

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
pub struct RewardPoint<BlockNumber, Balance> {
	start: BlockNumber,
	reward: Balance,
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Reward curve is not sorted.
		NotSorted,
	}
}

decl_event! {
	pub enum Event {
		/// A new reward curve was set.
		RewardCurveSet,
		/// Reward updated successfully.
		UpdateSuccessful,
		/// Reward failed to update.
		UpdateFailed,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Eras {
		/// Reward Curve for this chain
		pub RewardCurve get(fn reward_curve) config(): Vec<RewardPoint<T::BlockNumber, BalanceOf<T>>>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let mut weight: Weight = 0;
			if current_block % T::UpdateFrequency::get() == Zero::zero() {
				let _ = RewardCurve::<T>::try_mutate(|curve| -> Result<(), ()> {
					weight = weight.saturating_add(T::DbWeight::get().reads(1));
					ensure!(!curve.is_empty(), ());
					// We checked above that curve is not empty, so this will never panic.
					let point = curve.remove(0);
					ensure!(point.start <= current_block, ());
					let new_reward = point.reward;
					// Not much we can do if this fails.
					let result = T::SetReward::set_reward(new_reward);
					match result {
						Ok(..) => Self::deposit_event(Event::UpdateSuccessful),
						Err(..) => Self::deposit_event(Event::UpdateFailed),
					}
					weight = weight.saturating_add(T::DbWeight::get().writes(1));
					Ok(())
				});
			}
			weight
		}

		#[weight = T::DbWeight::get().writes(1)]
		fn set_reward_curve(origin, curve: Vec<RewardPoint<T::BlockNumber, BalanceOf<T>>>) {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::ensure_sorted(&curve)?;
			RewardCurve::<T>::put(curve);
			Self::deposit_event(Event::RewardCurveSet);
		}
	}
}

impl<T: Trait> Module<T> {
	fn ensure_sorted(curve: &[RewardPoint<T::BlockNumber, BalanceOf<T>>]) -> Result<(), Error<T>> {
		// Check curve is sorted
		ensure!(curve.windows(2).all(|w| w[0].start < w[1].start), Error::<T>::NotSorted);
		Ok(())
	}
}
