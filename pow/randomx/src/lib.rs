// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Kulupu.
//
// Copyright (c) 2019-2020 Wei Tang.
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

use std::sync::Arc;
use std::marker::PhantomData;

pub const HASH_SIZE: usize = sys::RANDOMX_HASH_SIZE as usize;

pub enum CacheMode {
	Full,
	Light,
}

pub unsafe trait WithCacheMode {
	fn has_dataset() -> bool;
	fn randomx_flags() -> sys::randomx_flags;
	fn description() -> &'static str;
}

pub enum WithFullCacheMode { }
unsafe impl WithCacheMode for WithFullCacheMode {
	fn has_dataset() -> bool { true }
	fn randomx_flags() -> sys::randomx_flags {
		unsafe {
			sys::randomx_get_flags() |
			sys::randomx_flags_RANDOMX_FLAG_FULL_MEM
		}

	}
	fn description() -> &'static str { "full" }
}

pub enum WithLightCacheMode { }
unsafe impl WithCacheMode for WithLightCacheMode {
	fn has_dataset() -> bool { false }
	fn randomx_flags() -> sys::randomx_flags {
		unsafe { sys::randomx_get_flags() }
	}
	fn description() -> &'static str { "light" }
}

pub struct Cache<M: WithCacheMode> {
	cache_ptr: *mut sys::randomx_cache,
	dataset_ptr: Option<*mut sys::randomx_dataset>,
	_marker: PhantomData<M>,
}

pub type FullCache = Cache<WithFullCacheMode>;
pub type LightCache = Cache<WithLightCacheMode>;

unsafe impl<M: WithCacheMode> Send for Cache<M> { }
unsafe impl<M: WithCacheMode> Sync for Cache<M> { }

impl<M: WithCacheMode> Cache<M> {
	pub fn new(key: &[u8]) -> Self {
		let flags = M::randomx_flags();

		let cache_ptr = unsafe {
			let ptr = sys::randomx_alloc_cache(flags);
			sys::randomx_init_cache(
				ptr,
				key.as_ptr() as *const std::ffi::c_void,
				key.len() as u64
			);

			ptr
		};

		let dataset_ptr = if M::has_dataset() {
			Some(unsafe {
				let ptr = sys::randomx_alloc_dataset(flags);
				let count = sys::randomx_dataset_item_count();
				sys::randomx_init_dataset(ptr, cache_ptr, 0, count);
				ptr
			})
		} else {
			None
		};

		Self { cache_ptr, dataset_ptr, _marker: PhantomData }
	}
}

impl<M: WithCacheMode> Drop for Cache<M> {
	fn drop(&mut self) {
		unsafe {
			sys::randomx_release_cache(self.cache_ptr);
		}

		if M::has_dataset() {
			unsafe {
				sys::randomx_release_dataset(self.dataset_ptr.expect("Dataset was created"));
			}
		}
	}
}

pub struct VM<M: WithCacheMode> {
	_cache: Arc<Cache<M>>,
	ptr: *mut sys::randomx_vm,
}

pub type FullVM = VM<WithFullCacheMode>;
pub type LightVM = VM<WithLightCacheMode>;

impl<M: WithCacheMode> VM<M> {
	pub fn new(cache: Arc<Cache<M>>) -> Self {
		let flags = M::randomx_flags();

		let ptr = unsafe {
			sys::randomx_create_vm(
				flags,
				cache.cache_ptr,
				cache.dataset_ptr.unwrap_or(std::ptr::null_mut()),
			)
		};

		Self { _cache: cache, ptr }
	}

	pub fn calculate(&mut self, input: &[u8]) -> [u8; HASH_SIZE] {
		let ret = [0u8; HASH_SIZE];

		unsafe {
			sys::randomx_calculate_hash(
				self.ptr,
				input.as_ptr() as *const std::ffi::c_void,
				input.len() as u64,
				ret.as_ptr() as *mut std::ffi::c_void,
			);
		}

		ret
	}

	pub fn begin<'a>(&'a mut self, input: &[u8]) -> Next<'a, M> {
		unsafe {
			sys::randomx_calculate_hash_first(
				self.ptr,
				input.as_ptr() as *const std::ffi::c_void,
				input.len() as u64,
			);
		}

		Next {
			inner: self,
		}
	}
}

impl<M: WithCacheMode> Drop for VM<M> {
	fn drop(&mut self) {
		unsafe {
			sys::randomx_destroy_vm(self.ptr);
		}
	}
}

pub struct Next<'a, M: WithCacheMode> {
	inner: &'a mut VM<M>,
}

impl<'a, M: WithCacheMode> Next<'a, M> {
	pub fn next(&mut self, input: &[u8]) -> [u8; HASH_SIZE] {
		let ret = [0u8; HASH_SIZE];

		unsafe {
			sys::randomx_calculate_hash_next(
				self.inner.ptr,
				input.as_ptr() as *const std::ffi::c_void,
				input.len() as u64,
				ret.as_ptr() as *mut std::ffi::c_void,
			);
		}

		ret
	}

	pub fn finish(self) -> [u8; HASH_SIZE] {
		let ret = [0u8; HASH_SIZE];

		unsafe {
			sys::randomx_calculate_hash_last(
				self.inner.ptr,
				ret.as_ptr() as *mut std::ffi::c_void,
			);
		}

		ret
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn should_create_light_vm() {
		let cache = Arc::new(LightCache::new(&b"RandomX example key"[..]));
		let mut vm = LightVM::new(cache);
		let hash = vm.calculate(&b"RandomX example input"[..]);
		assert_eq!(hash, [69, 167, 169, 170, 66, 104, 77, 15, 73, 13, 233, 6, 227, 92, 143, 244, 95, 153, 4, 251, 223, 169, 78, 126, 236, 216, 174, 147, 1, 213, 223, 59]);
	}

	#[test]
	fn should_work_with_full_vm() {
		let light_cache = Arc::new(LightCache::new(&b"RandomX example key"[..]));
		let mut light_vm = LightVM::new(light_cache);
		let hash = light_vm.calculate(&b"RandomX example input"[..]);
		let full_cache = Arc::new(FullCache::new(&b"RandomX example key"[..]));
		let mut full_vm = FullVM::new(full_cache);
		let full_hash = full_vm.calculate(&b"RandomX example input"[..]);
		assert_eq!(hash, full_hash);
	}
}
