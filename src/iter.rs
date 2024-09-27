// Copyright © 2024 Andrea Corbellini and contributors
// SPDX-License-Identifier: BSD-2-Clause

//! Contains the iterator returned by [`bytes()`](crate::bytes).

use core::iter::FusedIterator;

/// The iterator returned by [`bytes()`](crate::bytes).
///
/// See the [documentation for `bytes()`](crate::bytes) for details and
/// examples.
#[derive(Default, Debug)]
pub struct Bytes {
    buf: [u8; 8],
    pos: usize,
}

impl Clone for Bytes {
    #[inline]
    fn clone(&self) -> Self {
        // Cloning should result in a new random sequence
        Self::default()
    }
}

impl Iterator for Bytes {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == 0 {
            self.buf = crate::get().to_le_bytes();
        }
        let pos = self.pos;
        self.pos = (pos + 1) & 7;
        Some(self.buf[pos])
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }

    #[inline]
    fn nth(&mut self, _n: usize) -> Option<Self::Item> {
        self.next()
    }
}

impl FusedIterator for Bytes {}
