// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail,
    file_table::FileDescriptor,
    protocols::posix::waiters::SomeWaker,
    runtime::{Runtime, RuntimeBuf},
};

use std::{
    cell::RefCell,
    fmt,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use nix::{self, errno::Errno::*, errno::EWOULDBLOCK, sys::socket, unistd, Error};

//==============================================================================
// Constants & Structures
//==============================================================================

/// # Posix Futures
///
/// For each asynchronous operation in the Posix stack there is a corresponding
/// future. Each of these structures store enough information so that we can
/// poll for a given operation. Futhermore, futures have a reference to their
/// waker so that we can re-schedule operations that have not completed.  Also,
/// note that we have included the markers just because the current future
/// system requires futures to be generic over the runtime. In later versions we
/// shall drop this.

/// Maximum size fo `pop()`.
const POP_SIZE: usize = 1024;

/// Future Result for `accept()`
pub struct AcceptFuture<RT: Runtime> {
    fd: FileDescriptor,
    waiter: Rc<RefCell<SomeWaker>>,
    // TODO: drop marker once we fix the our futures.
    _marker: PhantomData<RT>,
}

/// Future Result for `connect()`
pub struct ConnectFuture<RT: Runtime> {
    fd: FileDescriptor,
    saddr: socket::SockAddr,
    waiter: Rc<RefCell<SomeWaker>>,
    // TODO: drop marker once we fix the our futures.
    _marker: PhantomData<RT>,
}

/// Future Result for `push()`
pub struct PushFuture<RT: Runtime> {
    fd: FileDescriptor,
    buf: RT::Buf,
    waiter: Rc<RefCell<SomeWaker>>,
    // TODO: drop marker once we fix the our futures.
    _marker: PhantomData<RT>,
}

/// Future Result for `pop()`
pub struct PopFuture<RT: Runtime> {
    fd: FileDescriptor,
    waiter: Rc<RefCell<SomeWaker>>,
    // TODO: drop marker once we fix the our futures.
    _marker: PhantomData<RT>,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [AcceptFuture].
impl<RT: Runtime> AcceptFuture<RT> {
    /// Creates an [AcceptFuture].
    pub fn new(fd: FileDescriptor, waiter: Rc<RefCell<SomeWaker>>) -> Self {
        AcceptFuture {
            fd,
            waiter,
            _marker: PhantomData::default(),
        }
    }

    /// Returns the file descriptor associated with the target [AcceptFuture].
    pub fn fd(&self) -> FileDescriptor {
        self.fd
    }
}

/// Associate functions for [ConnectFuture].
impl<RT: Runtime> ConnectFuture<RT> {
    /// Creates an [ConnectFuture].
    pub fn new(
        fd: FileDescriptor,
        saddr: socket::SockAddr,
        waiter: Rc<RefCell<SomeWaker>>,
    ) -> Self {
        ConnectFuture {
            fd,
            saddr,
            waiter,
            _marker: PhantomData::default(),
        }
    }

    /// Returns the file descriptor associated with the target [ConnectFuture].
    pub fn fd(&self) -> FileDescriptor {
        self.fd
    }
}

/// Associate functions for [PushFuture].
impl<RT: Runtime> PushFuture<RT> {
    /// Creates an [PushFuture].
    pub fn new(fd: FileDescriptor, buf: RT::Buf, waiter: Rc<RefCell<SomeWaker>>) -> Self {
        PushFuture {
            fd,
            buf,
            waiter,
            _marker: PhantomData::default(),
        }
    }

    /// Returns the file descriptor associated with the target [PushFuture].
    pub fn fd(&self) -> FileDescriptor {
        self.fd
    }
}

/// Associate functions for [PopFuture].
impl<RT: Runtime> PopFuture<RT> {
    /// Creates an [PopFuture].
    pub fn new(fd: FileDescriptor, waiter: Rc<RefCell<SomeWaker>>) -> Self {
        PopFuture {
            fd,
            waiter,
            _marker: PhantomData::default(),
        }
    }

    /// Returns the file descriptor associated with the target [PopFuture].
    pub fn fd(&self) -> FileDescriptor {
        self.fd
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Future trait implementation for [AcceptFuture].
impl<RT: Runtime> Future for AcceptFuture<RT> {
    type Output = Result<FileDescriptor, Fail>;

    /// Polls an accept operation.
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        info!("polling {:?}", self);
        let self_ = self.get_mut();
        match socket::accept(self_.fd as i32) {
            // Operation completed.
            Ok(newfd) => {
                info!("connection accepted!");
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                Poll::Ready(Ok(newfd as FileDescriptor))
            }
            // Operation not ready yet.
            Err(Error::Sys(e)) if e == EWOULDBLOCK || e == EAGAIN => {
                info!("waiting for connections...");
                let waker = ctx.waker();
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(Some(waker.clone()));
                Poll::Pending
            }
            // Operation failed.
            Err(e) => {
                warn!("failed to accept connection ({:?})", e);
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                // TODO: fail with right error code.
                Poll::Ready(Err(Fail::ConnectionAborted {}))
            }
        }
    }
}
/// Debug trait implementation for [AcceptFuture].
impl<RT: Runtime> fmt::Debug for AcceptFuture<RT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AcceptFuture({})", self.fd)
    }
}

/// Future trait implementation for [ConnectFuture].
impl<RT: Runtime> Future for ConnectFuture<RT> {
    type Output = Result<(), Fail>;

    /// Polls an connect operation.
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        info!("polling {:?}", self);
        let self_ = self.get_mut();
        match socket::connect(self_.fd as i32, &self_.saddr) {
            // Operation completed.
            Ok(_) => {
                info!("connection established!");
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                Poll::Ready(Ok(()))
            }
            // Operation not ready yet.
            Err(Error::Sys(e)) if e == EINPROGRESS => {
                info!("connection in progress...");
                let waker = ctx.waker();
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(Some(waker.clone()));
                Poll::Pending
            }
            // Operation failed.
            Err(e) => {
                warn!("failed to establish connection ({:?})", e);
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                // TODO: fail with right error code.
                Poll::Ready(Err(Fail::ConnectionRefused {}))
            }
        }
    }
}

