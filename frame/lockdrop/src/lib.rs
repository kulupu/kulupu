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
use sp_std::{cmp, prelude::*};
use sp_runtime::RuntimeDebug;
use frame_support::{
    ensure, decl_storage, decl_module, decl_event, decl_error, 
    traits::{Currency, LockableCurrency, WithdrawReasons, LockIdentifier},
};
use frame_system::{ensure_root, ensure_signed};

pub const MAX_PAYLOAD_LEN: usize = 32;

pub type CampaignIdentifier = [u8; 4];

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CampaignInfo<T: Config> {
    end_block: T::BlockNumber,
    min_lock_end_block: T::BlockNumber,
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
        /// The given campaign trying to create has already existed.
        CampaignAlreadyExists,
        /// Campaign end block must be in the future.
        CampaignEndInThePast,
        /// Campaign lock block must be after campaign end block.
        CampaignLockEndBeforeCampaignEnd,
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
        CampaignExpired(CampaignIdentifier),
        Locked(CampaignIdentifier, AccountId),
        Unlocked(CampaignIdentifier, AccountId),
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin { 
        type Error = Error<T>;

		fn deposit_event() = default;

        #[weight = 0]
        fn create_campaign(origin, identifier: CampaignIdentifier, end_block: T::BlockNumber, min_lock_end_block: T::BlockNumber) {
            ensure_root(origin)?;

            ensure!(!Campaigns::<T>::contains_key(&identifier), Error::<T>::CampaignAlreadyExists);
            
            let current_number = frame_system::Module::<T>::block_number();
            ensure!(end_block > current_number, Error::<T>::CampaignEndInThePast);
            ensure!(min_lock_end_block > end_block, Error::<T>::CampaignLockEndBeforeCampaignEnd);
            
            Campaigns::<T>::insert(identifier, CampaignInfo { end_block, min_lock_end_block });
            Self::deposit_event(Event::<T>::CampaignCreated(identifier));
        }

        #[weight = 0]
        fn remove_expired_campaign(origin, identifier: CampaignIdentifier) {
            ensure_signed(origin)?;

            let info = Campaigns::<T>::get(&identifier);
            if let Some(info) = info {
                let current_number = frame_system::Module::<T>::block_number();
                if current_number > info.end_block {
                    Campaigns::<T>::remove(identifier);
                    Self::deposit_event(Event::<T>::CampaignExpired(identifier));
                }
            }
        }

        #[weight = 0]
        fn lock(origin, amount: BalanceOf<T>, identifier: CampaignIdentifier, lock_end_block: T::BlockNumber) {
            let account_id = ensure_signed(origin)?;
            let campaign_info = Campaigns::<T>::get(&identifier).ok_or(Error::<T>::CampaignNotExists)?;
            
            let current_number = frame_system::Module::<T>::block_number();
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
            
            Locks::<T>::insert(identifier, account_id.clone(), lock_info);
            Self::deposit_event(Event::<T>::Locked(identifier, account_id));
        }

        #[weight = 0]
        fn unlock(origin, identifier: CampaignIdentifier) {
            let account_id = ensure_signed(origin)?;

            let info = Locks::<T>::get(&identifier, &account_id);
            if let Some(info) = info {
                let current_number = frame_system::Module::<T>::block_number();
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
    fn lock_identifier(identifier: CampaignIdentifier) -> LockIdentifier {
        [b'd', b'r', b'o', b'p', identifier[0], identifier[1], identifier[2], identifier[3]]
    }
}