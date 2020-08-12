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

// Get the last event from System
fn last_event() -> mock::Event {
	System::events().pop().expect("Event expected").event
}

// Move to the next block.
fn next_block() {
	Rewards::on_finalize(System::block_number());
	Balances::on_finalize(System::block_number());
	System::set_block_number(System::block_number() + 1);
	Balances::on_initialize(System::block_number());
	Rewards::on_initialize(System::block_number());
}

#[test]
fn genesis_config_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Author::<Test>::get(), None);
		assert_eq!(Reward::<Test>::get(), 60);
		assert_eq!(Balances::free_balance(1), 0);
		assert_eq!(Balances::free_balance(2), 0);
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
		next_block();
		// User gets reward
		assert_eq!(Balances::free_balance(1), 54);

		// Set new reward
		assert_ok!(Rewards::set_reward(Origin::root(), 42));
		assert_ok!(Rewards::set_taxation(Origin::root(), Perbill::zero()));
		assert_ok!(Rewards::set_author(Origin::none(), 2, Perbill::zero()));
		next_block();
		assert_eq!(Balances::free_balance(2), 42);
	});
}
