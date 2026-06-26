// Copyright © 2025-2026 Andrea Corbellini and contributors
// SPDX-License-Identifier: BSD-2-Clause

use rand_core::TryRng;

/// `poorentropy` implementation for use with the [`rand` crate]
///
/// This struct is available only when the optional `rand_core` feature is enabled.
///
/// [`rand` crate]: https://docs.rs/rand
///
/// # Examples
///
/// ```
/// use rand::RngExt;
///
/// let mut rng = poorentropy::Rng;
/// let a: u32 = rng.random();
/// let b: u32 = rng.random();
/// assert_ne!(a, b);
/// ```
#[derive(Default, Clone, Debug)]
pub struct Rng;

impl TryRng for Rng {
    type Error = core::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(crate::get() as u32)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(crate::get())
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        Ok(crate::fill(dst))
    }
}

#[cfg(test)]
mod tests {
    use crate::Rng;
    use rand_core::Rng as RngTrait;

    #[test]
    fn next_u32() {
        let mut rng = Rng;
        let a = rng.next_u32();
        let b = rng.next_u32();
        assert_ne!(a, b);
    }

    #[test]
    fn next_u64() {
        let mut rng = Rng;
        let a = rng.next_u64();
        let b = rng.next_u64();
        assert_ne!(a, b);
    }

    #[test]
    fn fill_bytes() {
        let mut rng = Rng;
        let mut a = [0u8; 64];
        let mut b = [0u8; 64];
        rng.fill_bytes(&mut a);
        rng.fill_bytes(&mut b);
        assert_ne!(a, b);
    }
}
