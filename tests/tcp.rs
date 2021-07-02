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
// Open/Close Passive Socket
//==============================================================================

/// Tests if a passive socket may be successfully opened and closed.
#[test]
fn tcp_connection_setup() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, tx, rx);

    let port = ip::Port::try_from(80).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);

    // Open and close a connection.
    let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    libos.bind(sockfd, local).unwrap();
    libos.listen(sockfd, 8).unwrap();
    libos.close(sockfd).unwrap();
}

//==============================================================================
// Establish Connection
//==============================================================================

/// Tests if data can be successfully established.
#[test]
fn tcp_establish_connection() {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd);
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);
        let qd = unsafe { r.qr_value.ares.qd } as u32;

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = libos_init(BOB_MAC, BOB_IPV4, bob_tx, alice_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(BOB_IPV4, port);
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        let qt = libos.connect(sockfd, remote);
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

//==============================================================================
// Push
//==============================================================================

/// Tests if data can be successfully established.
#[test]
fn tcp_push_remote() {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = libos_init(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd);
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);

        // // Pop data.
        let qd = unsafe { r.qr_value.ares.qd } as u32;
        let qt = libos.pop(qd);
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        libos_check_data(sga);
        libos.rt().free_sgarray(sga);

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = libos_init(BOB_MAC, BOB_IPV4, bob_tx, alice_rx);

        let port = ip::Port::try_from(80).unwrap();
        let local = ipv4::Endpoint::new(BOB_IPV4, port);
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote);
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Cook some data.
        let body_sga = libos_cook_data(&mut libos);

        // Push data.
        let qt = libos.push(sockfd, &body_sga);
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);
        libos.rt().free_sgarray(body_sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}