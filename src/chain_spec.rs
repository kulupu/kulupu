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
use sp_runtime::Perbill;
use sc_service::ChainType;
use kulupu_primitives::DOLLARS;
use kulupu_runtime::{
	BalancesConfig, GenesisConfig, IndicesConfig, SystemConfig,
	DifficultyConfig, ErasConfig, AccountId, RewardsConfig, WASM_BINARY,
};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

pub fn development_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

	Ok(ChainSpec::from_genesis(
		"Development",
		"dev",
		ChainType::Development,
		move || testnet_genesis(
			wasm_binary,
			U256::from(200000),
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
	))
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or("Development wasm binary not available".to_string())?;

	Ok(ChainSpec::from_genesis(
		"Local Testnet",
		"local",
		ChainType::Local,
		move || testnet_genesis(
			wasm_binary,
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
	))
}

pub fn breaknet4_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Kulupu breaknet4",
		"breaknet4",
		ChainType::Live,
		|| breaknet4_genesis(
			U256::from(2000),
		),
		vec![
			"/ip4/95.217.86.109/tcp/20999/p2p/12D3KooWR1SuQsNhZNUcQQtmSbqQ4JpMq7op5AuLyuQHCKGcTuZQ".parse().unwrap(),
		],
		None,
		Some("kulupubreaknet4"),
		Some(json!({
			"ss58Format": 16,
			"tokenDecimals": 12,
			"tokenSymbol": "KLPTEST4"
		}).as_object().expect("Created an object").clone()),
		None,
	)
}

pub fn mainnet_config() -> ChainSpec {
	ChainSpec::from_json_bytes(&include_bytes!("../res/eras/1/3-swamp-bottom/config.json")[..])
		.expect("Mainnet config included is valid")
}

fn breaknet4_genesis(initial_difficulty: U256) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: include_bytes!("../res/breaknet4/kulupu_runtime.compact.wasm").to_vec(),
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
		vesting: Some(Default::default()),
		rewards: Some(RewardsConfig {
			reward: 60 * DOLLARS,
			taxation: Perbill::from_percent(20),
		}),
	}
}

fn testnet_genesis(wasm_binary: &[u8], initial_difficulty: U256) -> GenesisConfig {
	GenesisConfig {
		system: Some(SystemConfig {
			code: wasm_binary.to_vec(),
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
		vesting: Some(Default::default()),
		rewards: Some(RewardsConfig {
			reward: 60 * DOLLARS,
			taxation: Perbill::from_percent(0),
		}),
	}
}

/// Swamp bottom genesis config generation.
#[allow(unused)]
pub fn mainnet_genesis() -> GenesisConfig {
	let era_state = crate::eras::era0_state();

	GenesisConfig {
		system: Some(SystemConfig {
			code: include_bytes!("../res/eras/1/3-swamp-bottom/kulupu_runtime.compact.wasm").to_vec(),
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
		vesting: None,
		rewards: Some(RewardsConfig {
			reward: 60 * DOLLARS,
			taxation: Perbill::from_percent(0),
		}),
	}
}
