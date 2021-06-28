// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

pub mod datagram;
pub mod peer;
mod options;
mod operations;
mod listener;
mod socket;

#[cfg(test)]
mod tests;

pub use peer::UdpPeer as Peer;
pub use options::UdpOptions as Options;
pub use datagram::UdpHeader;
pub use operations::UdpOperation as UdpOperation;
pub use operations::PopFuture as UdpPopFuture;