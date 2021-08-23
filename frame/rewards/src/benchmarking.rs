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

//! Benchmarking for Rewards pallet.

use super::*;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::{DigestItemOf, EventRecord, RawOrigin};
use sp_runtime::traits::Bounded;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	let events = frame_system::Module::<T>::events();
	let system_event: <T as frame_system::Config>::Event = generic_event.into();
	// compare to the last event record
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

// This function creates a new lock on `who` every block for `num_of_locks`
// starting at block zero.
fn create_locks<T: Config>(who: &T::AccountId, num_of_locks: u32) {
	let mut locks: BTreeMap<T::BlockNumber, BalanceOf<T>> = BTreeMap::new();
	let reward = T::Currency::minimum_balance();
	for i in 0..num_of_locks {
		locks.insert(i.into(), reward);
	}

	RewardLocks::<T>::insert(who, locks);
}

benchmarks! {
	// Worst case: Author info is in digest.
	on_initialize {
		let author: T::AccountId = account("author", 0, 0);
		let author_digest = DigestItemOf::<T>::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		frame_system::Module::<T>::deposit_log(author_digest);

		Reward::<T>::put(T::Currency::minimum_balance());

		// Whitelist transient storage items
		frame_benchmarking::benchmarking::add_to_whitelist(Author::<T>::hashed_key().to_vec().into());

		let block_number = frame_system::Module::<T>::block_number();
	}: { crate::Module::<T>::on_initialize(block_number); }
	verify {
		assert_eq!(Author::<T>::get(), Some(author));
	}

	// Worst case: This author already has `max_locks` locked up, produces a new block, and we unlock
	// everything in addition to creating brand new locks for the new reward.
	on_finalize {
		let author: T::AccountId = account("author", 0, 0);
		let reward = BalanceOf::<T>::max_value();

		// Setup pallet variables
		Author::<T>::put(&author);
		Reward::<T>::put(reward);

		// Create existing locks on author.
		let max_locks = T::GenerateRewardLocks::max_locks(T::LockParametersBounds::get());
		create_locks::<T>(&author, max_locks);

		// Move to a point where all locks would unlock.
		frame_system::Module::<T>::set_block_number(max_locks.into());
		assert_eq!(RewardLocks::<T>::get(&author).iter().count() as u32, max_locks);

		// Whitelist transient storage items
		frame_benchmarking::benchmarking::add_to_whitelist(Author::<T>::hashed_key().to_vec().into());

		let block_number = frame_system::Module::<T>::block_number();
	}: { crate::Module::<T>::on_finalize(block_number); }
	verify {
		assert!(Author::<T>::get().is_none());
		assert!(RewardLocks::<T>::get(&author).iter().count() > 0);
	}

	// Worst case: Target user has `max_locks` which are all unlocked during this call.
	unlock {
		let miner = account("miner", 0, 0);
		let max_locks = T::GenerateRewardLocks::max_locks(T::LockParametersBounds::get());
		create_locks::<T>(&miner, max_locks);
		let caller = whitelisted_caller();
		frame_system::Module::<T>::set_block_number(max_locks.into());
		assert_eq!(RewardLocks::<T>::get(&miner).iter().count() as u32, max_locks);
	}: _(RawOrigin::Signed(caller), miner.clone())
	verify {
		assert_eq!(RewardLocks::<T>::get(&miner).iter().count(), 0);
	}

	set_schedule {

	}: _(RawOrigin::Root, T::Currency::minimum_balance(), BTreeMap::new(), BTreeMap::new(), BTreeMap::new())

	// Worst case: a new lock params is set.
	set_lock_params {

	}: _(RawOrigin::Root, LockParameters {period: 150, divide: 25} )
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext(0).execute_with(|| {
			assert_ok!(test_benchmark_on_finalize::<Test>());
			assert_ok!(test_benchmark_on_initialize::<Test>());
			assert_ok!(test_benchmark_unlock::<Test>());
			assert_ok!(test_benchmark_set_schedule::<Test>());
			assert_ok!(test_benchmark_set_lock_params::<Test>());
		});
	}
}
