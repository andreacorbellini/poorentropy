Low-quality entropy generator for `no_std` crates

This crate provides a reliable entropy source to crates that cannot depend on the [Rust
standard library](https://doc.rust-lang.org/std/). The design goals for this crate are
simplicity and ease of use.

The entropy generated is not suitable for security or cryptography (see
[Limitations](#limitations) and [How It Works](#how-it-works) below for details), although it
may be combined with other entropy sources to produce a higher security level. This crate may
be used for testing or for all sort of randomizations where security is not a constraint.

# Highlights

- Zero external dependencies
- No dependency on the [Rust standard library](https://doc.rust-lang.org/std/): this is a
  `no_std` crate
- Works on all modern architectures: x86, AArch64, RISC-V 64, LoongArch64

# Usage and Examples

[`get()`](https://docs.rs/poorentropy/latest/poorentropy/fn.get.html) returns
some entropy as a
[`u64`](https://doc.rust-lang.org/stable/core/primitive.u64.html):

```rust
let e = poorentropy::get();
```

[`fill()`](https://docs.rs/poorentropy/latest/poorentropy/fn.fill.html) and
[`bytes()`](https://docs.rs/poorentropy/latest/poorentropy/fn.bytes.html) can
be used to obtain the entropy as bytes.

Generally speaking, entropy sources should not be used directly, but should rather be used as a
seed for a pseudo-random number generator. Here is an example using the [`rand`
crate](https://crates.io/crates/rand):

```rust
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::SmallRng;

let mut seed = <SmallRng as SeedableRng>::Seed::default();
poorentropy::fill(&mut seed);
let mut rng = SmallRng::from_seed(seed);

// Use the `rng`...
let r = rng.next_u32();
```

# How It Works

The crate works by reading the CPU "clock" or "cycle counter", and mixing it to produce a
pseudo-random value.

This table describes how the CPU clock/counter value is obtained for each supported
architecture:

| Target        | Source       |
|---------------|--------------|
| AArch64       | `cntvct_el0` |
| LoongArch64   | `rdtime.d`   |
| RISC-V 64     | `rdcycle`    |
| x86 / x86\_64 | `rdtsc`      |

The value obtained from the CPU is also added to an internal counter, with the
goal to avoid returning the same entropy values to concurrent threads that call
[`get()`](https://docs.rs/poorentropy/latest/poorentropy/fn.get.html) at the
same time.

The resulting value is then fed into the
[SplitMix64](https://en.wikipedia.org/wiki/Xorshift#Initialization) generator to make it appear
random.

# Limitations

* Because the crate relies on the CPU clock, the values that it produces may be easy to
  predict. As such, this crate alone may not be used for security applications, because
  attackers may be able to guess the entropy values produced. It can however be combined with
  other entropy sources to increase the security level.

* This crate tries to make it hard for two threads in the same process to obtain the same
  entropy value. However no guarantees are made for two distinct processes.

* On some CPUs, the clock may start from a fixed value at boot. As such, if this crates is used
  in applications that run early on boot (such as firmwares, bootloaders, or kernels), it is
  possible that the crate will yield the same entropy values at every boot.

* On some CPUs, the clock may be disabled or reset at runtime, and this may result in very
  low-quality entropy values being returned.
