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

//! Set the block reward with a reward curve.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use sp_std::prelude::*;
use sp_runtime::traits::{Bounded, Zero};
use frame_support::{decl_storage, decl_module, decl_error, ensure, weights::Weight};
use frame_support::traits::{Currency, LockableCurrency, Get, EnsureOrigin};
use pallet_rewards::SetReward;

pub trait Trait: frame_system::Trait {
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
	end: BlockNumber,
	reward: Balance,
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Reward curve is empty.
		Empty,
		/// Reward curve is not sorted.
		NotSorted,
		/// Reward curve does not capture all blocks.
		NotComplete,
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
		fn on_initialize(n: T::BlockNumber) -> Weight {
			if n % T::UpdateFrequency::get() == Zero::zero() {
				let _ = RewardCurve::<T>::try_mutate(|curve| -> Result<(), ()>{
					ensure!(curve.len() > 1, ());
					ensure!(curve.first().expect("We checked curve was not empty; QED").end < n, ());
					curve.remove(0);
					let new_reward = curve.first().expect("We checked curve had at least two elements").reward;
					// Not much we can do if this fails.
					let _ = T::SetReward::set_reward(new_reward);
					Ok(())
				});
			}
			0
		}

		#[weight = 0]
		fn set_reward_curve(origin, curve: Vec<RewardPoint<T::BlockNumber, BalanceOf<T>>>) {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::ensure_sorted_and_complete(&curve)?;
			RewardCurve::<T>::put(curve);
		}
	}
}

impl<T: Trait> Module<T> {
	fn ensure_sorted_and_complete(curve: &[RewardPoint<T::BlockNumber, BalanceOf<T>>]) -> Result<(), Error<T>> {
		// Check curve is not empty
		ensure!(curve.len() > 0, Error::<T>::Empty);
		// Check curve is sorted
		ensure!(curve.windows(2).all(|w| w[0].end < w[1].end), Error::<T>::NotSorted);
		// Check curve goes all the way to the last block
		ensure!(curve.last().expect("We checked curve was not empty; QED").end == T::BlockNumber::max_value(), Error::<T>::NotComplete);
		Ok(())
	}
}
