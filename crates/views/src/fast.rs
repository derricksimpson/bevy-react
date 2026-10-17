//! A fast hasher for the runtime's key maps.
//!
//! List keys are the app's own ids (numbers and short strings), never
//! attacker-chosen, and the maps are rebuilt on every list change, so the
//! default SipHash's DoS resistance buys nothing here and costs a lot at
//! 10k keys. This is the Fx hash (as `rustc` uses): one multiply per word.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for w in words {
            self.add(u64::from_le_bytes(*w));
        }
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buf) ^ ((rest.len() as u64) << 56));
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }

    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    fn write_i64(&mut self, i: i64) {
        self.add(i as u64);
    }

    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    fn write_isize(&mut self, i: isize) {
        self.add(i as u64);
    }

    fn finish(&self) -> u64 {
        // hashbrown takes its control bits from the top and its bucket from
        // the bottom: fold the high half down so small keys use both well.
        self.hash ^ (self.hash >> 32)
    }
}

pub(crate) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub(crate) type FastSet<K> = HashSet<K, BuildHasherDefault<FxHasher>>;

/// A map with room for `n` entries.
pub(crate) fn map_with_capacity<K, V>(n: usize) -> FastMap<K, V> {
    HashMap::with_capacity_and_hasher(n, BuildHasherDefault::default())
}

/// A set with room for `n` entries.
pub(crate) fn set_with_capacity<K>(n: usize) -> FastSet<K> {
    HashSet::with_capacity_and_hasher(n, BuildHasherDefault::default())
}

#[cfg(test)]
mod tests {
    use std::hash::{BuildHasher, Hash};

    use super::*;

    fn h<T: Hash>(t: &T) -> u64 {
        BuildHasherDefault::<FxHasher>::default().hash_one(t)
    }

    #[test]
    fn hashes_are_deterministic_and_spread_sequential_keys() {
        assert_eq!(h(&5i64), h(&5i64));
        assert_ne!(h(&5i64), h(&6i64));
        assert_ne!(h(&"ab"), h(&"ba"));
        assert_ne!(h(&"a"), h(&"a\0"), "length is part of the hash");
        // Sequential ids must not collide in the low bits (bucket index).
        let low: FastSet<u64> = (0..1024i64).map(|i| h(&i) & 0x3FF).collect();
        assert!(
            low.len() > 600,
            "only {} distinct buckets of 1024",
            low.len()
        );
    }
}
