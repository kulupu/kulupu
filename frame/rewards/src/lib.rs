// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2019-2020 Wei Tang.
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

//! Reward handling module for Kulupu.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod default_weights;
mod migrations;

use codec::{Encode, Decode};
use sp_std::{result, ops::Bound::Included, prelude::*, collections::btree_map::BTreeMap};
use sp_runtime::{RuntimeDebug, Perbill, traits::{Saturating, Zero}};
use sp_inherents::{InherentIdentifier, InherentData, ProvideInherent, IsFatalError};
use sp_consensus_pow::POW_ENGINE_ID;
#[cfg(feature = "std")]
use sp_inherents::ProvideInherentData;
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	traits::{Get, Currency, LockIdentifier, LockableCurrency, WithdrawReasons},
	weights::Weight,
};
use frame_system::{ensure_root, ensure_signed};

pub struct LockBounds {
	pub period_max: u16,
	pub period_min: u16,
	pub divide_max: u16,
	pub divide_min: u16,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, Debug)]
pub struct LockParameters {
	pub period: u16,
	pub divide: u16,
}

/// Trait for generating reward locks.
pub trait GenerateRewardLocks<T: Config> {
	/// Generate reward locks.
	fn generate_reward_locks(
		current_block: T::BlockNumber,
		total_reward: BalanceOf<T>,
		lock_parameters: Option<LockParameters>,
	) -> BTreeMap<T::BlockNumber, BalanceOf<T>>;

	fn max_locks(lock_bounds: LockBounds) -> u32;
}

impl<T: Config> GenerateRewardLocks<T> for () {
	fn generate_reward_locks(
		_current_block: T::BlockNumber,
		_total_reward: BalanceOf<T>,
		_lock_parameters: Option<LockParameters>,
	) -> BTreeMap<T::BlockNumber, BalanceOf<T>> {
		Default::default()
	}

	fn max_locks(_lock_bounds: LockBounds) -> u32 {
		0
	}
}

pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn on_finalize() -> Weight;
	fn unlock() -> Weight;
	fn set_schedule() -> Weight;
	fn set_lock_params() -> Weight;
}

/// Config for rewards.
pub trait Config: frame_system::Config {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	/// An implementation of on-chain currency.
	type Currency: LockableCurrency<Self::AccountId>;
	/// Donation destination.
	type DonationDestination: Get<Self::AccountId>;
	/// Generate reward locks.
	type GenerateRewardLocks: GenerateRewardLocks<Self>;
	/// Weights for this pallet.
	type WeightInfo: WeightInfo;
	/// Lock Parameters Bounds.
	type LockParametersBounds: Get<LockBounds>;
}

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

decl_error! {
	pub enum Error for Module<T: Config> {
		/// Reward set is too low.
		RewardTooLow,
		/// Mint value is too low.
		MintTooLow,
		/// Reward curve is not sorted.
		NotSorted,
		/// Lock parameters are out of bounds.
		LockParamsOutOfBounds,
		/// Lock period is not a mutiple of the divide.
		LockPeriodNotDivisible,
	}
}

decl_storage! {
	trait Store for Module<T: Config> as Rewards {
		/// Current block author.
		Author get(fn author): Option<T::AccountId>;

		/// Current block reward for miner.
		Reward get(fn reward) config(): BalanceOf<T>;
		/// Pending reward locks.
		RewardLocks get(fn reward_locks): map hasher(twox_64_concat) T::AccountId => BTreeMap<T::BlockNumber, BalanceOf<T>>;
		/// Reward changes planned in the future.
		RewardChanges get(fn reward_changes): BTreeMap<T::BlockNumber, BalanceOf<T>>;

		/// Current block mints.
		Mints get(fn mints) config(): BTreeMap<T::AccountId, BalanceOf<T>>;
		/// Mint changes planned in the future.
		MintChanges get(fn mint_changes): BTreeMap<T::BlockNumber, BTreeMap<T::AccountId, BalanceOf<T>>>;

		/// Lock parameters (period and divide).
		LockParams get(fn lock_params): Option<LockParameters>;

		StorageVersion build(|_| migrations::StorageVersion::V1): migrations::StorageVersion;
	}
}

