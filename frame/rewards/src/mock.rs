// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2020 Wei Tang.
// Copyright (c) 2020 Shawn Tabrizi.
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

//! Mock runtime for tests

use super::*;
use crate as pallet_rewards;

use sp_core::H256;
use codec::Encode;
use frame_support::{parameter_types, traits::OnInitialize};
use sp_runtime::{
	Digest, traits::{BlakeTwo256, IdentityLookup}, testing::{DigestItem, Header},
};
use frame_system::{self as system, InitKind};
use sp_std::{collections::btree_map::BTreeMap, cmp};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime! {
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Rewards: pallet_rewards::{Pallet, Call, Storage, Config<T>, Event<T>},
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

type Balance = u128;
type BlockNumber = u64;

impl system::Config for Test {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type BlockWeights = ();
	type BlockLength = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = MaxLocks;
	type WeightInfo = ();
}

const DOLLARS: Balance = 1;
const DAYS: BlockNumber = 1;

pub struct GenerateRewardLocks;
impl crate::GenerateRewardLocks<Test> for GenerateRewardLocks {
	fn generate_reward_locks(
		current_block: BlockNumber,
		total_reward: Balance,
		lock_parameters: Option<LockParameters>,
	) -> BTreeMap<BlockNumber, Balance> {
		let mut locks = BTreeMap::new();
		let locked_reward = total_reward.saturating_sub(1 * DOLLARS);

		if locked_reward > 0 {
			let total_lock_period: BlockNumber;
			let divide: BlockNumber;

			if let Some(lock_parameters) = lock_parameters {
				total_lock_period = u64::from(lock_parameters.period) * DAYS;
				divide = u64::from(lock_parameters.divide);
			} else {
				total_lock_period = 100 * DAYS;
				divide = 10;
			}
			for i in 0..divide {
				let one_locked_reward = locked_reward / divide as u128;

				let estimate_block_number = current_block.saturating_add((i + 1) * (total_lock_period / divide));
				let actual_block_number = estimate_block_number / DAYS * DAYS;

				locks.insert(actual_block_number, one_locked_reward);
			}
		}

		locks
	}

	fn max_locks(lock_bounds: pallet_rewards::LockBounds) -> u32 {
		// Max locks when a miner mines at least one block every day till the lock period of
		// the first mined block ends.
		cmp::max(100, u32::from(lock_bounds.period_max))
	}
}

parameter_types! {
	pub DonationDestination: u64 = 255;
	pub const LockBounds: pallet_rewards::LockBounds = pallet_rewards::LockBounds {period_max: 500, period_min: 20,
																					divide_max: 50, divide_min: 2};
}

impl pallet_rewards::Config for Test {
	type Event = Event;
	type Currency = Balances;
	type DonationDestination = DonationDestination;
	type GenerateRewardLocks = GenerateRewardLocks;
	type WeightInfo = ();
	type LockParametersBounds = LockBounds;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext(author: u64) -> sp_io::TestExternalities {
	let mut t = system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_rewards::GenesisConfig::<Test> {
		reward: 60,
		mints: BTreeMap::new(),
	}.assimilate_storage(&mut t).unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		let current_block = 1;
		let parent_hash = System::parent_hash();
		let pre_digest = DigestItem::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		System::initialize(
			&current_block,
			&parent_hash,
			&Digest { logs: vec![pre_digest] },
			InitKind::Full,
		);
		System::set_block_number(current_block);

		Balances::on_initialize(current_block);
		Rewards::on_initialize(current_block);
	});
	ext
}
