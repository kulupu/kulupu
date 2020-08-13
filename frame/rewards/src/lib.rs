// Copyright 2019-2020 Wei Tang.
// This file is part of Kulupu.

// Kulupu is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Kulupu is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Kulupu.  If not, see <http://www.gnu.org/licenses/>.

//! Reward handling module for Kulupu.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Encode, Decode};
use sp_std::{result, cmp::min, prelude::*, collections::btree_map::BTreeMap};
use sp_runtime::{RuntimeDebug, Perbill, traits::{Saturating, Zero, UniqueSaturatedFrom}};
use sp_inherents::{InherentIdentifier, InherentData, ProvideInherent, IsFatalError};
#[cfg(feature = "std")]
use sp_inherents::ProvideInherentData;
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	traits::{Get, Currency, LockIdentifier, LockableCurrency, WithdrawReason},
	weights::{DispatchClass, Weight},
};
use frame_system::{ensure_none, ensure_root, ensure_signed};
use kulupu_primitives::{DOLLARS, DAYS};

/// Trait for rewards.
pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// An implementation of on-chain currency.
	type Currency: LockableCurrency<Self::AccountId>;
	/// Donation destination.
	type DonationDestination: Get<Self::AccountId>;
}

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

#[derive(RuntimeDebug, Clone, Eq, PartialEq, Encode, Decode)]
/// Locking preferences.
pub enum LockPref {
	/// No lock. [20 KLP]
	None = 0,
	/// Single lock (8 days). [25 KLP]
	Single = 1,
	/// Double lock (16 days). [30 KLP]
	Double = 2,
	/// Triple lock (32 days). [35 KLP]
	Triple = 3,
	/// Quadruple lock (64 days). [40 KLP]
	Quadruple = 4,
	/// Quintuple lock (128 days). [50 KLP]
	Quintuple = 5,
	/// Hextuple lock (256 days). [60 KLP]
	Hextuple = 6,
}

impl Default for LockPref {
	fn default() -> LockPref {
		LockPref::None
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Author already set in block.
		AuthorAlreadySet,
		/// Reward set is too low.
		RewardTooLow,
		/// Donation already set in block.
		DonationAlreadySet,
		/// Author lock pref already set in block.
		LockPrefAlreadySet,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Current block author.
		Author get(fn author): Option<T::AccountId>;
		/// Current block donation.
		AuthorDonation get(fn author_donation): Option<Perbill>;
		/// Current block author preference.
		AuthorLockPref get(fn author_lock_pref): Option<LockPref>;
		/// Current block reward.
		Reward get(fn reward) config(): BalanceOf<T>;
		/// Taxation rate.
		Taxation get(fn taxation) config(): Perbill;
		/// Pending reward locks.
		RewardLocks get(fn reward_locks):
			map hasher(twox_64_concat) T::AccountId => BTreeMap<T::BlockNumber, BalanceOf<T>>;
	}
}

