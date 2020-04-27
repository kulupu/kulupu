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

//! Hard fork upgrade support via inherents.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use sp_std::{result, prelude::*, collections::btree_map::BTreeMap};
use sp_inherents::{ProvideInherent, InherentData, InherentIdentifier};
#[cfg(feature = "std")]
use sp_inherents::ProvideInherentData;
use sp_runtime::{
	traits::{StaticLookup, Dispatchable, UniqueSaturatedInto}, RuntimeDebug,
};
use frame_support::{Parameter, inherent::IsFatalError, decl_module, decl_event};
use frame_support::weights::{FunctionOf, Pays, GetDispatchInfo};
use frame_system::{self as system, ensure_none};

/// Anyupgrade configuration trait.
pub trait Trait: frame_system::Trait {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as frame_system::Trait>::Event>;

	/// A sudo-able call.
	type Call: Parameter + Dispatchable<Origin=Self::Origin> + GetDispatchInfo;
}

decl_module! {
	/// Anyupgrade module.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		/// Declare an anyupgrade.
		#[weight = FunctionOf(
			|args: (&Box<<T as Trait>::Call>,)| args.0.get_dispatch_info().weight + 10_000,
			|args: (&Box<<T as Trait>::Call>,)| args.0.get_dispatch_info().class,
			Pays::Yes,
		)]
		fn any(origin, call: Box<<T as Trait>::Call>) {
			ensure_none(origin)?;

			let res = match call.dispatch(frame_system::RawOrigin::Root.into()) {
				Ok(_) => true,
				Err(e) => {
					sp_runtime::print(e);
					false
				}
			};

			Self::deposit_event(Event::AnyDone(res));
		}

		/// Declare an anyupgrade as a user.
		#[weight = FunctionOf(
			|args: (&<T::Lookup as StaticLookup>::Source, &Box<<T as Trait>::Call>,)| {
				args.1.get_dispatch_info().weight + 10_000
			},
			|args: (&<T::Lookup as StaticLookup>::Source, &Box<<T as Trait>::Call>,)| {
				args.1.get_dispatch_info().class
			},
			Pays::Yes,
		)]
		fn any_as(origin, who: <T::Lookup as StaticLookup>::Source, call: Box<<T as Trait>::Call>) {
			ensure_none(origin)?;

			let who = T::Lookup::lookup(who)?;

			let res = match call.dispatch(frame_system::RawOrigin::Signed(who).into()) {
				Ok(_) => true,
				Err(e) => {
					sp_runtime::print(e);
					false
				}
			};

			Self::deposit_event(Event::AnyAsDone(res));
		}
	}
}

decl_event!(
	pub enum Event {
		AnyDone(bool),
		AnyAsDone(bool),
	}
);

pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"anyupgra";

#[derive(Encode, Decode, RuntimeDebug)]
pub enum InherentError {
	NotWhitelisted,
	RequiredNotFound,
	InvalidEncoding,
}

impl IsFatalError for InherentError {
	fn is_fatal_error(&self) -> bool {
		match *self {
			InherentError::NotWhitelisted => true,
			InherentError::RequiredNotFound => true,
			InherentError::InvalidEncoding => true,
		}
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

pub type InherentType = (u64, BTreeMap<u64, Vec<u8>>);

impl<T: Trait> ProvideInherent for Module<T> {
	type Call = Call<T>;
	type Error = InherentError;
	const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;

	fn create_inherent(data: &InherentData) -> Option<Self::Call> {
		let (_, whitelist) = data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.expect("Gets and decodes anyupgrade inherent data")?;

		let current_num = UniqueSaturatedInto::<u64>::unique_saturated_into(
			frame_system::Module::<T>::block_number()
		);
		for (num, call) in whitelist {
			if num == current_num {
				return Some(
					Call::decode(&mut &call[..]).expect("Gets and decodes anyupgrades call data")
				)
			}
		}

		None
	}

	fn is_inherent_required(data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
		let (check_from, whitelist) = match data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.map_err(|_| InherentError::InvalidEncoding)?
		{
			Some((check_from, whitelist)) => (check_from, whitelist),
			None => return Ok(None),
		};

		let current_num = UniqueSaturatedInto::<u64>::unique_saturated_into(
			frame_system::Module::<T>::block_number()
		);
		if current_num < check_from {
			return Ok(None)
		}

		Ok(if whitelist.get(&current_num).is_some() {
			Some(InherentError::RequiredNotFound)
		} else {
			None
		})
	}

	fn check_inherent(call: &Self::Call, data: &InherentData) -> result::Result<(), Self::Error> {
		let (check_from, whitelist) = match data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.map_err(|_| InherentError::InvalidEncoding)?
		{
			Some((check_from, whitelist)) => (check_from, whitelist),
			None => return Ok(()),
		};

		let current_num = UniqueSaturatedInto::<u64>::unique_saturated_into(
			frame_system::Module::<T>::block_number()
		);
		if current_num < check_from {
			return Ok(())
		}

		let ccall = call.encode();
		for (num, call) in whitelist {
			if num == current_num && call == ccall {
				return Ok(())
			}
		}

		Err(InherentError::NotWhitelisted)
	}
}

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
