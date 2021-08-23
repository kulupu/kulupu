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

use super::Calculation;
use crate::app;
use codec::{Decode, Encode};
use kulupu_primitives::Difficulty;
use sp_core::{crypto::Pair, hashing::blake2_256, H256};

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
	pub fn input(&self, signature: app::Signature) -> (Calculation, app::Signature) {
		let calculation = Calculation {
			difficulty: self.difficulty,
			pre_hash: self.pre_hash,
			nonce: self.nonce,
		};

		(calculation, signature)
	}

	pub fn seal_and_work(
		&self,
		signature: app::Signature,
		mode: super::ComputeMode,
	) -> Result<(SealV2, H256), super::Error> {
		let input = self.input(signature.clone());

		let work = super::compute::<(Calculation, app::Signature)>(&self.key_hash, &input, mode)?;

		Ok((
			SealV2 {
				nonce: self.nonce,
				difficulty: self.difficulty,
				signature,
			},
			work,
		))
	}

	pub fn seal(&self, signature: app::Signature) -> SealV2 {
		SealV2 {
			nonce: self.nonce,
			difficulty: self.difficulty,
			signature,
		}
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

	pub fn verify(&self, signature: &app::Signature, public: &app::Public) -> bool {
		let hash = self.signing_message();
		app::Pair::verify(signature, &hash[..], public)
	}
}
