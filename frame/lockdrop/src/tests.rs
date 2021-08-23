use super::*;

use crate as pallet_lockdrop;
use frame_support::{
	assert_noop, assert_ok, assert_storage_noop, parameter_types,
	traits::{Everything, OnFinalize, OnInitialize},
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// For testing the pallet, we construct a mock runtime.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Lockdrop: pallet_lockdrop::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Call = Call;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = u64;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

parameter_types! {
	pub const PayloadLenLimit: u32 = 32;
	pub const RemoveKeysLimit: u32 = 1024;
}

impl pallet_lockdrop::Config for Test {
	type Event = Event;
	type Currency = Balances;
	type PayloadLenLimit = PayloadLenLimit;
	type RemoveKeysLimit = RemoveKeysLimit;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = GenesisConfig {
		system: Default::default(),
		balances: pallet_balances::GenesisConfig {
			balances: vec![(1, 1000), (2, 2000)],
		},
	}
	.build_storage()
	.unwrap();
	t.into()
}

pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		if System::block_number() > 1 {
			Lockdrop::on_finalize(System::block_number());
			System::on_finalize(System::block_number());
		}
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
		Lockdrop::on_initialize(System::block_number());
	}
}

pub const TEST_CAMPAIGN: [u8; 4] = [b't', b'e', b's', b't'];

#[test]
fn create_campaign_lock_works() {
	new_test_ext().execute_with(|| {
		run_to_block(5);
		assert_ok!(Lockdrop::create_campaign(
			Origin::root(),
			TEST_CAMPAIGN,
			20,
			30
		));

		run_to_block(7);
		assert_ok!(Lockdrop::lock(
			Origin::signed(1),
			1000,
			TEST_CAMPAIGN,
			40,
			None
		));
		assert_noop!(
			Lockdrop::lock(Origin::signed(2), 5000, TEST_CAMPAIGN, 40, None),
			Error::<Test>::NotEnoughBalance
		);
		assert_noop!(
			Lockdrop::lock(Origin::signed(2), 2000, TEST_CAMPAIGN, 25, None),
			Error::<Test>::InvalidLockEndBlock
		);
		assert_storage_noop!(
			Lockdrop::conclude_campaign(Origin::signed(3), TEST_CAMPAIGN).unwrap()
		);
		assert_storage_noop!(Lockdrop::remove_expired_child_storage(
			Origin::signed(3),
			TEST_CAMPAIGN
		)
		.unwrap());

		run_to_block(15);
		assert_storage_noop!(Lockdrop::unlock(Origin::signed(1), TEST_CAMPAIGN).unwrap());
		assert_eq!(Balances::usable_balance(1), 0);
		assert_eq!(Balances::usable_balance(2), 2000);

		run_to_block(21);
		assert_noop!(
			Lockdrop::lock(Origin::signed(2), 1000, TEST_CAMPAIGN, 40, None),
			Error::<Test>::CampaignAlreadyExpired
		);
		assert_eq!(Balances::usable_balance(1), 0);
		assert_eq!(Balances::usable_balance(2), 2000);

		assert_ok!(Lockdrop::conclude_campaign(
			Origin::signed(3),
			TEST_CAMPAIGN
		));
		assert_ok!(Lockdrop::remove_expired_child_storage(
			Origin::signed(3),
			TEST_CAMPAIGN
		));

		run_to_block(23);
		assert_storage_noop!(Lockdrop::unlock(Origin::signed(1), TEST_CAMPAIGN).unwrap());
		assert_eq!(Balances::usable_balance(1), 0);
		assert_eq!(Balances::usable_balance(2), 2000);

		run_to_block(41);
		assert_ok!(Lockdrop::unlock(Origin::signed(1), TEST_CAMPAIGN));
		assert_eq!(Balances::usable_balance(1), 1000);
	})
}