decl_event! {
	pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId, Balance = BalanceOf<T> {
		/// A new schedule has been set.
		ScheduleSet,
		/// Reward has been sent.
		Rewarded(AccountId, Balance),
		/// Reward has been changed.
		RewardChanged(Balance),
		/// Mint has been sent.
		Minted(AccountId, Balance),
		/// Mint has been changed.
		MintsChanged(BTreeMap<AccountId, Balance>),
		/// Lock Parameters have been changed.
		LockParamsChanged(LockParameters),
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		fn on_initialize(now: T::BlockNumber) -> Weight {
			let author = frame_system::Module::<T>::digest()
				.logs
				.iter()
				.filter_map(|s| s.as_pre_runtime())
				.filter_map(|(id, mut data)| if id == POW_ENGINE_ID {
					T::AccountId::decode(&mut data).ok()
				} else {
					None
				})
				.next();

			if let Some(author) = author {
				<Self as Store>::Author::put(author);
			}

			RewardChanges::<T>::mutate(|reward_changes| {
				let mut removing = Vec::new();

				for (block_number, reward) in reward_changes.range((Included(Zero::zero()), Included(now))) {
					Reward::<T>::set(*reward);
					removing.push(*block_number);

					Self::deposit_event(Event::<T>::RewardChanged(*reward));
				}

				for block_number in removing {
					reward_changes.remove(&block_number);
				}
			});

			MintChanges::<T>::mutate(|mint_changes| {
				let mut removing = Vec::new();

				for (block_number, mints) in mint_changes.range((Included(Zero::zero()), Included(now))) {
					Mints::<T>::set(mints.clone());
					removing.push(*block_number);

					Self::deposit_event(Event::<T>::MintsChanged(mints.clone()));
				}

				for block_number in removing {
					mint_changes.remove(&block_number);
				}
			});

			T::WeightInfo::on_initialize().saturating_add(T::WeightInfo::on_finalize())
		}

		fn on_finalize(now: T::BlockNumber) {
			if let Some(author) = <Self as Store>::Author::get() {
				let reward = Reward::<T>::get();
				Self::do_reward(&author, reward, now);
			}

			let mints = Mints::<T>::get();
			Self::do_mints(&mints);

			<Self as Store>::Author::kill();
		}

		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			let version = StorageVersion::get();
			let new_version = version.migrate::<T>();
			StorageVersion::put(new_version);

			0
		}

		#[weight = T::WeightInfo::set_schedule()]
		fn set_schedule(
			origin,
			reward: BalanceOf<T>,
			mints: BTreeMap<T::AccountId, BalanceOf<T>>,
			reward_changes: BTreeMap<T::BlockNumber, BalanceOf<T>>,
			mint_changes: BTreeMap<T::BlockNumber, BTreeMap<T::AccountId, BalanceOf<T>>>,
		) {
			ensure_root(origin)?;

			ensure!(reward >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);
			for (_, mint) in &mints {
				ensure!(*mint >= T::Currency::minimum_balance(), Error::<T>::MintTooLow);
			}
			for (_, reward_change) in &reward_changes {
				ensure!(*reward_change >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);
			}
			for (_, mint_change) in &mint_changes {
				for (_, mint) in mint_change {
					ensure!(*mint >= T::Currency::minimum_balance(), Error::<T>::MintTooLow);
				}
			}

			Reward::<T>::put(reward);
			Self::deposit_event(RawEvent::RewardChanged(reward));

			Mints::<T>::put(mints.clone());
			Self::deposit_event(RawEvent::MintsChanged(mints));

			RewardChanges::<T>::put(reward_changes);
			MintChanges::<T>::put(mint_changes);
			Self::deposit_event(RawEvent::ScheduleSet);
		}

		#[weight = T::WeightInfo::set_lock_params()]
		fn set_lock_params(origin, lock_params: LockParameters) {
			ensure_root(origin)?;

			let bounds = T::LockParametersBounds::get();
			ensure!((bounds.period_min..=bounds.period_max).contains(&lock_params.period) &&
				(bounds.divide_min..=bounds.divide_max).contains(&lock_params.divide), Error::<T>::LockParamsOutOfBounds);
			ensure!(lock_params.period % lock_params.divide == 0, Error::<T>::LockPeriodNotDivisible);

			LockParams::put(lock_params);
			Self::deposit_event(RawEvent::LockParamsChanged(lock_params));
		}

		/// Unlock any vested rewards for `target` account.
		#[weight = T::WeightInfo::unlock()]
		fn unlock(origin, target: T::AccountId) {
			ensure_signed(origin)?;

			let locks = Self::reward_locks(&target);
			let current_number = frame_system::Module::<T>::block_number();
			Self::do_update_reward_locks(&target, locks, current_number);
		}
	}
}

const REWARDS_ID: LockIdentifier = *b"rewards ";

