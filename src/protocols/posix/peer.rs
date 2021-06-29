// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{futures, waiters::SomeWaker};

use crate::{
    fail::Fail,
    file_table::FileDescriptor,
    protocols::{ipv4, Protocol},
    runtime::Runtime,
    scheduler::SchedulerHandle,
};

use nix::{self, sys::socket, unistd};

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

//==============================================================================
// Constants & Structures
//==============================================================================

/// Sleep length for background task.
const SLEEP_LENGTH: u64 = 1;

/// Peer for Posix Stack
struct PosixPeerInner<RT: Runtime> {
    rt: RT,
    listeners: HashMap<FileDescriptor, Rc<RefCell<SomeWaker>>>,
    waiters: HashMap<FileDescriptor, Rc<RefCell<SomeWaker>>>,
    senders: HashMap<FileDescriptor, Rc<RefCell<SomeWaker>>>,
    receivers: HashMap<FileDescriptor, Rc<RefCell<SomeWaker>>>,
    #[allow(unused)]
    // NOTE: we need this in order to get our background task scheduled.
    _handle: Option<SchedulerHandle>,
}

/// Wrapper for Posix Peer
pub struct PosixPeer<RT: Runtime> {
    inner: Rc<RefCell<PosixPeerInner<RT>>>,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [PosixPeerInner].
impl<RT: Runtime> PosixPeerInner<RT> {
    /// Creates a Posix peer inner.
    fn new(rt: RT) -> Self {
        Self {
            rt,
            listeners: HashMap::default(),
            waiters: HashMap::default(),
            senders: HashMap::default(),
            receivers: HashMap::default(),
            _handle: None,
        }
    }
}

/// Associate functions for [PosixPeer].
impl<RT: Runtime> PosixPeer<RT> {
    /// Creates a Posix peer.
    pub fn new(rt: RT) -> Self {
        let inner = Rc::new(RefCell::new(PosixPeerInner::new(rt.clone())));
        let future = Self::background(inner.clone());
        let handle = rt.spawn(future);
        inner.borrow_mut()._handle = Some(handle);
        Self {
            inner: inner.clone(),
        }
    }

    /// Periodically pools asynchronous operations.
    async fn background(inner: Rc<RefCell<PosixPeerInner<RT>>>) {
        let rt = inner.borrow().rt.clone();
        loop {
            for (_, v) in inner.borrow().listeners.iter() {
                if let Some(w) = v.borrow_mut().take() {
                    w.wake();
                }
            }
            for (_, v) in inner.borrow().waiters.iter() {
                if let Some(w) = v.borrow_mut().take() {
                    w.wake();
                }
            }
            for (_, v) in inner.borrow().senders.iter() {
                if let Some(w) = v.borrow_mut().take() {
                    w.wake();
                }
            }
            for (_, v) in inner.borrow().receivers.iter() {
                if let Some(w) = v.borrow_mut().take() {
                    w.wake();
                }
            }

            // TODO: instead of waiting we could rely on poll().
            rt.wait(Duration::from_secs(SLEEP_LENGTH)).await;
        }
    }

    /// Creates a socket.
    pub fn socket(&self, _protocol: Protocol) -> FileDescriptor {
        let fd = socket::socket(
            socket::AddressFamily::Inet,
            socket::SockType::Stream,
            socket::SockFlag::SOCK_NONBLOCK,
            socket::SockProtocol::Tcp,
        )
        .expect("failed to open socket");

        fd as FileDescriptor
    }

    /// Binds a socket to an address.
    pub fn bind(&self, fd: FileDescriptor, endpoint: ipv4::Endpoint) -> Result<(), Fail> {
        let ip4: std::net::IpAddr = std::net::IpAddr::V4(endpoint.addr);
        let ip4: socket::IpAddr = socket::IpAddr::from_std(&ip4);
        let port16: u16 = endpoint.port.into();
        let inet = socket::InetAddr::new(ip4, port16);
        let addr = socket::SockAddr::new_inet(inet);

        match socket::bind(fd as i32, &addr) {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("failed to bind socket ({:?})", e);
                // TODO: fail with right error code.
                Err(Fail::BadFileDescriptor {
                    details: "cannot bind socket",
                })
            }
        }
    }

    /// Listens for connections.
    pub fn listen(&self, fd: FileDescriptor, backlog: usize) -> Result<(), Fail> {
        socket::listen(fd as i32, backlog).expect("failed to listen socket");

        Ok(())
    }

    /// Connects to a remote peer.
    pub fn connect(
        &self,
        fd: FileDescriptor,
        endpoint: ipv4::Endpoint,
    ) -> futures::ConnectFuture<RT> {
        let ip4: std::net::IpAddr = std::net::IpAddr::V4(endpoint.addr);
        let ip4: socket::IpAddr = socket::IpAddr::from_std(&ip4);
        let port16: u16 = endpoint.port.into();
        let inet = socket::InetAddr::new(ip4, port16);
        let addr = socket::SockAddr::new_inet(inet);

        let waiter = SomeWaker::default();
        let waiter = Rc::new(RefCell::new(waiter));
        self.inner.borrow_mut().waiters.insert(fd, waiter.clone());

        futures::ConnectFuture::new(fd, addr, waiter.clone())
    }

    /// Accepts incoming connections.
    pub fn accept(&self, fd: FileDescriptor) -> futures::AcceptFuture<RT> {
        let waiter = SomeWaker::default();
        let waiter = Rc::new(RefCell::new(waiter));
        self.inner.borrow_mut().listeners.insert(fd, waiter.clone());

        futures::AcceptFuture::new(fd, waiter.clone())
    }

    /// Closes a connection.
    pub fn close(&self, fd: FileDescriptor) -> Result<(), Fail> {
        self.inner.borrow_mut().waiters.remove(&fd);
        self.inner.borrow_mut().senders.remove(&fd);
        self.inner.borrow_mut().receivers.remove(&fd);
        unistd::close(fd as i32).expect("failed to close socket");
        Ok(())
    }

    /// Pushes data to a remote peer.
    pub fn push(&self, fd: FileDescriptor, buf: RT::Buf) -> futures::PushFuture<RT> {
        let sender = SomeWaker::default();
        let sender = Rc::new(RefCell::new(sender));
        self.inner.borrow_mut().senders.insert(fd, sender.clone());

        futures::PushFuture::new(fd, buf, sender.clone())
    }

    /// Pops data from a remote peer.
    pub fn pop(&self, fd: FileDescriptor) -> futures::PopFuture<RT> {
        let receiver = SomeWaker::default();
        let receiver = Rc::new(RefCell::new(receiver));
        self.inner
            .borrow_mut()
            .receivers
            .insert(fd, receiver.clone());

        futures::PopFuture::new(fd, receiver.clone())
    }
}
