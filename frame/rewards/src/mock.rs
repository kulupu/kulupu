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

//! Mock runtime for tests

use crate::{Module, Trait, GenesisConfig};
use sp_core::H256;
use frame_support::{impl_outer_origin, impl_outer_event, parameter_types, weights::Weight};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup}, testing::Header, Perbill,
};
use frame_system as system;
use sp_std::collections::btree_map::BTreeMap;

impl_outer_origin! {
	pub enum Origin for Test {}
}

mod pallet_rewards {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum Event for Test {
		frame_system<T>,
		pallet_balances<T>,
		pallet_rewards<T>,
	}
}

// Configure a mock runtime to test the pallet.
#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

type Balance = u128;
type BlockNumber = u64;

impl system::Trait for Test {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type ModuleToIndex = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Trait for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
}

parameter_types! {
	pub DonationDestination: u64 = 255;
}

const DOLLARS: Balance = 1;
const DAYS: BlockNumber = 1;

pub struct GenerateRewardLocks;
impl crate::GenerateRewardLocks<Test> for GenerateRewardLocks {
	fn generate_reward_locks(
		current_block: BlockNumber,
		total_reward: Balance,
	) -> BTreeMap<BlockNumber, Balance> {
		let mut locks = BTreeMap::new();
		let locked_reward = total_reward.saturating_sub(1 * DOLLARS);

		if locked_reward > 0 {
			const TOTAL_LOCK_PERIOD: BlockNumber = 100 * DAYS;
			const DIVIDE: BlockNumber = 10;

			for i in 0..DIVIDE {
				let one_locked_reward = locked_reward / DIVIDE as u128;

				let estimate_block_number = current_block + (i + 1) * (TOTAL_LOCK_PERIOD / DIVIDE);
				let actual_block_number = estimate_block_number / DAYS * DAYS;

				locks.insert(actual_block_number, one_locked_reward);
			}
		}

		locks
	}
}

impl Trait for Test {
	type Event = Event;
	type Currency = Balances;
	type DonationDestination = DonationDestination;
	type GenerateRewardLocks = GenerateRewardLocks;
}

pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;
pub type Rewards = Module<Test>;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = system::GenesisConfig::default().build_storage::<Test>().unwrap();
	GenesisConfig::<Test> {
		reward: 60,
		taxation: Perbill::from_percent(10),
	}.assimilate_storage(&mut t).unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
