// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2019-2020 Wei Tang.
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

use serde_json::json;
use sp_core::{U256, crypto::UncheckedFrom, Public, Pair, sr25519};
use sp_runtime::traits::{Verify, IdentifyAccount};
use sc_service::ChainType;
use kulupu_primitives::DOLLARS;
use kulupu_runtime::{
	BalancesConfig, GenesisConfig, IndicesConfig, SystemConfig,
	DifficultyConfig, ErasConfig, AccountId, RewardsConfig, WASM_BINARY, Signature,
};

type AccountPublic = <Signature as Verify>::Signer;

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
			U256::from(1000),
			true, // enable println in smart contracts for dev env
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
			false, // disable println for local network
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
	ChainSpec::from_json_bytes(&include_bytes!("../res/breaknet4/config.json")[..])
		.expect("Breaknet4 config included is valid")
}

pub fn mainnet_config() -> ChainSpec {
	ChainSpec::from_json_bytes(&include_bytes!("../res/eras/1/3-swamp-bottom/config.json")[..])
		.expect("Mainnet config included is valid")
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn testnet_genesis(wasm_binary: &[u8], initial_difficulty: U256, _enable_println: bool) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig {
			code: wasm_binary.to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: BalancesConfig {
			balances: vec![
				(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					10_000_000 * DOLLARS
				),
				(
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					10_000_000 * DOLLARS
				),
			],
		},
		indices: IndicesConfig {
			indices: vec![],
		},
		difficulty: DifficultyConfig {
			initial_difficulty,
		},
		rewards: RewardsConfig {
			reward: 60 * DOLLARS,
			mints: Default::default(),
		},
		..Default::default()
	}
}

/// Swamp bottom genesis config generation.
#[allow(unused)]
pub fn mainnet_genesis() -> GenesisConfig {
	let era_state = crate::eras::era0_state();

	GenesisConfig {
		system: SystemConfig {
			code: include_bytes!("../res/eras/1/3-swamp-bottom/kulupu_runtime.compact.wasm").to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: BalancesConfig {
			balances: era_state.balances.into_iter().map(|balance| {
				(AccountId::unchecked_from(balance.address), balance.balance.as_u128())
			}).collect(),
		},
		indices: IndicesConfig {
			indices: era_state.indices.into_iter().map(|index| {
				(index.index, AccountId::unchecked_from(index.address))
			}).collect(),
		},
		difficulty: DifficultyConfig {
			initial_difficulty: era_state.difficulty,
		},
		eras: ErasConfig {
			past_eras: vec![
				pallet_eras::Era {
					genesis_block_hash: era_state.previous_era.genesis_block_hash,
					final_block_hash: era_state.previous_era.final_block_hash,
					final_state_root: era_state.previous_era.final_state_root,
				}
			],
		},
		rewards: RewardsConfig {
			reward: 60 * DOLLARS,
			mints: Default::default(),
		},
		..Default::default()
	}
}
