#![cfg_attr(not(feature = "std"), no_std)]

use rstd::{result, prelude::*};
use sr_primitives::RuntimeString;
use support::{
	decl_module, decl_storage, decl_error, ensure,
	traits::{Get, Currency},
	weights::SimpleDispatchInfo,
};
use system::ensure_none;
use inherents::{InherentIdentifier, InherentData, ProvideInherent, IsFatalError};
#[cfg(feature = "std")]
use inherents::ProvideInherentData;
use codec::{Encode, Decode};

pub trait Trait: balances::Trait {
	type Reward: Get<Self::Balance>;
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Author already set in block.
		AuthorAlreadySet,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Rewards {
		Author: Option<T::AccountId>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		#[weight = SimpleDispatchInfo::FixedOperational(10_000)]
		fn set_author(origin, author: T::AccountId) {
			ensure_none(origin)?;
			ensure!(<Self as Store>::Author::get().is_some(), Error::<T>::AuthorAlreadySet);

			<Self as Store>::Author::put(author);
		}

		fn on_finalize() {
			if let Some(author) = <Self as Store>::Author::get() {
				drop(balances::Module::<T>::deposit_creating(&author, T::Reward::get()));
			}

			<Self as Store>::Author::kill();
		}
	}
}

pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"rewards_";

#[derive(Encode)]
#[cfg_attr(feature = "std", derive(Debug, Decode))]
pub enum InherentError {
	Other(RuntimeString),
}

impl IsFatalError for InherentError {
	fn is_fatal_error(&self) -> bool {
		match *self {
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

pub type InherentType = Vec<u8>;

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
