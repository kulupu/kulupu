#![cfg_attr(not(feature = "std"), no_std)]

use rstd::{result, prelude::*, collections::btree_map::BTreeMap};
use sr_primitives::{
	traits::{StaticLookup, Dispatchable, UniqueSaturatedInto},
	DispatchError, RuntimeString,
};
use support::{Parameter, weights::SimpleDispatchInfo, decl_module, decl_event};
use system::ensure_none;
use inherents::{InherentIdentifier, InherentData, ProvideInherent, IsFatalError};
#[cfg(feature = "std")]
use inherents::ProvideInherentData;
use codec::{Encode, Decode};

pub trait IsAnyUpgradeInherent {
	fn is_anyupgrade_inherent(&self) -> bool;
}

pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as system::Trait>::Event>;

	/// A sudo-able call.
	type Proposal: Parameter + Dispatchable<Origin=Self::Origin>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = SimpleDispatchInfo::FixedOperational(10_000)]
		fn any(origin, proposal: Box<T::Proposal>) {
			ensure_none(origin)?;

			let res = match proposal.dispatch(system::RawOrigin::Root.into()) {
				Ok(_) => true,
				Err(e) => {
					let e: DispatchError = e.into();
					sr_primitives::print(e);
					false
				}
			};

			Self::deposit_event(Event::AnyDone(res));
		}

		#[weight = SimpleDispatchInfo::FixedOperational(10_000)]
		fn any_as(origin, who: <T::Lookup as StaticLookup>::Source, proposal: Box<T::Proposal>) {
			ensure_none(origin)?;

			let who = T::Lookup::lookup(who)?;

			let res = match proposal.dispatch(system::RawOrigin::Signed(who).into()) {
				Ok(_) => true,
				Err(e) => {
					let e: DispatchError = e.into();
					sr_primitives::print(e);
					false
				}
			};

			Self::deposit_event(Event::AnyAsDone(res));
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn is_inherent_required(data: &InherentData) -> Result<bool, InherentError> {
		let (check_from, whitelist) = match data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.map_err(|_| InherentError::Other(RuntimeString::from("Invalid anyupgrade inherent data encoding.")))?
		{
			Some(whitelist) => whitelist,
			None => return Ok(false),
		};

		let current_num = UniqueSaturatedInto::<u64>::unique_saturated_into(
			system::Module::<T>::block_number()
		);
		if current_num < check_from {
			return Ok(false)
		}

		Ok(whitelist.get(&current_num).is_some())
	}

	pub fn check_inherents(has_anyupgrade_inherent: bool, data: &InherentData) -> Result<(), InherentError> {
		if Self::is_inherent_required(&data)? {
			if has_anyupgrade_inherent {
				Ok(())
			} else {
				Err(InherentError::RequiredNotFound)
			}
		} else {
			Ok(())
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

#[derive(Encode)]
#[cfg_attr(feature = "std", derive(Debug, Decode))]
pub enum InherentError {
	NotWhitelisted,
	Other(RuntimeString),
	RequiredNotFound,
}

impl IsFatalError for InherentError {
	fn is_fatal_error(&self) -> bool {
		match *self {
			InherentError::NotWhitelisted => true,
			InherentError::RequiredNotFound => true,
			InherentError::Other(_) => true,
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

#[cfg(feature = "std")]
pub struct InherentDataProvider(pub InherentType);

#[cfg(feature = "std")]
impl ProvideInherentData for InherentDataProvider {
	fn inherent_identifier(&self) -> &'static InherentIdentifier {
		&INHERENT_IDENTIFIER
	}

	fn provide_inherent_data(&self, inherent_data: &mut InherentData) -> Result<(), inherents::Error> {
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
		let (_, whitelist) = data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.expect("Gets and decodes anyupgrade inherent data")?;

		let current_num = UniqueSaturatedInto::<u64>::unique_saturated_into(
			system::Module::<T>::block_number()
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

	fn check_inherent(call: &Self::Call, data: &InherentData) -> result::Result<(), Self::Error> {
		let (check_from, whitelist) = match data.get_data::<InherentType>(&INHERENT_IDENTIFIER)
			.map_err(|_| InherentError::Other(RuntimeString::from("Invalid anyupgrade inherent data encoding.")))?
		{
			Some((check_from, whitelist)) => (check_from, whitelist),
			None => return Ok(()),
		};

		let current_num = UniqueSaturatedInto::<u64>::unique_saturated_into(
			system::Module::<T>::block_number()
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
