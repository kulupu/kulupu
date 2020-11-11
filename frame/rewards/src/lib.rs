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

use codec::{Encode, Decode};
#[cfg(feature = "std")]
use serde::{Serialize, Deserialize};
use sp_std::{result, cmp::min, prelude::*, collections::btree_map::BTreeMap};
use sp_runtime::{RuntimeDebug, Perbill, traits::{Saturating, Zero}};
use sp_inherents::{InherentIdentifier, InherentData, ProvideInherent, IsFatalError};
use sp_consensus_pow::POW_ENGINE_ID;
#[cfg(feature = "std")]
use sp_inherents::ProvideInherentData;
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	traits::{Get, Currency, LockIdentifier, LockableCurrency, WithdrawReasons},
	weights::{DispatchClass, Weight},
};
use frame_system::{ensure_none, ensure_root, ensure_signed};

/// Trait for generating reward locks.
pub trait GenerateRewardLocks<T: Trait> {
	/// Generate reward locks.
	fn generate_reward_locks(
		current_block: T::BlockNumber,
		total_reward: BalanceOf<T>,
	) -> BTreeMap<T::BlockNumber, BalanceOf<T>>;

	fn max_locks() -> u32;
}

impl<T: Trait> GenerateRewardLocks<T> for () {
	fn generate_reward_locks(
		_current_block: T::BlockNumber,
		_total_reward: BalanceOf<T>,
	) -> BTreeMap<T::BlockNumber, BalanceOf<T>> {
		Default::default()
	}

	fn max_locks() -> u32 {
		0
	}
}

pub trait WeightInfo {
	fn on_initialize() -> Weight;
	fn on_finalize() -> Weight;
	fn note_author_prefs() -> Weight;
	fn set_reward() -> Weight;
	fn set_taxation() -> Weight;
	fn unlock() -> Weight;
	fn set_curve(_l: u32, ) -> Weight;
}

/// Trait for rewards.
pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// An implementation of on-chain currency.
	type Currency: LockableCurrency<Self::AccountId>;
	/// Donation destination.
	type DonationDestination: Get<Self::AccountId>;
	/// Generate reward locks.
	type GenerateRewardLocks: GenerateRewardLocks<Self>;
	/// How often to check to update the reward curve.
	type UpdateFrequency: Get<Self::BlockNumber>;
	/// Weights for this pallet.
	type WeightInfo: WeightInfo;
}

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

#[derive(Encode, Decode, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CurvePoint<BlockNumber, Balance> {
	start: BlockNumber,
	reward: Balance,
	taxation: Perbill,
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Reward set is too low.
		RewardTooLow,
		/// Author preferences already set.
		AuthorPrefsAlreadySet,
		/// Reward curve is not sorted.
		NotSorted,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Current block author.
		Author get(fn author): Option<T::AccountId>;
		/// Current block donation.
		AuthorDonation get(fn author_donation): Option<Perbill>;
		/// Current block reward.
		Reward get(fn reward) config(): BalanceOf<T>;
		/// Taxation rate.
		Taxation get(fn taxation) config(): Perbill;
		/// Pending reward locks.
		RewardLocks get(fn reward_locks):
			map hasher(twox_64_concat) T::AccountId => BTreeMap<T::BlockNumber, BalanceOf<T>>;
		/// Curve for this chain, sorted in reverse by block number.
		Curve get(fn curve) config(): Vec<CurvePoint<T::BlockNumber, BalanceOf<T>>>;
	}
}

