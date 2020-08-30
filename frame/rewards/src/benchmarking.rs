// Copyright 2020 Wei Tang.
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

//! Benchmarking for Rewards pallet.

use super::*;
use frame_system::{RawOrigin, EventRecord, DigestItemOf};
use frame_benchmarking::{benchmarks, account, whitelisted_caller};
use frame_support::traits::{OnInitialize, OnFinalize};
use sp_runtime::traits::Bounded;

fn assert_last_event<T: Trait>(generic_event: <T as Trait>::Event) {
	let events = frame_system::Module::<T>::events();
	let system_event: <T as frame_system::Trait>::Event = generic_event.into();
	// compare to the last event record
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

// This function creates a new lock on `who` every block for `num_of_locks`
// starting at block zero.
fn create_locks<T: Trait>(who: &T::AccountId, num_of_locks: u32) {
	let mut locks: BTreeMap<T::BlockNumber, BalanceOf<T>> = BTreeMap::new();
	let reward = T::Currency::minimum_balance();
	for i in 0 .. num_of_locks {
		locks.insert(i.into(), reward);
	}

	RewardLocks::<T>::insert(who, locks);
}

benchmarks! {
	_ { }

	note_author_prefs { }: _(RawOrigin::None, Perbill::from_percent(50))
	verify {
		assert!(AuthorDonation::exists());
	}

	set_reward {
		let new_reward = BalanceOf::<T>::max_value();
	}: _(RawOrigin::Root, new_reward)
	verify {
		assert_last_event::<T>(Event::<T>::RewardChanged(new_reward).into());
	}

	set_taxation {
		let new_taxation = Perbill::from_percent(50);
	}: _(RawOrigin::Root, new_taxation)
	verify {
		assert_last_event::<T>(Event::<T>::TaxationChanged(new_taxation).into());
	}

	unlock {
		let miner = account("miner", 0, 0);
		let max_locks = T::GenerateRewardLocks::max_locks();
		create_locks::<T>(&miner, max_locks);
		let caller = whitelisted_caller();
		frame_system::Module::<T>::set_block_number(max_locks.into());
		assert_eq!(RewardLocks::<T>::get(&miner).iter().count() as u32, max_locks);
	}: _(RawOrigin::Signed(caller), miner.clone())
	verify {
		assert_eq!(RewardLocks::<T>::get(&miner).iter().count(), 0);
	}

	on_initialize {
		let author: T::AccountId = account("author", 0, 0);
		let author_digest = DigestItemOf::<T>::PreRuntime(sp_consensus_pow::POW_ENGINE_ID, author.encode());
		frame_system::Module::<T>::deposit_log(author_digest);

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
		let donation = Perbill::from_percent(50);
		let reward = BalanceOf::<T>::max_value();
		let taxation = Perbill::from_percent(50);

		// Setup pallet variables
		Author::<T>::put(&author);
		AuthorDonation::put(donation);
		Reward::<T>::put(reward);
		Taxation::put(taxation);

		// Create existing locks on author.
		let max_locks = T::GenerateRewardLocks::max_locks();
		create_locks::<T>(&author, max_locks);

		// Move to a point where all locks would unlock.
		frame_system::Module::<T>::set_block_number(max_locks.into());
		assert_eq!(RewardLocks::<T>::get(&author).iter().count() as u32, max_locks);

		// Whitelist transient storage items
		frame_benchmarking::benchmarking::add_to_whitelist(Author::<T>::hashed_key().to_vec().into());
		frame_benchmarking::benchmarking::add_to_whitelist(AuthorDonation::hashed_key().to_vec().into());

		let block_number = frame_system::Module::<T>::block_number();
	}: { crate::Module::<T>::on_finalize(block_number); }
	verify {
		assert!(Author::<T>::get().is_none());
		assert!(AuthorDonation::get().is_none());
		assert!(RewardLocks::<T>::get(&author).iter().count() > 0);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext(0).execute_with(|| {
			assert_ok!(test_benchmark_note_author_prefs::<Test>());
			assert_ok!(test_benchmark_set_reward::<Test>());
			assert_ok!(test_benchmark_set_taxation::<Test>());
			assert_ok!(test_benchmark_unlock::<Test>());
			assert_ok!(test_benchmark_on_initialize::<Test>());
			assert_ok!(test_benchmark_on_finalize::<Test>());
		});
	}
}
