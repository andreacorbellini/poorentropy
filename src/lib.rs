// Copyright © 2024 Andrea Corbellini and contributors
// SPDX-License-Identifier: BSD-2-Clause

//! Low-quality entropy generator for `no_std` crates
//!
//! This crate provides a reliable entropy source to crates that cannot depend on the [Rust
//! standard library](https://doc.rust-lang.org/std/). The design goals for this crate are
//! simplicity and ease of use.
//!
//! The entropy generated is not suitable for security or cryptography (see
//! [Limitations](#limitations) and [How It Works](#how-it-works) below for details), although it
//! may be combined with other entropy sources to produce a higher security level. This crate may
//! be used for testing or for all sort of randomizations where security is not a constraint.
//!
//! # Highlights
//!
//! - Zero external dependencies
//! - No dependency on the [Rust standard library](https://doc.rust-lang.org/std/): this is a
//!   `no_std` crate
//! - Works on all modern architectures: x86, AArch64, RISC-V 64, LoongArch64
//!
//! # Usage and Examples
//!
//! [`get()`] returns some entropy as a [`u64`]:
//!
//! ```
//! let e = poorentropy::get();
//! # let _ = e;
//! ```
//!
//! [`fill()`] and [`bytes()`] can be used to obtain the entropy as bytes.
//!
//! Generally speaking, entropy sources should not be used directly, but should rather be used as a
//! seed for a pseudo-random number generator. Here is an example using the [`rand`
//! crate](https://crates.io/crates/rand):
//!
//! ```
//! use rand::RngCore;
//! use rand::SeedableRng;
//! use rand::rngs::SmallRng;
//!
//! let mut seed = <SmallRng as SeedableRng>::Seed::default();
//! poorentropy::fill(&mut seed);
//! let mut rng = SmallRng::from_seed(seed);
//!
//! // Use the `rng`...
//! let r = rng.next_u32();
//! # let _ = r;
//! ```
//!
//! # How It Works
//!
//! The crate works by reading the CPU "clock" or "cycle counter", and mixing it to produce a
//! pseudo-random value.
//!
//! This table describes how the CPU clock/counter value is obtained for each supported
//! architecture:
//!
//! | Target        | Source       |
//! |---------------|--------------|
//! | AArch64       | `cntvct_el0` |
//! | LoongArch64   | `rdtime.d`   |
//! | RISC-V 64     | `rdcycle`    |
//! | x86 / x86\_64 | `rdtsc`      |
//!
//! The value obtained from the CPU is also added to an internal counter, with the goal to avoid
//! returning the same entropy values to concurrent threads that call [`get()`] at the same time.
//!
//! The resulting value is then fed into the
//! [SplitMix64](https://en.wikipedia.org/wiki/Xorshift#Initialization) generator to make it appear
//! random.
//!
//! # Limitations
//!
//! * Because the crate relies on the CPU clock, the values that it produces may be easy to
//!   predict. As such, this crate alone may not be used for security applications, because
//!   attackers may be able to guess the entropy values produced. It can however be combined with
//!   other entropy sources to increase the security level.
//!
//! * This crate tries to make it hard for two threads in the same process to obtain the same
//!   entropy value. However no guarantees are made for two distinct processes.
//!
//! * On some CPUs, the clock may start from a fixed value at boot. As such, if this crates is used
//!   in applications that run early on boot (such as firmwares, bootloaders, or kernels), it is
//!   possible that the crate will yield the same entropy values at every boot.
//!
//! * On some CPUs, the clock may be disabled or reset at runtime, and this may result in very
//!   low-quality entropy values being returned.

#![no_std]

#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused_qualifications)]
#![doc(test(attr(deny(warnings))))]

pub mod iter;

use core::arch::asm;
use core::cmp::min;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;

#[inline(always)]
#[cfg(target_arch = "aarch64")]
fn cpu_counter() -> u64 {
    let cnt: u64;
    // https://developer.arm.com/documentation/ddi0595/2021-03/AArch64-Registers/CNTVCT-EL0--Counter-timer-Virtual-Count-register
    //
    // > Virtual count value.
    // >
    // > On a Warm reset, this field resets to an architecturally UNKNOWN value.
    //
    // This counter is updated at a low frequency, so it's *very likely* that subsequent calls to
    // `cpu_counter()` will return the same value more than once. This is compensated by the use of
    // `internal_counter()` in `get()`.
    unsafe {
        asm!(
            "mrs {cnt}, cntvct_el0",
            cnt = out(reg) cnt,
            options(nomem, nostack, preserves_flags),
        );
    }
    cnt
}

