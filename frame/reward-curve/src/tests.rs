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
use sp_runtime::testing::{Digest, DigestItem};
use frame_system::InitKind;
use frame_support::{assert_ok, assert_noop};
use frame_support::error::BadOrigin;
use frame_support::traits::{OnInitialize, OnFinalize};

// Get the last event from System
fn last_event() -> mock::Event {
	System::events().pop().expect("Event expected").event
}

/// Run until a particular block.
fn run_to_block(n: u64, author: u64) {
	while System::block_number() < n {
		RewardCurveModule::on_finalize(System::block_number());
		Rewards::on_finalize(System::block_number());
		Balances::on_finalize(System::block_number());

		let current_block = System::block_number() + 1;
		let parent_hash = System::parent_hash();
		let pre_digest = DigestItem::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		System::initialize(
			&current_block,
			&parent_hash,
			&Default::default(),
			&Digest { logs: vec![pre_digest] },
			InitKind::Full
		);
		System::set_block_number(current_block);

		Balances::on_initialize(System::block_number());
		Rewards::on_initialize(System::block_number());
		RewardCurveModule::on_initialize(System::block_number());
	}
}

fn reward_point(start: u64, reward: u128) -> RewardPoint<u64, u128> {
	RewardPoint { start, reward }
}

fn test_curve() -> Vec<RewardPoint<u64, u128>> {
	vec![
		reward_point(10, 100),
		reward_point(20, 50),
		reward_point(40, 25),
		reward_point(50, 20),
	]
}

#[test]
fn reward_curve_works() {
	new_test_ext(1).execute_with(|| {
		// Set reward curve
		assert_ok!(RewardCurveModule::set_reward_curve(Origin::root(), test_curve()));
		assert_eq!(last_event(), mock::Event::pallet_reward_curve(crate::Event::RewardCurveSet));
		// Check current reward
		assert_eq!(Rewards::reward(), 60);
		run_to_block(9, 1);
		assert_eq!(Rewards::reward(), 60);
		run_to_block(10, 1);
		// Update successful
		assert_eq!(Rewards::reward(), 100);
		// Success reported
		assert_eq!(last_event(), mock::Event::pallet_reward_curve(crate::Event::UpdateSuccessful));
		run_to_block(20, 1);
		assert_eq!(Rewards::reward(), 50);
		run_to_block(30, 1);
		assert_eq!(Rewards::reward(), 50);
		run_to_block(40, 1);
		assert_eq!(Rewards::reward(), 25);
		run_to_block(50, 1);
		assert_eq!(Rewards::reward(), 20);
		run_to_block(100, 1);
		assert_eq!(Rewards::reward(), 20);
	});
}

#[test]
fn set_reward_curve_works() {
	new_test_ext(1).execute_with(|| {
		// Bad Origin
		assert_noop!(RewardCurveModule::set_reward_curve(Origin::signed(1), test_curve()), BadOrigin);
		// Duplicate Point
		let duplicate_curve = vec![reward_point(20, 50), reward_point(20, 30)];
		assert_noop!(
			RewardCurveModule::set_reward_curve(Origin::root(), duplicate_curve),
			Error::<Test>::NotSorted,
		);
		// Unsorted
		let unsorted_curve = vec![reward_point(20, 50), reward_point(10, 30)];
		assert_noop!(
			RewardCurveModule::set_reward_curve(Origin::root(), unsorted_curve),
			Error::<Test>::NotSorted,
		);
		// Single Point OK
		let single_point = vec![reward_point(100, 100)];
		assert_ok!(RewardCurveModule::set_reward_curve(Origin::root(), single_point));
		// Empty Curve OK
		assert_ok!(RewardCurveModule::set_reward_curve(Origin::root(), vec![]));
	});
}

#[test]
fn failed_update_reported() {
	new_test_ext(1).execute_with(|| {
		// Shouldn't be able to set reward to 0
		let bad_curve = vec![reward_point(10, 100), reward_point(20, 0), reward_point(30, 50)];
		// Set reward curve
		assert_ok!(RewardCurveModule::set_reward_curve(Origin::root(), bad_curve));
		// Check current reward
		assert_eq!(Rewards::reward(), 60);
		run_to_block(10, 1);
		assert_eq!(Rewards::reward(), 100);
		run_to_block(20, 1);
		// Unchanged because of bad reward amount
		assert_eq!(Rewards::reward(), 100);
		assert_eq!(last_event(), mock::Event::pallet_reward_curve(crate::Event::UpdateFailed));
		// Continues to work after the fact
		run_to_block(30, 1);
		assert_eq!(Rewards::reward(), 50);
	});
}