decl_event! {
	pub enum Event<T> where Balance = BalanceOf<T> {
		/// Block reward has changed. [reward]
		RewardChanged(Balance),
		/// Block taxation has changed. [taxation]
		TaxationChanged(Perbill),
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = (
			T::DbWeight::get().reads_writes(2, 2),
			DispatchClass::Mandatory
		)]
		fn set_author(
			origin,
			author: T::AccountId,
			donation: Perbill,
			lock_pref: LockPref
		) {
			ensure_none(origin)?;
			ensure!(<Self as Store>::Author::get().is_none(), Error::<T>::AuthorAlreadySet);
			ensure!(<Self as Store>::AuthorDonation::get().is_none(), Error::<T>::DonationAlreadySet);
			ensure!(<Self as Store>::AuthorLockPref::get().is_none(), Error::<T>::LockPrefAlreadySet);

			<Self as Store>::Author::put(author);
			<Self as Store>::AuthorDonation::put(donation);
			<Self as Store>::AuthorLockPref::put(lock_pref);
		}

		#[weight = (
			T::DbWeight::get().reads_writes(0, 1),
			DispatchClass::Operational
		)]
		fn set_reward(origin, reward: BalanceOf<T>) {
			ensure_root(origin)?;
			Self::check_new_reward_taxation(reward, Taxation::get())?;

			Reward::<T>::put(reward);
			Self::deposit_event(RawEvent::RewardChanged(reward));
		}

		#[weight = (
			T::DbWeight::get().reads_writes(0, 1),
			DispatchClass::Operational
		)]
		fn set_taxation(origin, taxation: Perbill) {
			ensure_root(origin)?;
			Self::check_new_reward_taxation(Reward::<T>::get(), taxation)?;

			Taxation::put(taxation);
			Self::deposit_event(RawEvent::TaxationChanged(taxation));
		}

		#[weight = T::DbWeight::get().reads_writes(1, 1)]
		fn update_locks(origin) {
			let account = ensure_signed(origin)?;
			let locks = Self::reward_locks(&account);

			Self::do_update_locks(account, locks);
		}

		fn on_finalize() {
			if let Some(author) = <Self as Store>::Author::get() {
				let treasury_id = T::DonationDestination::get();

				let reward = Reward::<T>::get();

				let tax = Self::taxation() * reward;
				let donate = min(
					tax,
					Self::author_donation().map(|dp| dp * reward).unwrap_or(Zero::zero())
				);

				let miner_pref = <Self as Store>::AuthorLockPref::get().unwrap_or_default();
				let miner_total = reward.saturating_sub(tax);
				let (miner_reward, lock_days) = match miner_pref {
					LockPref::None => (miner_total / 3.into(), 0),
					LockPref::Single => (miner_total / 3.into() + miner_total / 12.into(), 8),
					LockPref::Double => (miner_total / 2.into(), 16),
					LockPref::Triple => (miner_total / 2.into() + miner_total / 12.into(), 32),
					LockPref::Quadruple => (miner_total / 3.into() + miner_total / 3.into(), 64),
					LockPref::Quintuple => (miner_total / 2.into() + miner_total / 2.into(), 128),
					LockPref::Hextuple => (miner_total, 256),
				};

				drop(T::Currency::deposit_creating(&author, miner_reward));
				drop(T::Currency::deposit_creating(&treasury_id, donate));

				// Make sure at least 1 KLP is not locked, avoiding the miner
				// does not have fund to send transactions.
				let miner_locked = miner_reward.saturating_sub(
					UniqueSaturatedFrom::unique_saturated_from(1 * DOLLARS)
				);

				if miner_locked > Zero::zero() && lock_days > 0 {
					let mut locks = Self::reward_locks(&author);
					let target_lock_block =
						(frame_system::Module::<T>::block_number() + (lock_days * DAYS).into())
						/ DAYS.into() * DAYS.into();

					locks.insert(target_lock_block, miner_locked);
					Self::do_update_locks(author, locks);
				}
			}

			<Self as Store>::Author::kill();
			<Self as Store>::AuthorDonation::kill();
			<Self as Store>::AuthorLockPref::kill();
		}

		// [fixme: should be removed in next runtime upgrade]
		fn on_runtime_upgrade() -> Weight {
			Taxation::put(Perbill::zero());

			0
		}
	}
}

const REWARDS_ID: LockIdentifier = *b"rewards ";

impl<T: Trait> Module<T> {
	fn check_new_reward_taxation(reward: BalanceOf<T>, taxation: Perbill) -> Result<(), Error<T>> {
		let tax = taxation * reward;
		let miner = reward.saturating_sub(tax);

		ensure!(miner >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);

		Ok(())
	}

	fn do_update_locks(author: T::AccountId, mut locks: BTreeMap<T::BlockNumber, BalanceOf<T>>) {
		let current_number = frame_system::Module::<T>::block_number();
		let mut expired = Vec::new();
		let mut total_locked = Zero::zero();

		for (block_number, locked_balance) in &locks {
			if block_number <= &current_number {
				expired.push(*block_number);
			} else {
				total_locked += *locked_balance;
			}
		}

		for block_number in expired {
			locks.remove(&block_number);
		}

		T::Currency::set_lock(
			REWARDS_ID,
			&author,
			total_locked,
			WithdrawReason::Transfer | WithdrawReason::Reserve,
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
		let author = T::AccountId::decode(&mut &author_raw[..]).ok()?;

		Some(Call::set_author(author, donation, LockPref::None))
	}

	fn check_inherent(_call: &Self::Call, _data: &InherentData) -> result::Result<(), Self::Error> {
		Ok(())
	}
}