decl_event! {
	pub enum Event<T> where Balance = BalanceOf<T> {
		/// Block reward has changed. [reward]
		RewardChanged(Balance),
		/// Block taxation has changed. [taxation]
		TaxationChanged(Perbill),
		/// A new curve has been set.
		CurveSet,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
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

			if now % T::UpdateFrequency::get() == Zero::zero() {
				Curve::<T>::mutate(|curve| {
					if let Some(point) = curve.pop() {
						if point.start <= now {
							Reward::<T>::mutate(|reward| {
								if reward != &point.reward {
									*reward = point.reward;
									Self::deposit_event(RawEvent::RewardChanged(point.reward));
								}
							});

							Taxation::mutate(|taxation| {
								if taxation != &point.taxation {
									*taxation = point.taxation;
									Self::deposit_event(RawEvent::TaxationChanged(point.taxation));
								}
							});
						} else {
							curve.push(point);
						}
					}
				});
			}

			T::WeightInfo::on_initialize().saturating_add(T::WeightInfo::on_finalize())
		}

		fn on_finalize(now: T::BlockNumber) {
			if let Some(author) = <Self as Store>::Author::get() {
				let reward = Reward::<T>::get();
				Self::do_reward(&author, reward, now);
			}

			<Self as Store>::Author::kill();
			<Self as Store>::AuthorDonation::kill();
		}

		/// As the block author, note your preferences for the block you produced.
		#[weight = (
			T::WeightInfo::note_author_prefs(),
			DispatchClass::Mandatory
		)]
		fn note_author_prefs(
			origin,
			donation: Perbill,
		) {
			ensure_none(origin)?;
			ensure!(
				<Self as Store>::AuthorDonation::get().is_none(),
				Error::<T>::AuthorPrefsAlreadySet
			);

			<Self as Store>::AuthorDonation::put(donation);
		}

		/// Set the reward amount for this chain. Must be Root origin.
		#[weight = (
			T::WeightInfo::set_reward(),
			DispatchClass::Operational
		)]
		fn set_reward(origin, reward: BalanceOf<T>) {
			ensure_root(origin)?;
			Self::check_new_reward_taxation(reward, Taxation::get())?;

			Reward::<T>::put(reward);
			Self::deposit_event(RawEvent::RewardChanged(reward));
		}

		/// Set the taxation amount for this chain. Must be Root origin.
		#[weight = (
			T::WeightInfo::set_taxation(),
			DispatchClass::Operational
		)]
		fn set_taxation(origin, taxation: Perbill) {
			ensure_root(origin)?;
			Self::check_new_reward_taxation(Reward::<T>::get(), taxation)?;

			Taxation::put(taxation);
			Self::deposit_event(RawEvent::TaxationChanged(taxation));
		}

		/// Unlock any vested rewards for `target` account.
		#[weight = T::WeightInfo::unlock()]
		fn unlock(origin, target: T::AccountId) {
			ensure_signed(origin)?;

			let locks = Self::reward_locks(&target);
			let current_number = frame_system::Module::<T>::block_number();
			Self::do_update_locks(&target, locks, current_number);
		}

		/// Set the reward and taxation curve for this chain. Must be Root origin.
		///
		/// NOTE: The curve should be reverse sorted by block number.
		#[weight = T::WeightInfo::set_curve(curve.len() as u32)]
		fn set_curve(origin, curve: Vec<CurvePoint<T::BlockNumber, BalanceOf<T>>>) {
			ensure_root(origin)?;
			Self::ensure_sorted(&curve)?;
			for point in &curve {
				Self::check_new_reward_taxation(point.reward, point.taxation)?;
			}

			Curve::<T>::put(curve);
			Self::deposit_event(Event::<T>::CurveSet);
		}
	}
}

const REWARDS_ID: LockIdentifier = *b"rewards ";

impl<T: Trait> Module<T> {
	fn ensure_sorted(curve: &[CurvePoint<T::BlockNumber, BalanceOf<T>>]) -> Result<(), Error<T>> {
		// Check curve is reverse sorted by block number
		ensure!(curve.windows(2).all(|w| w[0].start > w[1].start), Error::<T>::NotSorted);
		Ok(())
	}

	fn check_new_reward_taxation(reward: BalanceOf<T>, taxation: Perbill) -> Result<(), Error<T>> {
		let tax = taxation * reward;
		let miner = reward.saturating_sub(tax);

		ensure!(miner >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);

		Ok(())
	}

	fn do_reward(author: &T::AccountId, reward: BalanceOf<T>, when: T::BlockNumber) {
		let treasury_id = T::DonationDestination::get();
		let tax = Self::taxation() * reward;
		let donate = min(
			tax,
			Self::author_donation().map(|dp| dp * reward).unwrap_or(Zero::zero())
		);

		let miner_total = reward.saturating_sub(tax);

		let miner_reward_locks = T::GenerateRewardLocks::generate_reward_locks(
			when,
			miner_total
		);

		drop(T::Currency::deposit_creating(&author, miner_total));
		drop(T::Currency::deposit_creating(&treasury_id, donate));

		if miner_reward_locks.len() > 0 {
			let mut locks = Self::reward_locks(&author);

			for (new_lock_number, new_lock_balance) in miner_reward_locks {
				let old_balance = *locks.get(&new_lock_number).unwrap_or(&BalanceOf::<T>::default());
				let new_balance = old_balance.saturating_add(new_lock_balance);
				locks.insert(new_lock_number, new_balance);
			}

			Self::do_update_locks(&author, locks, when);
		}
	}

	fn do_update_locks(
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

impl<T: Trait> ProvideInherent for Module<T> {
	type Call = Call<T>;
	type Error = InherentError;
	const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;

	fn create_inherent(data: &InherentData) -> Option<Self::Call> {
		let (author_raw, donation) = data.get_data::<InherentType>(&INHERENT_IDENTIFIER).ok()??;
		let _author = T::AccountId::decode(&mut &author_raw[..]).ok()?;

		Some(Call::note_author_prefs(donation))
	}

	fn check_inherent(_call: &Self::Call, _data: &InherentData) -> result::Result<(), Self::Error> {
		Ok(())
	}
}
