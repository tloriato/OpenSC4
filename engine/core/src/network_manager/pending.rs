//! The network lot manager's `pending` set (§8.5 *[compat]*, NM13): an SGI
//! `hash_set<u16>` with identity hash, so the processing order (and with it the
//! order of the §17 draws) is the original's bucket order, not key order.
//!
//! - Buckets start at 193 (the first prime not below the constructor's 100).
//! - Insert first resizes with `count + 1` (even for a key already present):
//!   if `B < count + 1`, `B` becomes the first prime not below `count + 1` and
//!   every node moves, old buckets ascending, each from its head, each pushed
//!   to the head of its new bucket. Then a new key is pushed to the head of its
//!   bucket; an existing key stays where it is.
//! - Erase never shrinks `B`. Iteration: buckets ascending, each from its head.

/// The 28-entry SGI prime list (§8.5).
pub const PRIMES: [u32; 28] = [
    53, 97, 193, 389, 769, 1543, 3079, 6151, 12289, 24593, 49157, 98317, 196613, 393241, 786433, 1572869, 3145739,
    6291469, 12582917, 25165843, 50331653, 100663319, 201326611, 402653189, 805306457, 1610612741, 3221225473,
    4294967291,
];

/// The bucket count the manager's constructor asks for (§8.5).
pub const INITIAL_REQUEST: u32 = 100;

/// The first prime of [`PRIMES`] not below `n` (the last one if none).
pub fn next_prime(n: u32) -> u32 {
    PRIMES
        .iter()
        .copied()
        .find(|&p| p >= n)
        .unwrap_or(PRIMES[PRIMES.len() - 1])
}

/// `pending`. Each bucket is stored head first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingSet {
    buckets: Vec<Vec<u16>>,
    count: usize,
}

impl Default for PendingSet {
    fn default() -> Self {
        PendingSet::with_bucket_count(next_prime(INITIAL_REQUEST))
    }
}

impl PendingSet {
    /// An empty set with `b` buckets (a captured `B`, §8.5 replay rule 6).
    pub fn with_bucket_count(b: u32) -> PendingSet {
        PendingSet {
            buckets: vec![Vec::new(); b.max(1) as usize],
            count: 0,
        }
    }

    /// `B`.
    pub fn bucket_count(&self) -> u32 {
        self.buckets.len() as u32
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn bucket(&self, key: u16) -> usize {
        key as usize % self.buckets.len()
    }

    /// SGI `resize(hint)`.
    fn resize(&mut self, hint: usize) {
        let old = self.buckets.len();
        if hint <= old {
            return;
        }
        let n = next_prime(u32::try_from(hint).unwrap_or(u32::MAX)) as usize;
        if n <= old {
            return;
        }
        let mut nb: Vec<Vec<u16>> = vec![Vec::new(); n];
        for b in std::mem::take(&mut self.buckets) {
            for k in b {
                nb[k as usize % n].insert(0, k);
            }
        }
        self.buckets = nb;
    }

    /// `insert_unique` (`0x0060E210`). Returns whether the key was new.
    pub fn insert(&mut self, key: u16) -> bool {
        self.resize(self.count + 1);
        let i = self.bucket(key);
        if self.buckets[i].contains(&key) {
            return false;
        }
        self.buckets[i].insert(0, key);
        self.count += 1;
        true
    }

    /// Erase by key value (a no-op when absent).
    pub fn remove(&mut self, key: &u16) -> bool {
        let i = self.bucket(*key);
        if let Some(p) = self.buckets[i].iter().position(|k| k == key) {
            self.buckets[i].remove(p);
            self.count -= 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, key: &u16) -> bool {
        self.buckets[self.bucket(*key)].contains(key)
    }

    /// `begin()`: the first key in iteration order.
    pub fn first(&self) -> Option<u16> {
        self.buckets.iter().find_map(|b| b.first().copied())
    }

    /// Keys in iteration order.
    pub fn iter(&self) -> impl Iterator<Item = &u16> + '_ {
        self.buckets.iter().flatten()
    }
}
