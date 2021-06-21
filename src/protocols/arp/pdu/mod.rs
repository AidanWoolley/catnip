// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{fail::Fail, protocols::ethernet2::MacAddress, runtime::RuntimeBuf};
use byteorder::{ByteOrder, NetworkEndian};
use num_traits::FromPrimitive;
use std::{convert::TryInto, net::Ipv4Addr};

const ARP_HTYPE_ETHER2: u16 = 1;
const ARP_HLEN_ETHER2: u8 = 6;
const ARP_PTYPE_IPV4: u16 = 0x800;
const ARP_PLEN_IPV4: u8 = 4;
const ARP_MESSAGE_SIZE: usize = 28;

#[repr(u16)]
#[derive(FromPrimitive, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArpOperation {
    Request = 1,
    Reply = 2,
}

///
/// # Protocol Data Unit (PDU) for ARP
///
#[derive(Clone, Debug)]
pub struct ArpPdu {
    // We only support Ethernet/Ipv4, so omit these fields.
    // hardware_type: u16,
    // protocol_type: u16,
    // hardware_address_len: u8,
    // protocol_address_len: u8,
    pub operation: ArpOperation,
    pub sender_hardware_addr: MacAddress,
    pub sender_protocol_addr: Ipv4Addr,
    pub target_hardware_addr: MacAddress,
    pub target_protocol_addr: Ipv4Addr,
}

impl ArpPdu {
    /// Computes the size of the target ARP PDU.
    pub fn compute_size(&self) -> usize {
        ARP_MESSAGE_SIZE
    }

    pub fn parse<T: RuntimeBuf>(buf: T) -> Result<Self, Fail> {
        if buf.len() < ARP_MESSAGE_SIZE {
            return Err(Fail::Malformed {
                details: "ARP message too short",
            });
        }
        let buf: &[u8; ARP_MESSAGE_SIZE] = &buf[..ARP_MESSAGE_SIZE].try_into().unwrap();
        let hardware_type = NetworkEndian::read_u16(&buf[0..2]);
        if hardware_type != ARP_HTYPE_ETHER2 {
            return Err(Fail::Unsupported {
                details: "Unsupported HTYPE",
            });
        }
        let protocol_type = NetworkEndian::read_u16(&buf[2..4]);
        if protocol_type != ARP_PTYPE_IPV4 {
            return Err(Fail::Unsupported {
                details: "Unsupported PTYPE",
            });
        }
        let hardware_address_len = buf[4];
        if hardware_address_len != ARP_HLEN_ETHER2 {
            return Err(Fail::Unsupported {
                details: "Unsupported HLEN",
            });
        }
        let protocol_address_len = buf[5];
        if protocol_address_len != ARP_PLEN_IPV4 {
            return Err(Fail::Unsupported {
                details: "Unsupported PLEN",
            });
        }
        let operation = FromPrimitive::from_u16(NetworkEndian::read_u16(&buf[6..8])).ok_or({
            Fail::Unsupported {
                details: "Unsupported OPER",
            }
        })?;
        let sender_hardware_addr = MacAddress::from_bytes(&buf[8..14]);
        let sender_protocol_addr = Ipv4Addr::from(NetworkEndian::read_u32(&buf[14..18]));
        let target_hardware_addr = MacAddress::from_bytes(&buf[18..24]);
        let target_protocol_addr = Ipv4Addr::from(NetworkEndian::read_u32(&buf[24..28]));
        let pdu = Self {
            operation,
            sender_hardware_addr,
            sender_protocol_addr,
            target_hardware_addr,
            target_protocol_addr,
        };
        Ok(pdu)
    }

    /// Serializes the target ARP PDU.
    pub fn serialize(&self, buf: &mut [u8]) {
        let buf: &mut [u8; ARP_MESSAGE_SIZE] = (&mut buf[..ARP_MESSAGE_SIZE]).try_into().unwrap();
        NetworkEndian::write_u16(&mut buf[0..2], ARP_HTYPE_ETHER2);
        NetworkEndian::write_u16(&mut buf[2..4], ARP_PTYPE_IPV4);
        buf[4] = ARP_HLEN_ETHER2;
        buf[5] = ARP_PLEN_IPV4;
        NetworkEndian::write_u16(&mut buf[6..8], self.operation as u16);
        buf[8..14].copy_from_slice(&self.sender_hardware_addr.octets());
        buf[14..18].copy_from_slice(&self.sender_protocol_addr.octets());
        buf[18..24].copy_from_slice(&self.target_hardware_addr.octets());
        buf[24..28].copy_from_slice(&self.target_protocol_addr.octets());
    }
}
