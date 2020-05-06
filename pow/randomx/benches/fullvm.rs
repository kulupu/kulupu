use criterion::{Criterion, criterion_group, criterion_main};

use randomx::{VM, FullVM};

pub fn criterion_benchmark(c: &mut Criterion) {
	let mut vm = VM::new(&b"RandomX example key"[..]);
	let hash = vm.calculate(&b"RandomX example input"[..]);
	let mut full_vm = FullVM::new(&b"RandomX example key"[..]);

    c.bench_function("fullvm", |b| b.iter(|| {
		let full_hash = full_vm.calculate(&b"RandomX example input"[..]);
		assert_eq!(hash, full_hash);
	}));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