/// Debug trait implementation for [ConnectFuture].
impl<RT: Runtime> fmt::Debug for ConnectFuture<RT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ConnectFuture({})", self.fd)
    }
}

/// Future trait implementation for [PushFuture].
impl<RT: Runtime> Future for PushFuture<RT> {
    type Output = Result<(), Fail>;

    /// Polls an connect operation.
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        info!("polling {:?}", self);
        let self_ = self.get_mut();
        match unistd::write(self_.fd as i32, &self_.buf[..]) {
            // Operation completed.
            Ok(_) => {
                info!("data pushed!");
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                Poll::Ready(Ok(()))
            }
            // Operation in progress.
            Err(Error::Sys(e)) if e == EWOULDBLOCK || e == EAGAIN => {
                info!("push in progress...");
                let waker = ctx.waker();
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(Some(waker.clone()));
                Poll::Pending
            }
            // Error.
            Err(e) => {
                warn!("push failed ({:?})", e);
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                // TODO: fail with right error code.
                Poll::Ready(Err(Fail::IoError {}))
            }
        }
    }
}

/// Debug trait implementation for [PushFuture].
impl<RT: Runtime> fmt::Debug for PushFuture<RT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PushFuture({})", self.fd)
    }
}

/// Future trait implementation for [PopFuture].
impl<RT: Runtime> Future for PopFuture<RT> {
    type Output = Result<RT::Buf, Fail>;

    /// Polls an connect operation.
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        info!("polling {:?}", self);
        let self_ = self.get_mut();
        // FIXME: we shouldn't impose this constraint.
        let mut bytes: [u8; POP_SIZE] = [0; POP_SIZE];
        match unistd::read(self_.fd as i32, &mut bytes[..]) {
            // Operation completed.
            Ok(nbytes) => {
                info!("data popped!");
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                let buf = RT::Buf::from_slice(&bytes[0..nbytes]);
                Poll::Ready(Ok(buf))
            }
            // Operation in progress.
            Err(Error::Sys(e)) if e == EWOULDBLOCK || e == EAGAIN => {
                info!("pop in progress...");
                let waker = ctx.waker();
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(Some(waker.clone()));
                Poll::Pending
            }
            // Error.
            Err(e) => {
                warn!("pop failed ({:?})", e);
                let mut waiter = self_.waiter.borrow_mut();
                waiter.put(None);
                // TODO: fail with right error code.
                Poll::Ready(Err(Fail::IoError {}))
            }
        }
    }
}

/// Debug trait implementation for [PopFuture].
impl<RT: Runtime> fmt::Debug for PopFuture<RT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PopFuture({})", self.fd)
    }
}
