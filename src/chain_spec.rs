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

use kulupu_runtime::{
	BalancesConfig, GenesisConfig, IndicesConfig, SystemConfig,
	DifficultyConfig, WASM_BINARY,
};
use sp_core::U256;
use sc_service::ChainType;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

pub fn development_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Development",
		"dev",
		ChainType::Development,
		|| testnet_genesis(
			U256::from(200),
		),
		vec![],
		None,
		None,
		None,
		None,
	)
}

pub fn local_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Local Testnet",
		"local",
		ChainType::Local,
		|| testnet_genesis(
			U256::from(200),
		),
		vec![],
		None,
		None,
		None,
		None,
	)
}

fn testnet_genesis(initial_difficulty: U256) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: WASM_BINARY.to_vec(),
			changes_trie_config: Default::default(),
		}),
		balances: Some(BalancesConfig {
			balances: vec![],
		}),
		indices: Some(IndicesConfig {
			indices: vec![],
		}),
		difficulty: Some(DifficultyConfig {
			initial_difficulty,
		}),
		collective_Instance1: Some(Default::default()),
		collective_Instance2: Some(Default::default()),
		democracy: Some(Default::default()),
		treasury: Some(Default::default()),
		elections_phragmen: Some(Default::default()),
		eras: Some(Default::default()),
		membership_Instance1: Some(Default::default()),
		timestamp: Some(Default::default()),
	}
}
