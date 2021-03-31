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

#[cfg(test)]
mod tests;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod default_weights;

use codec::{Encode, Decode};
#[cfg(feature = "std")]
use serde::{Serialize, Deserialize};
use sp_std::{cmp, prelude::*};
use sp_runtime::{RuntimeDebug, traits::Hash};
use frame_support::{
	ensure, decl_storage, decl_module, decl_event, decl_error, storage::child,
	traits::{Currency, LockableCurrency, WithdrawReasons, LockIdentifier, Get},
	weights::Weight,
};
use frame_system::{ensure_root, ensure_signed};

pub trait WeightInfo {
	fn create_campaign() -> Weight;
	fn conclude_campaign() -> Weight;
	fn remove_expired_child_storage() -> Weight;
	fn lock() -> Weight;
	fn unlock() -> Weight;
}

pub type CampaignIdentifier = [u8; 4];

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CampaignInfo<T: Config> {
	end_block: T::BlockNumber,
	min_lock_end_block: T::BlockNumber,
	child_root: Option<Vec<u8>>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LockInfo<T: Config> {
	balance: BalanceOf<T>,
	end_block: T::BlockNumber,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ChildLockData<T: Config> {
	balance: BalanceOf<T>,
	end_block: T::BlockNumber,
	payload: Option<Vec<u8>>,
}

pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// An implementation of on-chain currency.
	type Currency: LockableCurrency<Self::AccountId>;

	/// Payload length limit.
	type PayloadLenLimit: Get<u32>;
	/// Max number of storage keys to remove per extrinsic call.
	type RemoveKeysLimit: Get<u32>;

	/// Weights for this pallet.
	type WeightInfo: WeightInfo;
}

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

decl_storage! {
	trait Store for Module<T: Config> as Eras {
		Campaigns get(fn campaigns): map hasher(blake2_128_concat) CampaignIdentifier => Option<CampaignInfo<T>>;
		Locks get(fn locks): double_map hasher(blake2_128_concat) CampaignIdentifier, hasher(blake2_128_concat) T::AccountId => Option<LockInfo<T>>;
	}
}

decl_error! {
	pub enum Error for Module<T: Config> {
		/// The given campaign name was used in the past.
		CampaignIdentifierUsedInPast,
		/// The given campaign trying to create has already existed.
		CampaignAlreadyExists,
		/// Campaign end block must be in the future.
		CampaignEndInThePast,
		/// Campaign lock block must be after campaign end block.
		CampaignLockEndBeforeCampaignEnd,
		/// Not enough balance.
		NotEnoughBalance,
		/// Payload over length limit.
		PayloadOverLenLimit,
		/// Campaign does not exist.
		CampaignNotExists,
		/// Campaign has already expired.
		CampaignAlreadyExpired,
		/// Attempt to lock less than what is already locked.
		AttemptedToLockLess,
		/// Invalid lock end block.
		InvalidLockEndBlock,
	}
}

decl_event! {
	pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
		CampaignCreated(CampaignIdentifier),
		CampaignConcluded(CampaignIdentifier, Vec<u8>),
		ChildStorageRemoved(CampaignIdentifier),
		ChildStoragePartiallyRemoved(CampaignIdentifier),
		Locked(CampaignIdentifier, AccountId),
		Unlocked(CampaignIdentifier, AccountId),
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = T::WeightInfo::create_campaign()]
		fn create_campaign(origin, identifier: CampaignIdentifier, end_block: T::BlockNumber, min_lock_end_block: T::BlockNumber) {
			ensure_root(origin)?;

			let campaign_name_used_in_past = Locks::<T>::iter_prefix_values(identifier).next().is_some();
			ensure!(!campaign_name_used_in_past, Error::<T>::CampaignIdentifierUsedInPast);

			ensure!(!Campaigns::<T>::contains_key(&identifier), Error::<T>::CampaignAlreadyExists);

			let current_number = frame_system::Pallet::<T>::block_number();
			ensure!(end_block > current_number, Error::<T>::CampaignEndInThePast);
			ensure!(min_lock_end_block > end_block, Error::<T>::CampaignLockEndBeforeCampaignEnd);

			Campaigns::<T>::insert(identifier, CampaignInfo { end_block, min_lock_end_block, child_root: None });
			Self::deposit_event(Event::<T>::CampaignCreated(identifier));
		}

		#[weight = T::WeightInfo::conclude_campaign()]
		fn conclude_campaign(origin, identifier: CampaignIdentifier) {
			ensure_signed(origin)?;

			Campaigns::<T>::mutate(&identifier, |info| {
				if let Some(ref mut info) = info {
					if info.child_root.is_none() {
						let current_number = frame_system::Pallet::<T>::block_number();
						if current_number > info.end_block {
							let child_root = Self::child_root(&identifier);
							info.child_root = Some(child_root.clone());
							Self::deposit_event(Event::<T>::CampaignConcluded(identifier, child_root));
						}
					}
				}
			});
		}

		#[weight = T::WeightInfo::remove_expired_child_storage()]
		fn remove_expired_child_storage(origin, identifier: CampaignIdentifier) {
			ensure_signed(origin)?;

			let info = Campaigns::<T>::get(&identifier);
			if let Some(info) = info {
				let current_number = frame_system::Pallet::<T>::block_number();
				if current_number > info.end_block && info.child_root.is_some() {
					match Self::child_kill(&identifier) {
						child::KillChildStorageResult::AllRemoved(_) => {
							Self::deposit_event(Event::<T>::ChildStorageRemoved(identifier));
						},
						child::KillChildStorageResult::SomeRemaining(_) => {
							Self::deposit_event(Event::<T>::ChildStoragePartiallyRemoved(identifier));
						}
					}
				}
			}
		}

		#[weight = T::WeightInfo::lock()]
		fn lock(origin, amount: BalanceOf<T>, identifier: CampaignIdentifier, lock_end_block: T::BlockNumber, payload: Option<Vec<u8>>) {
			let account_id = ensure_signed(origin)?;

			ensure!(T::Currency::free_balance(&account_id) >= amount, Error::<T>::NotEnoughBalance);

			if let Some(ref payload) = payload {
				ensure!(payload.len() <= T::PayloadLenLimit::get() as usize, Error::<T>::PayloadOverLenLimit);
			}
			let campaign_info = Campaigns::<T>::get(&identifier).ok_or(Error::<T>::CampaignNotExists)?;

			let current_number = frame_system::Pallet::<T>::block_number();
			ensure!(current_number <= campaign_info.end_block, Error::<T>::CampaignAlreadyExpired);
			ensure!(lock_end_block > campaign_info.min_lock_end_block, Error::<T>::InvalidLockEndBlock);

			let lock_info = match Locks::<T>::get(&identifier, &account_id) {
				Some(mut lock_info) => {
					ensure!(amount >= lock_info.balance, Error::<T>::AttemptedToLockLess);
					ensure!(lock_end_block >= lock_info.end_block, Error::<T>::AttemptedToLockLess);

					lock_info.balance = cmp::max(amount, lock_info.balance);
					lock_info.end_block = cmp::max(lock_end_block, lock_info.end_block);
					lock_info
				},
				None => LockInfo { balance: amount, end_block: lock_end_block },
			};

			let lock_identifier = Self::lock_identifier(identifier);
			T::Currency::extend_lock(lock_identifier, &account_id, amount, WithdrawReasons::all());

			let child_lock_data = ChildLockData { balance: lock_info.balance, end_block: lock_info.end_block, payload };
			Self::child_data_put(&identifier, &account_id, &child_lock_data);

			Locks::<T>::insert(identifier, account_id.clone(), lock_info);
			Self::deposit_event(Event::<T>::Locked(identifier, account_id));
		}

		#[weight = T::WeightInfo::unlock()]
		fn unlock(origin, identifier: CampaignIdentifier) {
			let account_id = ensure_signed(origin)?;

			let info = Locks::<T>::get(&identifier, &account_id);
			if let Some(info) = info {
				let current_number = frame_system::Pallet::<T>::block_number();
				if current_number > info.end_block {
					Locks::<T>::remove(identifier, account_id.clone());

					let lock_identifier = Self::lock_identifier(identifier);
					T::Currency::remove_lock(lock_identifier, &account_id);

					Self::deposit_event(Event::<T>::Unlocked(identifier, account_id));
				}
			}
		}
	}
}

