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

//! Tests for Rewards Pallet

use crate::*;
use crate::mock::*;
use frame_support::{assert_ok, assert_noop};
use frame_support::error::BadOrigin;
use frame_support::traits::{OnInitialize, OnFinalize};
use pallet_balances::Error as BalancesError;

// Get the last event from System
fn last_event() -> mock::Event {
	System::events().pop().expect("Event expected").event
}

/// Run until a particular block.
fn run_to_block(n: u64) {
	while System::block_number() < n {
		Rewards::on_finalize(System::block_number());
		Balances::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		Balances::on_initialize(System::block_number());
		Rewards::on_initialize(System::block_number());
	}
}

#[test]
fn genesis_config_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Author::<Test>::get(), None);
		assert_eq!(Reward::<Test>::get(), 60);
		assert_eq!(Balances::free_balance(1), 0);
		assert_eq!(Balances::free_balance(2), 0);
		assert_eq!(System::block_number(), 1);
	});
}

#[test]
fn set_reward_works() {
	new_test_ext().execute_with(|| {
		// Fails with bad origin
		assert_noop!(Rewards::set_reward(Origin::signed(1), 42), BadOrigin);
		assert_noop!(Rewards::set_reward(Origin::none(), 42), BadOrigin);
		// Successful
		assert_ok!(Rewards::set_reward(Origin::root(), 42));
		assert_eq!(Reward::<Test>::get(), 42);
		assert_eq!(last_event(), RawEvent::RewardChanged(42).into());
		// Fails when too low
		assert_noop!(Rewards::set_reward(Origin::root(), 0), Error::<Test>::RewardTooLow);
	});
}

#[test]
fn set_author_works() {
	new_test_ext().execute_with(|| {
		// Fails with bad origin
		assert_noop!(Rewards::set_author(Origin::signed(1), 1, Perbill::zero()), BadOrigin);
		// Block author can successfully set themselves
		assert_ok!(Rewards::set_author(Origin::none(), 1, Perbill::zero()));
		// Cannot set author twice
		assert_noop!(Rewards::set_author(Origin::none(), 2, Perbill::zero()), Error::<Test>::AuthorAlreadySet);
		assert_eq!(Author::<Test>::get(), Some(1));
	});
}

#[test]
fn reward_payment_works() {
	new_test_ext().execute_with(|| {
		// Block author sets themselves as author
		assert_ok!(Rewards::set_author(Origin::none(), 1, Perbill::zero()));
		// Next block
		run_to_block(2);
		// User gets reward
		assert_eq!(Balances::free_balance(1), 54);

		// Set new reward
		assert_ok!(Rewards::set_reward(Origin::root(), 42));
		assert_ok!(Rewards::set_taxation(Origin::root(), Perbill::zero()));
		assert_ok!(Rewards::set_author(Origin::none(), 2, Perbill::zero()));
		run_to_block(3);
		assert_eq!(Balances::free_balance(2), 42);
	});
}

#[test]
fn reward_locks_work() {
	new_test_ext().execute_with(|| {
		// Make numbers better :)
		assert_ok!(Rewards::set_taxation(Origin::root(), Perbill::zero()));
		assert_ok!(Rewards::set_reward(Origin::root(), 101));

		// Block author sets themselves as author
		assert_ok!(Rewards::set_author(Origin::none(), 1, Perbill::zero()));
		// Next block
		run_to_block(2);
		// User gets reward
		assert_eq!(Balances::free_balance(1), 101);
		// 100 is locked, 1 is unlocked for paying txs
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 1));

		// Cannot transfer because of locks
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// Confirm locks (10 of them, each of value 10)
		let mut expected_locks = (1..=10).map(|x| (x * 10 + 1, 10)).collect::<BTreeMap<_, _>>();
		assert_eq!(Rewards::reward_locks(1), expected_locks);

		// 10 blocks later (10 days)
		run_to_block(11);
		// User update locks
		assert_ok!(Rewards::update_locks(Origin::signed(1)));
		// Locks updated
		expected_locks.remove(&11);
		assert_eq!(Rewards::reward_locks(1), expected_locks);
		// Transfer works
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 10));
		// Cannot transfer more
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// User mints more blocks
		assert_ok!(Rewards::set_author(Origin::none(), 1, Perbill::zero()));
		run_to_block(12);
		assert_ok!(Rewards::set_author(Origin::none(), 1, Perbill::zero()));
		run_to_block(13);

		// Locks as expected
		// Left over from block 1 + from block 11
		let mut expected_locks = (2..=10).map(|x| (x * 10 + 1, 10 + 10)).collect::<BTreeMap<_, _>>();
		// Last one from block 11
		expected_locks.insert(111, 10);
		// From block 12
		expected_locks.append(&mut (2..=11).map(|x| (x * 10 + 2, 10)).collect::<BTreeMap<_, _>>());
		assert_eq!(Rewards::reward_locks(1), expected_locks);

		// User gains 2 free for txs
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 2));
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// 20 more is unlocked on block 21
		run_to_block(21);
		assert_ok!(Rewards::update_locks(Origin::signed(1)));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 20));
		// 10 more unlocked on block 22
		run_to_block(22);
		assert_ok!(Rewards::update_locks(Origin::signed(1)));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 10));

		// Cannot transfer more
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);
	});
}
