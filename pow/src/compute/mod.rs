// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2020 Wei Tang.
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

mod v1;
mod v2;

pub use self::{
	v1::{ComputeV1, SealV1},
	v2::{ComputeV2, SealV2},
};
pub use randomx::{Config, Error as RandomxError};

use codec::{Decode, Encode};
use kulupu_primitives::Difficulty;
use kulupu_randomx as randomx;
use lazy_static::lazy_static;
use log::info;
use lru_cache::LruCache;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use randomx::WithCacheMode;
use sp_core::H256;
use std::{cell::RefCell, sync::Arc};

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

static GLOBAL_CONFIG: OnceCell<Config> = OnceCell::new();
static DEFAULT_CONFIG: Config = Config::new();

#[derive(Debug)]
pub enum Error {
	CacheNotAvailable,
	Randomx(RandomxError),
}

impl Error {
	pub fn description(&self) -> &'static str {
		match self {
			Error::Randomx(e) => e.description(),
			Error::CacheNotAvailable => "Randomx cache not available",
		}
	}
}

impl From<Error> for String {
	fn from(e: Error) -> Self {
		e.description().to_string()
	}
}

pub fn global_config() -> &'static Config {
	GLOBAL_CONFIG.get().unwrap_or(&DEFAULT_CONFIG)
}

