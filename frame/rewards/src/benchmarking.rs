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
use frame_system::{RawOrigin, EventRecord};
use frame_benchmarking::{benchmarks, account, whitelisted_caller};
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
		frame_system::Module::<T>::set_block_number(T::BlockNumber::max_value());
		assert_eq!(RewardLocks::<T>::get(&miner).iter().count() as u32, max_locks);
	}: _(RawOrigin::Signed(caller), miner.clone())
	verify {
		assert_eq!(RewardLocks::<T>::get(&miner).iter().count(), 0);
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
		});
	}
}