impl<T: Config> Module<T> {
	pub fn lock_identifier(identifier: CampaignIdentifier) -> LockIdentifier {
		[b'd', b'r', b'o', b'p', identifier[0], identifier[1], identifier[2], identifier[3]]
	}

	pub fn child_info(identifier: &CampaignIdentifier) -> child::ChildInfo {
		let mut buf = Vec::new();
		buf.extend_from_slice(b"lockdrop:");
		buf.extend_from_slice(identifier);
		child::ChildInfo::new_default(T::Hashing::hash(&buf[..]).as_ref())
	}

	fn child_data_put(identifier: &CampaignIdentifier, account_id: &T::AccountId, data: &ChildLockData<T>) {
		account_id.using_encoded(|account_id| child::put(&Self::child_info(identifier), &account_id, &data))
	}

	pub fn child_data_get(identifier: &CampaignIdentifier, account_id: &T::AccountId) -> Option<ChildLockData<T>> {
		account_id.using_encoded(|account_id| child::get(&Self::child_info(identifier), &account_id))
	}

	pub fn child_root(identifier: &CampaignIdentifier) -> Vec<u8> {
		child::root(&Self::child_info(identifier))
	}

	fn child_kill(identifier: &CampaignIdentifier) -> child::KillChildStorageResult {
		child::kill_storage(&Self::child_info(identifier), Some(T::RemoveKeysLimit::get()))
	}
}