impl<T: Config> Module<T> {
	fn do_reward(author: &T::AccountId, reward: BalanceOf<T>, when: T::BlockNumber) {
		let miner_total = reward;

		let miner_reward_locks = T::GenerateRewardLocks::generate_reward_locks(
			when,
			miner_total,
			LockParams::get(),
		);

		drop(T::Currency::deposit_creating(&author, miner_total));

		if miner_reward_locks.len() > 0 {
			let mut locks = Self::reward_locks(&author);

			for (new_lock_number, new_lock_balance) in miner_reward_locks {
				let old_balance = *locks.get(&new_lock_number).unwrap_or(&BalanceOf::<T>::default());
				let new_balance = old_balance.saturating_add(new_lock_balance);
				locks.insert(new_lock_number, new_balance);
			}

			Self::do_update_reward_locks(&author, locks, when);
		}
	}

	fn do_update_reward_locks(
		author: &T::AccountId,
		mut locks: BTreeMap<T::BlockNumber, BalanceOf<T>>,
		current_number: T::BlockNumber
	) {
		let mut expired = Vec::new();
		let mut total_locked: BalanceOf<T> = Zero::zero();

		for (block_number, locked_balance) in &locks {
			if block_number <= &current_number {
				expired.push(*block_number);
			} else {
				total_locked = total_locked.saturating_add(*locked_balance);
			}
		}

		for block_number in expired {
			locks.remove(&block_number);
		}

		T::Currency::set_lock(
			REWARDS_ID,
			&author,
			total_locked,
			WithdrawReasons::except(WithdrawReasons::TRANSACTION_PAYMENT),
		);

		<Self as Store>::RewardLocks::insert(author, locks);
	}

	fn do_mints(
		mints: &BTreeMap<T::AccountId, BalanceOf<T>>,
	) {
		for (destination, mint) in mints {
			drop(T::Currency::deposit_creating(&destination, *mint));
		}
	}
}

pub const INHERENT_IDENTIFIER_V0: InherentIdentifier = *b"rewards_";
pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"rewards1";

#[derive(Encode, Decode, RuntimeDebug)]
pub enum InherentError { }

impl IsFatalError for InherentError {
	fn is_fatal_error(&self) -> bool {
		match *self { }
	}
}

impl InherentError {
	/// Try to create an instance ouf of the given identifier and data.
	#[cfg(feature = "std")]
	pub fn try_from(id: &InherentIdentifier, data: &[u8]) -> Option<Self> {
		if id == &INHERENT_IDENTIFIER {
			<InherentError as codec::Decode>::decode(&mut &data[..]).ok()
		} else {
			None
		}
	}
}

#[cfg(feature = "std")]
pub struct InherentDataProviderV0(pub Vec<u8>);

#[cfg(feature = "std")]
impl ProvideInherentData for InherentDataProviderV0 {
	fn inherent_identifier(&self) -> &'static InherentIdentifier {
		&INHERENT_IDENTIFIER_V0
	}

	fn provide_inherent_data(
		&self,
		inherent_data: &mut InherentData
	) -> Result<(), sp_inherents::Error> {
		inherent_data.put_data(INHERENT_IDENTIFIER_V0, &self.0)
	}

	fn error_to_string(&self, error: &[u8]) -> Option<String> {
		InherentError::try_from(&INHERENT_IDENTIFIER_V0, error).map(|e| format!("{:?}", e))
	}
}

pub type InherentType = (Vec<u8>, Perbill);

#[cfg(feature = "std")]
pub struct InherentDataProvider(pub InherentType);

#[cfg(feature = "std")]
impl ProvideInherentData for InherentDataProvider {
	fn inherent_identifier(&self) -> &'static InherentIdentifier {
		&INHERENT_IDENTIFIER
	}

	fn provide_inherent_data(
		&self,
		inherent_data: &mut InherentData
	) -> Result<(), sp_inherents::Error> {
		inherent_data.put_data(INHERENT_IDENTIFIER, &self.0)
	}

	fn error_to_string(&self, error: &[u8]) -> Option<String> {
		InherentError::try_from(&INHERENT_IDENTIFIER, error).map(|e| format!("{:?}", e))
	}
}

impl<T: Config> ProvideInherent for Module<T> {
	type Call = Call<T>;
	type Error = InherentError;
	const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;

	fn create_inherent(_data: &InherentData) -> Option<Self::Call> {
		None
	}

	fn check_inherent(_call: &Self::Call, _data: &InherentData) -> result::Result<(), Self::Error> {
		Ok(())
	}
}
