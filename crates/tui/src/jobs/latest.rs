use std::{collections::HashMap, hash::Hash};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LatestGen {
    cur: u64,
}

impl LatestGen {
    pub(crate) fn next(&mut self) -> u64 {
        self.cur = self.cur.wrapping_add(1);
        self.cur
    }

    pub(crate) fn current(&self) -> u64 {
        self.cur
    }

    pub(crate) fn is_latest(&self, generation: u64) -> bool {
        self.cur == generation
    }
}

#[derive(Debug, Default)]
pub(crate) struct LatestByKey<K: Eq + Hash> {
    gens: HashMap<K, u64>,
}

impl<K: Eq + Hash> LatestByKey<K> {
    pub(crate) fn next_for(&mut self, key: K) -> u64 {
        let next = self.gens.get(&key).copied().unwrap_or(0).wrapping_add(1);
        self.gens.insert(key, next);
        next
    }

    pub(crate) fn is_latest_for(&self, key: &K, generation: u64) -> bool {
        self.gens.get(key).copied().unwrap_or(0) == generation
    }

    #[allow(dead_code)]
    pub(crate) fn current_for(&self, key: &K) -> u64 {
        self.gens.get(key).copied().unwrap_or(0)
    }
}
