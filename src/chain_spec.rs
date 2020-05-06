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

use serde_json::json;
use sp_core::{U256, crypto::UncheckedFrom};
use sc_service::ChainType;
use kulupu_runtime::{
	BalancesConfig, GenesisConfig, IndicesConfig, SystemConfig,
	DifficultyConfig, ErasConfig, AccountId, WASM_BINARY,
};

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
		Some("kulupudev"),
		Some(json!({
			"ss58Format": 16,
			"tokenDecimals": 12,
			"tokenSymbol": "KLPD"
		}).as_object().expect("Created an object").clone()),
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
		Some("kulupulocal"),
		Some(json!({
			"ss58Format": 16,
			"tokenDecimals": 12,
			"tokenSymbol": "KLPD"
		}).as_object().expect("Created an object").clone()),
		None,
	)
}

pub fn mainnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Kulupu",
		"kulupu",
		ChainType::Live,
		|| mainnet_genesis(),
		vec![], // FIXME(era1-transition): Replace this with new bootnodes.
		None,
		Some("kulupu"),
		Some(json!({
			"ss58Format": 16,
			"tokenDecimals": 12,
			"tokenSymbol": "KLP"
		}).as_object().expect("Created an object").clone()),
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

fn mainnet_genesis() -> GenesisConfig {
	let era_state = crate::eras::era0_state();

	GenesisConfig {
		system: Some(SystemConfig {
			code: include_bytes!("../res/eras/1/genesis/kulupu_runtime.compact.wasm").to_vec(),
			changes_trie_config: Default::default(),
		}),
		balances: Some(BalancesConfig {
			balances: era_state.balances.into_iter().map(|balance| {
				(AccountId::unchecked_from(balance.address), balance.balance.as_u128())
			}).collect(),
		}),
		indices: Some(IndicesConfig {
			indices: era_state.indices.into_iter().map(|index| {
				(index.index, AccountId::unchecked_from(index.address))
			}).collect(),
		}),
		difficulty: Some(DifficultyConfig {
			initial_difficulty: era_state.difficulty,
		}),
		eras: Some(ErasConfig {
			past_eras: vec![
				pallet_eras::Era {
					genesis_block_hash: era_state.previous_era.genesis_block_hash,
					final_block_hash: era_state.previous_era.final_block_hash,
					final_state_root: era_state.previous_era.final_state_root,
				}
			],
		}),
		collective_Instance1: Some(Default::default()),
		collective_Instance2: Some(Default::default()),
		democracy: Some(Default::default()),
		treasury: Some(Default::default()),
		elections_phragmen: Some(Default::default()),
		membership_Instance1: Some(Default::default()),
		timestamp: Some(Default::default()),
	}
}
