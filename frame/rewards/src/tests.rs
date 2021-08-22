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

//! Tests for Rewards Pallet

use crate::*;
use crate::mock::*;
use sp_runtime::{Digest, testing::DigestItem};
use frame_system::InitKind;
use frame_support::{assert_ok, assert_noop};
use frame_support::error::BadOrigin;
use frame_support::traits::{OnInitialize, OnFinalize};
use pallet_balances::Error as BalancesError;

// Get the last event from System
fn last_event() -> mock::Event {
	System::events().pop().expect("Event expected").event
}

/// Run until a particular block.
fn run_to_block(n: u64, author: u64) {
	while System::block_number() < n {
		Rewards::on_finalize(System::block_number());
		Balances::on_finalize(System::block_number());

		let current_block = System::block_number() + 1;
		let parent_hash = System::parent_hash();
		let pre_digest = DigestItem::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		System::initialize(
			&current_block,
			&parent_hash,
			&Digest { logs: vec![pre_digest] },
			InitKind::Full,
		);
		System::set_block_number(current_block);

		Balances::on_initialize(System::block_number());
		Rewards::on_initialize(System::block_number());
	}
}

#[test]
fn genesis_config_works() {
	new_test_ext(1).execute_with(|| {
		assert_eq!(Author::<Test>::get(), Some(1));
		assert_eq!(Reward::<Test>::get(), 60);
		assert_eq!(Balances::free_balance(1), 0);
		assert_eq!(Balances::free_balance(2), 0);
		assert_eq!(System::block_number(), 1);
	});
}

#[test]
fn set_reward_works() {
	new_test_ext(1).execute_with(|| {
		// Fails with bad origin
		assert_noop!(Rewards::set_schedule(Origin::signed(1), 42, Default::default(), Default::default(), Default::default()), BadOrigin);
		// Successful
		assert_ok!(Rewards::set_schedule(Origin::root(), 42, Default::default(), Default::default(), Default::default()));
		assert_eq!(Reward::<Test>::get(), 42);
		assert_eq!(last_event(), RawEvent::ScheduleSet.into());
		// Fails when too low
		assert_noop!(Rewards::set_schedule(Origin::root(), 0, Default::default(), Default::default(), Default::default()), Error::<Test>::RewardTooLow);
	});
}

#[test]
fn set_author_works() {
	new_test_ext(1).execute_with(|| {
		assert_eq!(Author::<Test>::get(), Some(1));
	});
}

#[test]
fn reward_payment_works() {
	new_test_ext(1).execute_with(|| {
		// Next block
		run_to_block(2, 2);
		// User gets reward
		assert_eq!(Balances::free_balance(1), 60);

		// Set new reward
		assert_ok!(Rewards::set_schedule(Origin::root(), 42, Default::default(), Default::default(), Default::default()));
		run_to_block(3, 1);
		assert_eq!(Balances::free_balance(2), 42);
	});
}

#[test]
fn reward_locks_work() {
	new_test_ext(1).execute_with(|| {
		// Make numbers better :)
		assert_ok!(Rewards::set_schedule(Origin::root(), 101, Default::default(), Default::default(), Default::default()));

		// Next block
		run_to_block(2, 1);
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
		System::set_block_number(11);
		// User update locks
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		// Locks updated
		expected_locks.remove(&11);
		assert_eq!(Rewards::reward_locks(1), expected_locks);
		// Transfer works
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 10));
		// Cannot transfer more
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// User mints more blocks
		run_to_block(12, 1);
		run_to_block(13, 1);

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
		System::set_block_number(21);
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 20));
		// 10 more unlocked on block 22
		System::set_block_number(22);
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 10));

		// Cannot transfer more
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);

		// Change lock params, 50 subperiods long 6 days each (2 coins each subperiod)
		assert_ok!(Rewards::set_lock_params(Origin::root(), LockParameters {period: 300, divide:50}));
		// Moving to block 25 to mine it so unlocks will happen on blocks 31,37,43,50,57...325
		System::set_block_number(25);
		// Mine it
		run_to_block(26, 1);
		// Now only 1 free coin should be available
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 1));
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);
		// Reinitialize the reference BTreeMap and check equality
		let mut expected_locks = BTreeMap::new();
		for block in 31..=325 {
			let mut amount = 0;
			if block <= 101 {
				if block % 10 == 1 { amount += 20; }
				if block % 10 == 2 { amount += 10; }
			} else if block <= 111 {
				if block % 10 == 1 { amount += 10; }
				if block % 10 == 2 { amount += 10; }
			} else if block <= 112 {
				if block % 10 == 2 { amount += 10; }
			}
			if block % 6 == 1 { amount += 2; }
			if amount > 0 { expected_locks.insert(block, amount); }
		}
		assert_eq!(Rewards::reward_locks(1), expected_locks);

		// 22 more is unlocked on block 31
		System::set_block_number(31);
		assert_ok!(Rewards::unlock(Origin::signed(1), 1));
		assert_ok!(Balances::transfer(Origin::signed(1), 2, 22));
		assert_noop!(Balances::transfer(Origin::signed(1), 2, 1), BalancesError::<Test, _>::LiquidityRestrictions);
	});
}