pub fn set_global_config(config: Config) -> Result<(), Config> {
	GLOBAL_CONFIG.set(config)
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Debug)]
pub enum ComputeMode {
	Sync,
	Mining,
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Debug)]
pub enum Loop<R> {
	Continue,
	Break(R),
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Calculation {
	pub pre_hash: H256,
	pub difficulty: Difficulty,
	pub nonce: H256,
}

impl From<RandomxError> for Error {
	fn from(e: RandomxError) -> Self {
		Error::Randomx(e)
	}
}

fn need_new_vm<M: randomx::WithCacheMode>(
	key_hash: &H256,
	machine: &RefCell<Option<(H256, randomx::VM<M>)>>,
) -> bool {
	let ms = machine.borrow();

	let need_new_vm = ms
		.as_ref()
		.map(|(mkey_hash, _)| mkey_hash != key_hash)
		.unwrap_or(true);

	need_new_vm
}

fn do_new_vm<M: randomx::WithCacheMode>(
	key_hash: &H256,
	machine: &RefCell<Option<(H256, randomx::VM<M>)>>,
	shared_caches: &Mutex<LruCache<H256, Arc<randomx::Cache<M>>>>,
	f_has_large_pages: fn(&Config) -> bool,
) -> Result<(), Error> {
	let mut shared_caches = shared_caches.lock();

	if !f_has_large_pages(global_config()) {
		if let Some(cache) = shared_caches.get_mut(key_hash) {
			machine.replace(Some((
				*key_hash,
				randomx::VM::new(cache.clone(), global_config()),
			)));

			Ok(())
		} else {
			info!(
				target: "kulupu-randomx",
				"At block boundary, generating new RandomX {} cache with key hash {} ...",
				M::description(),
				key_hash,
			);

			let cache = Arc::new(randomx::Cache::new(&key_hash[..], global_config())?);

			shared_caches.insert(*key_hash, cache.clone());
			machine.replace(Some((*key_hash, randomx::VM::new(cache, global_config()))));

			Ok(())
		}
	} else {
		if let Some(cache) = shared_caches.get_mut(key_hash) {
			machine.replace(Some((
				*key_hash,
				randomx::VM::new(cache.clone(), global_config()),
			)));

			Ok(())
		} else {
			let info = format!(
				"At block boundary, generating new RandomX {} cache with key hash {} ...",
				M::description(),
				key_hash,
			);

			if shared_caches.is_empty() {
				info!(target: "kulupu-randomx", "{}", info);
				let cache = Arc::new(randomx::Cache::new(&key_hash[..], global_config())?);

				shared_caches.insert(*key_hash, cache.clone());
				machine.replace(Some((*key_hash, randomx::VM::new(cache, global_config()))));

				Ok(())
			} else {
				machine.replace(None);
				let key_to_replace = (*shared_caches)
					.iter()
					.find(|&(_, cache)| Arc::strong_count(cache) == 1)
					.and_then(|(key, _)| Some(*key))
					.ok_or(Error::CacheNotAvailable)?;

				info!(target: "kulupu-randomx", "{}", info);
				let mut cache = shared_caches
					.remove(&key_to_replace)
					.expect("That key should still be in the lru cache.");

				Arc::get_mut(&mut cache)
					.expect("The mutable reference should be available as strong_count is 1.")
					.reinit(&key_hash[..]);
				shared_caches.insert(*key_hash, cache.clone());
				machine.replace(Some((*key_hash, randomx::VM::new(cache, global_config()))));

				Ok(())
			}
		}
	}
}

fn loop_raw_with_cache<M: randomx::WithCacheMode, FPre, I, FValidate, R>(
	key_hash: &H256,
	machine: &RefCell<Option<(H256, randomx::VM<M>)>>,
	shared_caches: &Arc<Mutex<LruCache<H256, Arc<randomx::Cache<M>>>>>,
	mut f_pre: FPre,
	f_validate: FValidate,
	f_has_large_pages: fn(&Config) -> bool,
	round: usize,
) -> Result<Option<R>, Error>
where
	FPre: FnMut() -> (Vec<u8>, I),
	FValidate: Fn(H256, I) -> Loop<Option<R>>,
{
	if need_new_vm(key_hash, machine) {
		do_new_vm(key_hash, machine, shared_caches, f_has_large_pages)?
	}

	let mut ms = machine.borrow_mut();

	let ret = ms
		.as_mut()
		.map(|(mkey_hash, vm)| {
			assert_eq!(
				mkey_hash, key_hash,
				"Condition failed checking cached key_hash. This is a bug"
			);

			let mut ret = None;

			match round {
				0 => (),
				1 => {
					let (pre, int) = f_pre();
					let hash = H256::from(vm.calculate(&pre[..]));
					let validate = f_validate(hash, int);

					match validate {
						Loop::Continue => (),
						Loop::Break(b) => {
							ret = b;
						}
					}
				}
				_ => {
					let (prev_pre, mut prev_int) = f_pre();
					let mut vmn = vm.begin(&prev_pre[..]);

					for _ in 1..round {
						let (pre, int) = f_pre();
						let prev_hash = H256::from(vmn.next(&pre[..]));
						let prev_validate = f_validate(prev_hash, prev_int);

						prev_int = int;

						match prev_validate {
							Loop::Continue => (),
							Loop::Break(b) => {
								ret = b;
								break;
							}
						}
					}

					let prev_hash = H256::from(vmn.finish());
					let prev_validate = f_validate(prev_hash, prev_int);

					match prev_validate {
						Loop::Continue => (),
						Loop::Break(b) => {
							ret = b;
						}
					}
				}
			}

			ret
		})
		.expect("Local MACHINES always set to Some above; qed");

	Ok(ret)
}

pub fn loop_raw<FPre, I, FValidate, R>(
	key_hash: &H256,
	mode: ComputeMode,
	f_pre: FPre,
	f_validate: FValidate,
	round: usize,
) -> Result<Option<R>, Error>
where
	FPre: FnMut() -> (Vec<u8>, I),
	FValidate: Fn(H256, I) -> Loop<Option<R>>,
{
	match mode {
		ComputeMode::Mining => FULL_MACHINE.with(|machine| {
			loop_raw_with_cache::<randomx::WithFullCacheMode, _, _, _, _>(
				key_hash,
				machine,
				&FULL_SHARED_CACHES,
				f_pre,
				f_validate,
				randomx::WithFullCacheMode::has_large_pages,
				round,
			)
		}),
		ComputeMode::Sync => {
			let full_ret = FULL_MACHINE.with(|machine| {
				if !need_new_vm::<randomx::WithFullCacheMode>(key_hash, machine) {
					Ok(
						loop_raw_with_cache::<randomx::WithFullCacheMode, _, _, _, _>(
							key_hash,
							machine,
							&FULL_SHARED_CACHES,
							f_pre,
							f_validate,
							randomx::WithFullCacheMode::has_large_pages,
							round,
						),
					)
				} else {
					Err((f_pre, f_validate))
				}
			});

			match full_ret {
				Ok(ret) => ret,
				Err((f_pre, f_validate)) => LIGHT_MACHINE.with(|machine| {
					loop_raw_with_cache::<randomx::WithLightCacheMode, _, _, _, _>(
						key_hash,
						machine,
						&LIGHT_SHARED_CACHES,
						f_pre,
						f_validate,
						randomx::WithLightCacheMode::has_large_pages,
						round,
					)
				}),
			}
		}
	}
}

pub fn compute<T: Encode>(key_hash: &H256, input: &T, mode: ComputeMode) -> Result<H256, Error> {
	Ok(loop_raw(
		key_hash,
		mode,
		|| (input.encode(), ()),
		|hash, ()| Loop::Break(Some(hash)),
		1,
	)?
	.expect("Loop break always returns Some; qed"))
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
	fn randomx_collision() -> Result<(), String> {
		let mut compute = ComputeV1 {
			key_hash: H256::from([
				210, 164, 216, 149, 3, 68, 116, 1, 239, 110, 111, 48, 180, 102, 53, 180, 91, 84,
				242, 90, 101, 12, 71, 70, 75, 83, 17, 249, 214, 253, 71, 89,
			]),
			pre_hash: H256::default(),
			difficulty: U256::default(),
			nonce: H256::default(),
		};
		let hash1 = compute.clone().seal_and_work(ComputeMode::Sync)?;
		U256::one().to_big_endian(&mut compute.nonce[..]);
		let hash2 = compute.seal_and_work(ComputeMode::Sync)?;
		assert!(hash1.1 != hash2.1);

		let mut compute2 = ComputeV2 {
			key_hash: H256::from([
				210, 164, 216, 149, 3, 68, 116, 1, 239, 110, 111, 48, 180, 102, 53, 180, 91, 84,
				242, 90, 101, 12, 71, 70, 75, 83, 17, 249, 214, 253, 71, 89,
			]),
			pre_hash: H256::default(),
			difficulty: U256::default(),
			nonce: H256::default(),
		};
		let hash3 = compute2
			.clone()
			.seal_and_work(Default::default(), ComputeMode::Sync)?;
		U256::one().to_big_endian(&mut compute2.nonce[..]);
		let hash4 = compute2.seal_and_work(Default::default(), ComputeMode::Sync)?;
		assert!(hash3.1 != hash4.1);
		assert!(hash1.1 != hash3.1);
		assert!(hash2.1 != hash4.1);

		Ok(())
	}
}
