use std::f64::consts::{E, PI};
use std::ffi::c_void;
use std::fs::File;
use std::hint::black_box;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;
use std::{env, str, thread, vec};

extern "C" {
	pub fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
	pub fn memset(dest: *mut c_void, c: u8, n: usize) -> *mut c_void;
}

const NR_RUNS: usize = 1000;

#[cfg(target_arch = "x86_64")]
#[inline]
fn get_timestamp() -> u64 {
	unsafe {
		let mut _aux = 0;
		let value = core::arch::x86_64::__rdtscp(&mut _aux);
		core::arch::x86_64::_mm_lfence();
		value
	}
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn get_timestamp() -> u64 {
	use aarch64_cpu::registers::{Readable, CNTPCT_EL0};

	CNTPCT_EL0.get()
}

#[cfg(target_arch = "riscv64")]
#[inline]
fn get_timestamp() -> u64 {
	riscv::register::time::read64()
}

extern "C" {
	#[cfg(target_os = "hermit")]
	fn sys_getpid() -> u32;
}

pub fn bench_syscall() -> Result<(), ()> {
	let n = 1000000;

	let ticks = unsafe {
		// cache warmup
		#[cfg(target_os = "hermit")]
		let _ = sys_getpid();
		#[cfg(target_os = "linux")]
		let _ = syscalls::syscall!(syscalls::Sysno::getpid);
		let _ = get_timestamp();

		let start = get_timestamp();
		for _ in 0..n {
			#[cfg(target_os = "hermit")]
			let _ = sys_getpid();
			#[cfg(target_os = "linux")]
			let _ = syscalls::syscall!(syscalls::Sysno::getpid);
		}
		get_timestamp() - start
	};

	println!("Time {} for a system call (in ticks)", ticks / n);

	Ok(())
}

pub fn bench_sched_one_thread() -> Result<(), ()> {
	let n = 1000000;

	// cache warmup
	thread::yield_now();
	thread::yield_now();
	let _ = get_timestamp();

	let start = get_timestamp();
	for _ in 0..n {
		thread::yield_now();
	}
	let ticks = get_timestamp() - start;

	println!("Scheduling time {} ticks (1 thread)", ticks / n);

	Ok(())
}

pub fn bench_sched_two_threads() -> Result<(), ()> {
	let n = 1000000;
	let nthreads = 2;

	// cache warmup
	thread::yield_now();
	thread::yield_now();
	let _ = get_timestamp();

	let start = get_timestamp();
	let threads: Vec<_> = (0..nthreads - 1)
		.map(|_| {
			thread::spawn(move || {
				for _ in 0..n {
					thread::yield_now();
				}
			})
		})
		.collect();

	for _ in 0..n {
		thread::yield_now();
	}

	let ticks = get_timestamp() - start;

	for t in threads {
		t.join().unwrap();
	}

	println!(
		"Scheduling time {} ticks (2 threads)",
		ticks / (nthreads * n)
	);

	Ok(())
}

// derived from
// https://github.com/rust-lang/compiler-builtins/blob/master/testcrate/benches/mem.rs
fn memcpy_builtin(n: usize) {
	let v1 = vec![1u8; n];
	let mut v2 = vec![0u8; n];

	let now = Instant::now();
	for _i in 0..NR_RUNS {
		let src: &[u8] = black_box(&v1);
		let dst: &mut [u8] = black_box(&mut v2);
		dst.copy_from_slice(src);
	}

	println!(
		"memcpy_builtin:  {} block size, {} MByte/s",
		n,
		(NR_RUNS * n) as f64 / (1024.0 * 1024.0 * now.elapsed().as_secs_f64())
	);
}

// derived from
// https://github.com/rust-lang/compiler-builtins/blob/master/testcrate/benches/mem.rs
fn memset_builtin(n: usize) {
	let mut v1 = vec![0u8; n];
	let now = Instant::now();
	for _i in 0..NR_RUNS {
		let dst: &mut [u8] = black_box(&mut v1);
		let val: u8 = black_box(27);
		for b in dst {
			*b = val;
		}
	}

	println!(
		"memset_builtin:  {} block, {} MByte/s",
		n,
		((NR_RUNS * n) >> 20) as f64 / now.elapsed().as_secs_f64()
	);
}

// derived from
// https://github.com/rust-lang/compiler-builtins/blob/master/testcrate/benches/mem.rs
fn memcpy_rust(n: usize) {
	let v1 = vec![1u8; n];
	let mut v2 = vec![0u8; n];
	let now = Instant::now();
	for _i in 0..NR_RUNS {
		let src: &[u8] = black_box(&v1[0..]);
		let dst: &mut [u8] = black_box(&mut v2[0..]);
		unsafe {
			memcpy(
				dst.as_mut_ptr() as *mut c_void,
				src.as_ptr() as *mut c_void,
				n,
			);
		}
	}

	println!(
		"memcpy_rust:  {} block, {} MByte/s",
		n,
		((NR_RUNS * n) >> 20) as f64 / now.elapsed().as_secs_f64()
	);
}

// derived from
// https://github.com/rust-lang/compiler-builtins/blob/master/testcrate/benches/mem.rs
fn memset_rust(n: usize) {
	let mut v1 = vec![0u8; n];
	let now = Instant::now();
	for _i in 0..NR_RUNS {
		let dst: &mut [u8] = black_box(&mut v1[0..]);
		let val = black_box(27);
		unsafe {
			memset(dst.as_mut_ptr() as *mut c_void, val, n);
		}
	}

	println!(
		"memset_rust:  {} block, {} MByte/s",
		n,
		((NR_RUNS * n) >> 20) as f64 / now.elapsed().as_secs_f64()
	);
}

pub fn bench_mem() -> Result<(), ()> {
	memcpy_builtin(4096);
	memcpy_builtin(1048576);
	memcpy_builtin(16 * 1048576);
	memset_builtin(4096);
	memset_builtin(1048576);
	memset_builtin(16 * 1048576);
	memcpy_rust(4096);
	memcpy_rust(1048576);
	memcpy_rust(16 * 1048576);
	memset_rust(4096);
	memset_rust(1048576);
	memset_rust(16 * 1048576);

	Ok(())
}
