// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::*;
use crate::test_helpers;

/// Tests that an entry of the ARP Cache gets evicted at the right time.
#[test]
fn evit_with_default_ttl() {
    let now = Instant::now();
    let ttl = Duration::from_secs(1);
    let later = now + ttl;

    // Insert an IPv4 address in the ARP Cache.
    let mut cache = ArpCache::new(now, Some(ttl), false);
    cache.insert(test_helpers::ALICE_IPV4, test_helpers::ALICE_MAC);
    assert!(cache.get_link_addr(test_helpers::ALICE_IPV4) == Some(&test_helpers::ALICE_MAC));

    // Advance the internal clock of the cache and clear it.
    cache.advance_clock(later);
    cache.clear();

    // The IPv4 address must be gone.
    assert!(cache.get_link_addr(test_helpers::ALICE_IPV4).is_none());
}

/// Tests import on the ARP Cache.
#[test]
fn import() {
    let now = Instant::now();
    let ttl = Duration::from_secs(1);

    // Create an address resolution map.
    let mut map: HashMap<Ipv4Addr, MacAddress> = HashMap::new();
    map.insert(test_helpers::ALICE_IPV4, test_helpers::ALICE_MAC);

    // Create an ARP Cache and import address resolution map.
    let mut cache = ArpCache::new(now, Some(ttl), false);
    cache.import(map);

    // Check if address resolutions are in the ARP Cache.
    assert!(cache.get_link_addr(test_helpers::ALICE_IPV4) == Some(&test_helpers::ALICE_MAC));
}

/// Tests export on the ARP Cache.
#[test]
fn export() {
    let now = Instant::now();
    let ttl = Duration::from_secs(1);

    // Insert an IPv4 address in the ARP Cache.
    let mut cache = ArpCache::new(now, Some(ttl), false);
    cache.insert(test_helpers::ALICE_IPV4, test_helpers::ALICE_MAC);
    assert!(cache.get_link_addr(test_helpers::ALICE_IPV4) == Some(&test_helpers::ALICE_MAC));

    // Export address resolution map.
    let map: HashMap<Ipv4Addr, MacAddress> = cache.export();

    // Check if address resolutions are in the map that was exported.
    assert!(
        map.get_key_value(&test_helpers::ALICE_IPV4)
            == Some((&test_helpers::ALICE_IPV4, &test_helpers::ALICE_MAC))
    );
}
