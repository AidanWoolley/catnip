// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

// todo: implement `HashMap` forwarding as needed.

#[cfg(test)]
mod tests;

use std::collections::{
    hash_map::Entry as HashMapEntry,
    HashMap,
};
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::Debug,
    hash::Hash,
    time::{
        Duration,
        Instant,
    },
};

#[derive(Debug, PartialEq, Eq, Clone)]
struct Expiry(Instant);

impl Expiry {
    pub fn has_expired(&self, now: Instant) -> bool {
        now >= self.0
    }
}

impl Ord for Expiry {
    fn cmp(&self, other: &Expiry) -> Ordering {
        // `BinaryHeap` is a max-heap, so we need to reverse the order of
        // comparisons in order to get `peek()` and `pop()` to return the
        // smallest time.
        match self.0.cmp(&other.0) {
            Ordering::Equal => Ordering::Equal,
            Ordering::Less => Ordering::Greater,
            Ordering::Greater => Ordering::Less,
        }
    }
}

impl PartialOrd for Expiry {
    fn partial_cmp(&self, other: &Expiry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
struct Record<V> {
    value: V,
    expiry: Option<Expiry>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Tombstone<K>
where
    K: Eq,
{
    key: K,
    expiry: Expiry,
}

impl<K> Ord for Tombstone<K>
where
    K: Eq,
{
    fn cmp(&self, other: &Tombstone<K>) -> Ordering {
        self.expiry.cmp(&other.expiry)
    }
}

impl<K> PartialOrd for Tombstone<K>
where
    K: Eq,
{
    fn partial_cmp(&self, other: &Tombstone<K>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// todo: `HashMap<>` has an `S` parameter that i'd like to include but causes
// problems with the inference engine. the workaround is to leave it out but
// what am i doing wrong?
#[derive(Debug)]
pub struct HashTtlCache<K, V>
where
    K: Eq + Hash,
{
    map: HashMap<K, Record<V>>,
    graveyard: BinaryHeap<Tombstone<K>>,
    default_ttl: Option<Duration>,
    clock: Instant,
}

pub type Iter<'a, K, V> = dyn Iterator<Item = (&'a K, &'a V)>;

impl<K, V> HashTtlCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(now: Instant, default_ttl: Option<Duration>) -> HashTtlCache<K, V> {
        if let Some(ttl) = default_ttl {
            assert!(ttl > Duration::new(0, 0));
        }
        let default_ttl = None;

        HashTtlCache {
            map: HashMap::default(),
            graveyard: BinaryHeap::new(),
            default_ttl,
            clock: now,
        }
    }

    pub fn insert_with_ttl(&mut self, key: K, value: V, ttl: Option<Duration>) -> Option<V> {
        if let Some(ttl) = ttl {
            assert!(ttl > Duration::new(0, 0));
        }

        let expiry = ttl.map(|dt| Expiry(self.clock + dt));

        let old_value = match self.map.entry(key.clone()) {
            HashMapEntry::Occupied(mut e) => {
                let mut record = e.get_mut();
                let old_value = if let Some(ref expiry) = record.expiry {
                    if expiry.has_expired(self.clock) {
                        None
                    } else {
                        Some(record.value.clone())
                    }
                } else {
                    Some(record.value.clone())
                };

                record.value = value;
                record.expiry = expiry.clone();

                old_value
            },
            HashMapEntry::Vacant(e) => {
                e.insert(Record {
                    value,
                    expiry: expiry.clone(),
                });

                None
            },
        };

        if let Some(expiry) = expiry {
            let expiry = Tombstone { key, expiry };

            self.graveyard.push(expiry);
        }

        old_value
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_with_ttl(key, value, self.default_ttl)
    }

    pub fn remove(&mut self, _key: &K) -> Option<V> {
        None
    }

    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: Debug,
    {
        trace!("HashTtlCache::get({:?})", key);
        debug!("self.map.len() -> {:?}", self.map.len());
        return self.map.get(key).map(|r| &r.value);
    }

    pub fn advance_clock(&mut self, now: Instant) {
        assert!(now >= self.clock);
        self.clock = now;
    }

    pub fn try_evict(&mut self, _count: usize) -> HashMap<K, V> {
        HashMap::default()
    }

    // todo: how do i implement `std::iter::IntoIterator` for this type?
    // todo: how do i get `&cache` to alias to `cache.iter()`?
    pub fn iter(&self) -> impl Iterator<Item = (&'_ K, &'_ V)> {
        let clock = self.clock;
        self.map.iter().flat_map(move |(key, record)| {
            if let Some(e) = record.expiry.clone() {
                if e.has_expired(clock) {
                    return None;
                }
            }

            Some((key, &record.value))
        })
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.graveyard.clear();
    }
}