#[inline(always)]
#[cfg(target_arch = "loongarch64")]
fn cpu_counter() -> u64 {
    let cnt: u64;
    // https://loongson.github.io/LoongArch-Documentation/LoongArch-Vol1-EN#_rdtimelh_w_rdtime_d
    //
    // > The LoongArch instruction system defines-a constant frequency timer, whose main body is-a
    // > 64-bit counter called StableCounter. StableCounter is set to 0 after reset, and then
    // > increments by 1 every counting clock cycle. When the count reaches all 1s, it automatically
    // > wraps around to 0 and continues to increment.
    //
    // `rdtime.d` returns two values: the counter value (64 bits), and the counter id. We don't
    // care about the id, so we discard it.
    unsafe {
        asm!(
            "rdtime.d {cnt}, {id}",
            cnt = out(reg) cnt,
            id = out(reg) _,
            options(nomem, nostack, preserves_flags),
        );
    }
    cnt
}

#[inline(always)]
#[cfg(target_arch = "riscv64")]
fn cpu_counter() -> u64 {
    let cnt: u64;
    // https://riscv.org/wp-content/uploads/2016/06/riscv-spec-v2.1.pdf
    //
    // > The RDCYCLE pseudo-instruction reads the low XLEN bits of the cycle CSR which holds a
    // > count of the number of clock cycles executed by the processor on which the hardware thread
    // > is running from an arbitrary start time in the past.
    unsafe {
        asm!(
            "rdcycle {cnt}",
            cnt = out(reg) cnt,
            options(nomem, nostack, preserves_flags),
        );
    }
    cnt
}

#[inline(always)]
#[cfg(target_arch = "x86")]
fn cpu_counter() -> u64 {
    let mut cnt_hi: u32;
    let mut cnt_lo: u32;
    // https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2b-manual.pdf
    //
    // > Reads the current value of the processor’s time-stamp counter (a 64-bit MSR) into the
    // > EDX:EAX registers. The EDX register is loaded with the high-order 32 bits of the MSR and
    // > the EAX register is loaded with the low-order 32 bits. (On processors that support the
    // > Intel 64 architecture, the high-order 32 bits of each of RAX and RDX are cleared.)
    unsafe {
        asm!(
            "rdtsc",
            out("eax") cnt_lo,
            out("edx") cnt_hi,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((cnt_hi as u64) << 32) | cnt_lo as u64
}

#[inline(always)]
#[cfg(target_arch = "x86_64")]
fn cpu_counter() -> u64 {
    let mut cnt_hi: u64;
    let mut cnt_lo: u64;
    // https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2b-manual.pdf
    //
    // > Reads the current value of the processor’s time-stamp counter (a 64-bit MSR) into the
    // > EDX:EAX registers. The EDX register is loaded with the high-order 32 bits of the MSR and
    // > the EAX register is loaded with the low-order 32 bits. (On processors that support the
    // > Intel 64 architecture, the high-order 32 bits of each of RAX and RDX are cleared.)
    unsafe {
        asm!(
            "rdtsc",
            out("rax") cnt_lo,
            out("rdx") cnt_hi,
            options(nomem, nostack, preserves_flags),
        );
    }
    (cnt_hi << 32) | cnt_lo
}

#[inline(always)]
fn internal_counter() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[inline]
fn split_mix_64(state: u64) -> u64 {
    let mut z = state.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z = z ^ (z >> 31);
    z
}

/// Returns a pseudo-random value as a [`u64`].
///
/// # Examples
///
/// ```
/// let a = poorentropy::get();
/// let b = poorentropy::get();
/// assert_ne!(a, b);
/// ```
#[must_use]
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "loongarch64",
    target_arch = "riscv64",
    target_arch = "x86",
    target_arch = "x86_64"
))]
pub fn get() -> u64 {
    // Get the clock/tick counter from the CPU (`cpu_counter()`), and then add an atomic monotonic
    // counter to it (`internal_counter()`). The atomic monotonic counter serves two purposes:
    //
    // 1. it helps ensuring that if two threads call `get()` at the same time, they will see
    //    different values;
    // 2. work around the limitation on some architectures (ARM, AArch64) where the clock updates
    //    at a low frequency, therefore subsequent calls to `cpu_counter()` are *very likely* to
    //    return the same value.
    let cnt = cpu_counter().wrapping_add(internal_counter());
    // Use a pseudo-random number generator to make the output look random.
    split_mix_64(cnt)
}

