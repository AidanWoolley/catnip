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

use common::libos::*;

use common::*;

//==============================================================================
// Open/Close Passive Socket
//==============================================================================

/// Tests if a passive socket may be successfully opened and closed.
fn do_tcp_connection_setup(port: u16) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp());


    let port = ip::Port::try_from(port).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);

    // Open and close a connection.
    let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    libos.bind(sockfd, local).unwrap();
    libos.listen(sockfd, 8).unwrap();
    libos.close(sockfd).unwrap();
}

#[test]
fn catnip_tcp_connection_setup() {
    do_tcp_connection_setup(PORT_BASE);
}

//==============================================================================
// Establish Connection
//==============================================================================

/// Tests if data can be successfully established.
fn do_tcp_establish_connection(port: u16) {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd).unwrap();
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);
        let qd = unsafe { r.qr_value.ares.qd } as u32;

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = DummyLibOS::new(BOB_MAC, BOB_IPV4, bob_tx, alice_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

#[test]
fn catnip_tcp_establish_connection() {
    do_tcp_establish_connection(PORT_BASE + 1)
}

//==============================================================================
// Push
//==============================================================================

/// Tests if data can be successfully established.
fn do_tcp_push_remote(port: u16) {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd).unwrap();
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);

        // Pop data.
        let qd = unsafe { r.qr_value.ares.qd } as u32;
        let qt = libos.pop(qd).unwrap();
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        DummyLibOS::check_data(sga);
        libos.rt().free_sgarray(sga);

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = DummyLibOS::new(BOB_MAC, BOB_IPV4, bob_tx, alice_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Cook some data.
        let body_sga = DummyLibOS::cook_data(&mut libos);

        // Push data.
        let qt = libos.push(sockfd, &body_sga).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);
        libos.rt().free_sgarray(body_sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

#[test]
fn catnip_tcp_push_remote() {
    do_tcp_push_remote(PORT_BASE + 2)
}
