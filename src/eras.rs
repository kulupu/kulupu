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

use serde::{Deserialize, Serialize};
use sp_core::{H256, U256};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousEra {
	pub genesis_block_hash: H256,
	pub final_block_hash: H256,
	pub final_state_root: H256,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
	pub address: H256,
	pub balance: U256,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
	pub address: H256,
	pub index: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
	pub previous_era: PreviousEra,
	pub difficulty: U256,
	pub balances: Vec<Balance>,
	pub indices: Vec<Index>,
}

/// Get the state of era 0.
pub fn era0_state() -> State {
	serde_json::from_slice(include_bytes!("../res/eras/0/final.json"))
		.expect("Included era state is valid")
}
