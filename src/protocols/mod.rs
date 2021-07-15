// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

pub mod arp;
pub mod ethernet2;
pub mod icmpv4;
pub mod ip;
pub mod ipv4;
pub mod posix;
pub mod tcp;
pub mod udp;

pub enum Protocol {
    Tcp,
    Udp,
}
