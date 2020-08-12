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
use sp_std::{result, cmp::min, prelude::*};
use sp_runtime::{RuntimeDebug, Permill, traits::{Saturating, Zero}};
use sp_inherents::{InherentIdentifier, InherentData, ProvideInherent, IsFatalError};
#[cfg(feature = "std")]
use sp_inherents::ProvideInherentData;
use frame_support::{
	decl_module, decl_storage, decl_error, decl_event, ensure,
	traits::{Get, Currency},
	weights::DispatchClass,
};
use frame_system::{ensure_none, ensure_root};

/// Trait for rewards.
pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// An implementation of on-chain currency.
	type Currency: Currency<Self::AccountId>;
	/// Donation destination.
	type DonationDestination: Get<T::AccountId>
}

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Author already set in block.
		AuthorAlreadySet,
		/// Reward set is too low.
		RewardTooLow,
		/// Donation already set in block.
		DonationAlreadySet,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		/// Current block author.
		Author get(fn author): Option<T::AccountId>;
		/// Current block donation.
		AuthorDonation get(fn author_donation): Option<Permill>;
		/// Current block reward.
		Reward get(fn reward) config(): BalanceOf<T>;
		/// Taxation rate.
		Taxation get(fn taxation): Permill;
	}
}

decl_event! {
	pub enum Event<T> where Balance = BalanceOf<T> {
		/// Block reward has changed. [reward]
		RewardChanged(Balance),
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = (
			T::DbWeight::get().reads_writes(1, 1),
			DispatchClass::Mandatory
		)]
		fn set_author(origin, author: T::AccountId) {
			ensure_none(origin)?;
			ensure!(<Self as Store>::Author::get().is_none(), Error::<T>::AuthorAlreadySet);

			<Self as Store>::Author::put(author);
		}

		#[weight = (
			T::DbWeight::get().reads_writes(1, 1),
			DispatchClass::Mandatory
		)]
		fn donate(origin, donation: Permill) {
			ensure_none(origin)?;
			ensure!(<Self as Store>::AuthorDonation::get().is_none(), Error::<T>::DonationAlreadySet);

			<Self as Store>::AuthorDonation::put(donation);
		}

		#[weight = (
			T::DbWeight::get().reads_writes(0, 1),
			DispatchClass::Operational
		)]
		fn set_reward(origin, reward: BalanceOf<T>) {
			ensure_root(origin)?;
			ensure!(reward >= T::Currency::minimum_balance(), Error::<T>::RewardTooLow);
			Reward::<T>::put(reward);
			Self::deposit_event(RawEvent::RewardChanged(reward))
		}

		fn on_finalize() {
			if let Some(author) = <Self as Store>::Author::get() {
				let treasury_id = T::DonationDestination::get();

				let mut reward = Reward::<T>::get();

				let tax = Self::taxation() * reward;
				let donate = min(
					tax,
					Self::author_donation().map(|dp| dp * reward).unwrap_or(T::Balance::zero())
				);

				let miner = reward.saturating_sub(tax);

				drop(pallet_balances::Module::<T>::deposit_creating(&author, miner));
				drop(pallet_balances::Module::<T>::deposit_creating(&treasury_id, donate));
			}

			<Self as Store>::Author::kill();
			<Self as Store>::AuthorDonation::kill();
		}
	}
}

pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"rewards_";

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

pub type InherentType = Vec<u8>;

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
		let author_raw = data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.expect("Gets and decodes anyupgrade inherent data")?;

		let author = T::AccountId::decode(&mut &author_raw[..])
			.expect("Decodes author raw inherent data");

		Some(Call::set_author(author))
	}

	fn check_inherent(_call: &Self::Call, _data: &InherentData) -> result::Result<(), Self::Error> {
		Ok(())
	}
}