fn test_curve() -> Vec<(u64, u128)> {
	vec![
		(50, 20),
		(40, 25),
		(20, 50),
		(10, 100),
	]
}

#[test]
fn curve_works() {
	new_test_ext(1).execute_with(|| {
		// Set reward curve
		assert_ok!(Rewards::set_schedule(Origin::root(), 60, Default::default(), test_curve(), Default::default()));
		assert_eq!(last_event(), mock::Event::Rewards(crate::Event::<Test>::ScheduleSet));
		// Check current reward
		assert_eq!(Rewards::reward(), 60);
		run_to_block(9, 1);
		assert_eq!(Rewards::reward(), 60);
		run_to_block(10, 1);
		// Update successful
		assert_eq!(Rewards::reward(), 100);
		// Success reported
		assert_eq!(last_event(), mock::Event::Rewards(crate::Event::<Test>::RewardChanged(100)));
		run_to_block(20, 1);
		assert_eq!(Rewards::reward(), 50);
		// No change, not part of the curve
		run_to_block(30, 1);
		assert_eq!(Rewards::reward(), 50);
		run_to_block(40, 1);
		assert_eq!(Rewards::reward(), 25);
		run_to_block(50, 1);
		assert_eq!(Rewards::reward(), 20);
		// Curve is finished and is empty
		assert_eq!(RewardChanges::<Test>::get(), Default::default());
		// Everything works fine past the curve definition
		run_to_block(100, 1);
		assert_eq!(Rewards::reward(), 20);
	});
}

#[test]
fn set_lock_params_works() {
	new_test_ext(1).execute_with(|| {
		// Check initial data
		assert_eq!(LockParams::get(), None);
		// Set lock params
		assert_ok!(Rewards::set_lock_params(Origin::root(), LockParameters {period: 90, divide:30}));
		assert_eq!(last_event(), mock::Event::Rewards(crate::Event::<Test>::LockParamsChanged(LockParameters {period: 90, divide:30})));
		assert_eq!(LockParams::get(), Some(LockParameters {period: 90, divide:30}));
		assert_ok!(Rewards::set_lock_params(Origin::root(), LockParameters {period: 300, divide:50}));
		assert_eq!(last_event(), mock::Event::Rewards(crate::Event::<Test>::LockParamsChanged(LockParameters {period: 300, divide:50})));
		assert_eq!(LockParams::get(), Some(LockParameters {period: 300, divide:50}));
		// Check bounds
		assert_noop!(Rewards::set_lock_params(Origin::root(), LockParameters {period: 600, divide: 10}),
																	Error::<Test>::LockParamsOutOfBounds);
		assert_eq!(LockParams::get(), Some(LockParameters {period: 300, divide:50}));
		assert_noop!(Rewards::set_lock_params(Origin::root(), LockParameters {period: 400, divide: 100}),
																	Error::<Test>::LockParamsOutOfBounds);
		assert_eq!(LockParams::get(), Some(LockParameters {period: 300, divide:50}));
		// Check divisibility
		assert_noop!(Rewards::set_lock_params(Origin::root(), LockParameters {period: 400, divide:47}),
																	Error::<Test>::LockPeriodNotDivisible);
		assert_eq!(LockParams::get(), Some(LockParameters {period: 300, divide:50}));
	});
}
