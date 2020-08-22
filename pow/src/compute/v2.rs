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

use codec::{Encode, Decode};
use sp_core::{H256, crypto::Pair, hashing::blake2_256};
use kulupu_primitives::Difficulty;
use super::Calculation;
use crate::app;

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct SealV2 {
	pub difficulty: Difficulty,
	pub nonce: H256,
	pub signature: app::Signature,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ComputeV2 {
	pub key_hash: H256,
	pub pre_hash: H256,
	pub difficulty: Difficulty,
	pub nonce: H256,
}

impl ComputeV2 {
	pub fn seal_and_work(&self, signature: app::Signature, mode: super::ComputeMode) -> (SealV2, H256) {
		let calculation = Calculation {
			difficulty: self.difficulty,
			pre_hash: self.pre_hash,
			nonce: self.nonce,
		};

		let work = super::compute::<(Calculation, app::Signature)>(
			&self.key_hash,
			&(calculation, signature.clone()),
			mode,
		);

		(SealV2 {
			nonce: self.nonce,
			difficulty: self.difficulty,
			signature,
		}, work)
	}

	fn signing_message(&self) -> [u8; 32] {
		let calculation = Calculation {
			difficulty: self.difficulty,
			pre_hash: self.pre_hash,
			nonce: self.nonce,
		};

		blake2_256(&calculation.encode()[..])
	}

	pub fn sign(&self, pair: &app::Pair) -> app::Signature {
		let hash = self.signing_message();
		pair.sign(&hash[..])
	}

	pub fn verify(
		&self,
		signature: &app::Signature,
		public: &app::Public,
	) -> bool {
		let hash = self.signing_message();
		app::Pair::verify(
			signature,
			&hash[..],
			public,
		)
	}
}
