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

use criterion::{criterion_group, criterion_main, Criterion};

use randomx::{FullVM, VM};

pub fn criterion_benchmark(c: &mut Criterion) {
	let mut vm = VM::new(&b"RandomX example key"[..]);
	let hash = vm.calculate(&b"RandomX example input"[..]);
	let mut full_vm = FullVM::new(&b"RandomX example key"[..]);

	c.bench_function("fullvm", |b| {
		b.iter(|| {
			let full_hash = full_vm.calculate(&b"RandomX example input"[..]);
			assert_eq!(hash, full_hash);
		})
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
