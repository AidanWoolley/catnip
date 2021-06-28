// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg(test)]
mod tests;

use crate::{collections::HashTtlCache, protocols::ethernet2::MacAddress};

use std::{
    collections::HashMap,
    net::Ipv4Addr,
    time::{Duration, Instant},
};

const DUMMY_MAC_ADDRESS: MacAddress = MacAddress::new([0; 6]);

#[derive(Debug)]
struct Record {
    link_addr: MacAddress,
    ipv4_addr: Ipv4Addr,
}

///
/// # ARP Cache
/// - TODO: Allow multiple waiters for the same address
/// - TODO: Deregister waiters here when the receiver goes away.
/// - TODO: Implement eviction.
/// - TODO: Implement remove.
pub struct ArpCache {
    /// Cache for IPv4 Addresses
    cache: HashTtlCache<Ipv4Addr, Record>,

    /// Disable ARP?
    disable: bool,
}

impl ArpCache {
    /// Creates an ARP Cache.
    pub fn new(
        now: Instant,
        default_ttl: Option<Duration>,
        values: Option<&HashMap<Ipv4Addr, MacAddress>>,
        disable: bool,
    ) -> ArpCache {
        let mut peer = ArpCache {
            cache: HashTtlCache::new(now, default_ttl),
            disable,
        };

        // Populate cache.
        if let Some(values) = values {
            for (&k, &v) in values {
                peer.insert(k, v);
            }
        }

        peer
    }

    // Exports address resolutions that are stored in the ARP cache.
    pub fn export(&self) -> HashMap<Ipv4Addr, MacAddress> {
        let mut map: HashMap<Ipv4Addr, MacAddress> = HashMap::default();
        for (k, v) in self.cache.iter() {
            map.insert(*k, v.link_addr);
        }
        map
    }

    /// Caches an address resolution.
    pub fn insert(&mut self, ipv4_addr: Ipv4Addr, link_addr: MacAddress) -> Option<MacAddress> {
        let record = Record {
            link_addr,
            ipv4_addr,
        };
        self.cache.insert(ipv4_addr, record).map(|r| r.link_addr)
    }

    /// Gets the MAC address of given IPv4 address.
    pub fn get(&self, ipv4_addr: Ipv4Addr) -> Option<&MacAddress> {
        if self.disable {
            Some(&DUMMY_MAC_ADDRESS)
        } else {
            self.cache.get(&ipv4_addr).map(|r| &r.link_addr)
        }
    }

    /// Advances internal clock of the ARP Cache.
    pub fn advance_clock(&mut self, now: Instant) {
        self.cache.advance_clock(now)
    }

    /// Clears the ARP cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}
