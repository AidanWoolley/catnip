// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#![feature(new_uninit)]
#![feature(const_fn, const_panic, const_alloc_layout)]
#![feature(const_mut_refs, const_type_name)]
#![feature(maybe_uninit_uninit_array, maybe_uninit_extra, maybe_uninit_ref)]

use catnip::{
    interop::dmtr_opcode_t,
    protocols::{ip, ipv4},
    runtime::Runtime,
};
use crossbeam_channel::{self};
use libc;
use std::{convert::TryFrom, thread};

mod common;

use common::*;

//==============================================================================
// Connect
//==============================================================================

/// Tests if a connection can be successfully established and closed to a remote
/// endpoint.
#[test]
fn udp_connect_remote() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, tx, rx);

    let port = ip::Port::try_from(80).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);
    let remote = ipv4::Endpoint::new(BOB_IPV4, port);

    // Open and close a connection.
    let sockfd = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
    libos.bind(sockfd, local).unwrap();
    let qt = libos.connect(sockfd, remote).unwrap();
    assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);
    libos.close(sockfd).unwrap();
}

/// Tests if a connection can be successfully established in loopback mode.
#[test]
fn udp_connect_loopback() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, tx, rx);

    let port = ip::Port::try_from(80).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);
    let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

    // Open and close a connection.
    let sockfd = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
    libos.bind(sockfd, local).unwrap();
    let qt = libos.connect(sockfd, remote).unwrap();
    assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);
    libos.close(sockfd).unwrap();
}

//==============================================================================
// Push
//==============================================================================

/// Tests if data can be successfully pushed/popped form a local endpoint to
/// itself.
#[test]
fn udp_push_remote() {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);
        let remote = ipv4::Endpoint::new(BOB_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Cook some data.
        let body_sga = libos_cook_data(&mut libos);

        // Push data.
        let qt = libos.push(sockfd, &body_sga).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);

        // Pop data.
        let qt = libos.pop(sockfd).unwrap();
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        libos_check_data(sga);
        libos.rt().free_sgarray(sga);

        libos.rt().free_sgarray(body_sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = libos_init(BOB_MAC, BOB_IPV4, bob_tx, alice_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(BOB_IPV4, port);
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Pop data.
        let qt = libos.pop(sockfd).unwrap();
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        libos_check_data(sga);

        // Push data.
        let qt = libos.push(sockfd, &sga).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);

        libos.rt().free_sgarray(sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

/// Tests if data can be successfully pushed/popped in loopback mode.
#[test]
fn udp_lookback() {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Cook some data.
        let body_sga = libos_cook_data(&mut libos);

        // Push data.
        let qt = libos.push(sockfd, &body_sga).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);

        // Pop data.
        let qt = libos.pop(sockfd).unwrap();
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        libos_check_data(sga);
        libos.rt().free_sgarray(sga);

        libos.rt().free_sgarray(body_sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, bob_tx, alice_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_DGRAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Pop data.
        let qt = libos.pop(sockfd).unwrap();
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        libos_check_data(sga);

        // Push data.
        let qt = libos.push(sockfd, &sga).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);

        libos.rt().free_sgarray(sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}
