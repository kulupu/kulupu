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

mod v1;
mod v2;

pub use self::v1::{ComputeV1, SealV1};
pub use self::v2::{ComputeV2, SealV2};

use log::info;
use codec::{Encode, Decode};
use std::sync::{Arc, Mutex};
use std::cell::RefCell;
use sp_core::H256;
use lazy_static::lazy_static;
use lru_cache::LruCache;
use kulupu_randomx as randomx;
use kulupu_primitives::Difficulty;

lazy_static! {
	static ref FULL_SHARED_CACHES: Arc<Mutex<LruCache<H256, Arc<randomx::FullCache>>>> =
		Arc::new(Mutex::new(LruCache::new(2)));
	static ref LIGHT_SHARED_CACHES: Arc<Mutex<LruCache<H256, Arc<randomx::LightCache>>>> =
		Arc::new(Mutex::new(LruCache::new(3)));
}

thread_local! {
	static FULL_MACHINE: RefCell<Option<(H256, randomx::FullVM)>> = RefCell::new(None);
	static LIGHT_MACHINE: RefCell<Option<(H256, randomx::LightVM)>> = RefCell::new(None);
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Debug)]
pub enum ComputeMode {
	Sync,
	Mining,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Calculation {
	pub pre_hash: H256,
	pub difficulty: Difficulty,
	pub nonce: H256,
}

fn compute_raw_with_cache<M: randomx::WithCacheMode>(
	key_hash: &H256,
	input: &[u8],
	machine: &RefCell<Option<(H256, randomx::VM<M>)>>,
	shared_caches: &Arc<Mutex<LruCache<H256, Arc<randomx::Cache<M>>>>>,
) -> H256 {
	let mut ms = machine.borrow_mut();

	let need_new_vm = ms.as_ref().map(|(mkey_hash, _)| {
		mkey_hash != key_hash
	}).unwrap_or(true);

	if need_new_vm {
		let mut shared_caches = shared_caches.lock().expect("Mutex poisioned");

		if let Some(cache) = shared_caches.get_mut(key_hash) {
			*ms = Some((*key_hash, randomx::VM::new(cache.clone())));
		} else {
			info!("At block boundary, generating new RandomX cache with key hash {} ...",
				  key_hash);
			let cache = Arc::new(randomx::Cache::new(&key_hash[..]));
			shared_caches.insert(*key_hash, cache.clone());
			*ms = Some((*key_hash, randomx::VM::new(cache)));
		}
	}

	let work = ms.as_mut()
		.map(|(mkey_hash, vm)| {
			assert_eq!(mkey_hash, key_hash,
					   "Condition failed checking cached key_hash. This is a bug");
			vm.calculate(input)
		})
		.expect("Local MACHINES always set to Some above; qed");

	H256::from(work)
}

fn compute_raw(key_hash: &H256, input: &[u8], mode: ComputeMode) -> H256 {
	match mode {
		ComputeMode::Mining =>
			FULL_MACHINE.with(|machine| {
				compute_raw_with_cache::<randomx::WithFullCacheMode>(
					key_hash,
					input,
					machine,
					&FULL_SHARED_CACHES,
				)
			}),
		ComputeMode::Sync =>
			LIGHT_MACHINE.with(|machine| {
				compute_raw_with_cache::<randomx::WithLightCacheMode>(
					key_hash,
					input,
					machine,
					&LIGHT_SHARED_CACHES,
				)
			}),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::{H256, U256};

	#[test]
	fn randomx_len() {
		assert_eq!(randomx::HASH_SIZE, 32);
	}

	#[test]
	fn randomx_collision() {
		let mut compute = ComputeV1 {
			key_hash: H256::from([210, 164, 216, 149, 3, 68, 116, 1, 239, 110, 111, 48, 180, 102, 53, 180, 91, 84, 242, 90, 101, 12, 71, 70, 75, 83, 17, 249, 214, 253, 71, 89]),
			pre_hash: H256::default(),
			difficulty: U256::default(),
			nonce: H256::default(),
		};
		let hash1 = compute.clone().seal_and_work();
		U256::one().to_big_endian(&mut compute.nonce[..]);
		let hash2 = compute.seal_and_work();
		assert!(hash1.1 != hash2.1);

		let mut compute2 = ComputeV2 {
			key_hash: H256::from([210, 164, 216, 149, 3, 68, 116, 1, 239, 110, 111, 48, 180, 102, 53, 180, 91, 84, 242, 90, 101, 12, 71, 70, 75, 83, 17, 249, 214, 253, 71, 89]),
			pre_hash: H256::default(),
			difficulty: U256::default(),
			nonce: H256::default(),
		};
		let hash3 = compute2.clone().seal_and_work(Default::default());
		U256::one().to_big_endian(&mut compute2.nonce[..]);
		let hash4 = compute2.seal_and_work(Default::default());
		assert!(hash3.1 != hash4.1);
		assert!(hash1.1 != hash3.1);
		assert!(hash2.1 != hash4.1);
	}
}