/// Fills a byte buffer with pseudo-random bytes.
///
/// # Examples
///
/// ```
/// let mut buf = [0u8; 30];
/// poorentropy::fill(&mut buf);
/// assert_ne!(buf, [0u8; 30]);
/// ```
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "loongarch64",
    target_arch = "riscv64",
    target_arch = "x86",
    target_arch = "x86_64"
))]
pub fn fill(mut buf: &mut [u8]) {
    while !buf.is_empty() {
        let ent = get().to_le_bytes();
        let len = min(ent.len(), buf.len());
        let dst: &mut [u8];
        (dst, buf) = buf.split_at_mut(len);
        dst.copy_from_slice(&ent[..len]);
    }
}

/// Returns an iterator that yields pseudo-random bytes.
///
/// The iterator never ends.
///
/// # Examples
///
/// ```
/// let mut bytes = poorentropy::bytes();
/// let a = bytes.next().unwrap();
/// let b = bytes.next().unwrap();
/// assert_ne!(a, b);
/// ```
#[inline]
#[must_use]
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "loongarch64",
    target_arch = "riscv64",
    target_arch = "x86",
    target_arch = "x86_64"
))]
pub fn bytes() -> iter::Bytes {
    iter::Bytes::default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let a = crate::get();
        let b = crate::get();
        assert_ne!(a, b);
    }

    fn monte_carlo<I>(iter: I)
    where
        I: IntoIterator<Item = f64>,
    {
        use core::f64::consts::PI;

        let mut inside = 0usize;
        let mut outside = 0usize;
        let mut iter = iter.into_iter();

        for _ in 0..500_000 {
            let x = iter.next().unwrap();
            let y = iter.next().unwrap();
            assert!((0. ..=1.).contains(&x), "x out of range: {x}");
            assert!((0. ..=1.).contains(&y), "y out of range: {y}");
            let d = (x * x + y * y).sqrt();
            if d <= 1. {
                inside += 1
            } else {
                outside += 1
            }
        }

        let pi_approx = 4. * (inside as f64) / ((inside + outside) as f64);
        let abs_diff = (PI - pi_approx).abs();

        assert!(
            abs_diff < 0.01,
            "calculated pi: {pi_approx} (expected: {PI}), absolute difference: {abs_diff}"
        );
    }

    mod get {
        #[test]
        fn monte_carlo() {
            let iter = core::iter::from_fn(|| Some((crate::get() as f64) / (u64::MAX as f64)));
            super::monte_carlo(iter);
        }

        #[test]
        fn no_repeat() {
            let mut seen = [0u64; 8000];
            for s in seen.iter_mut() {
                *s = crate::get();
            }

            seen.sort();
            for (prev, next) in seen.iter().zip(seen.iter().skip(1)) {
                assert_ne!(prev, next, "same entropy value returned more than once");
            }
        }
    }

    mod fill {
        #[test]
        fn monte_carlo() {
            let iter = core::iter::from_fn(|| {
                let mut buf = [0u8; 16];
                crate::fill(&mut buf);
                let n = u128::from_le_bytes(buf);
                Some((n as f64) / (u128::MAX as f64))
            });
            super::monte_carlo(iter);
        }
    }

    mod bytes {
        #[test]
        fn monte_carlo() {
            let mut bytes = crate::bytes();
            let iter = core::iter::from_fn(|| {
                let a = bytes.next().unwrap() as u16;
                let b = bytes.next().unwrap() as u16;
                let n = (a << 8) | b;
                Some((n as f64) / (u16::MAX as f64))
            });
            super::monte_carlo(iter);
        }

        #[test]
        fn frequency() {
            let count = 500_000usize;
            let mut seen = [0usize; 256];
            for byte in crate::bytes().take(count) {
                seen[byte as usize] += 1;
            }

            let expected_freq = 1. / 256f64;
            let frequencies = seen.map(|occurrences| (occurrences as f64) / (count as f64));
            for (byte, freq) in frequencies.into_iter().enumerate() {
                let abs_diff = (expected_freq - freq).abs();
                assert!(
                    abs_diff < 0.001,
                    "frequency for byte {byte:03}: {freq} (expected: {expected_freq})"
                );
            }
        }
    }
}
