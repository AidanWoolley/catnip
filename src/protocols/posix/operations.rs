// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::futures::{AcceptFuture, ConnectFuture, PopFuture, PushFuture};

use crate::{
    file_table::FileDescriptor,
    operations::{OperationResult, ResultFuture},
    runtime::Runtime,
};

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

//==============================================================================
// Constants & Structures
//==============================================================================

/// Operations on Posix stack.
pub enum PosixOperation<RT: Runtime> {
    Accept(ResultFuture<AcceptFuture<RT>>),
    Connect(ResultFuture<ConnectFuture<RT>>),
    Push(ResultFuture<PushFuture<RT>>),
    Pop(ResultFuture<PopFuture<RT>>),
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [PosixOperation].
impl<RT: Runtime> PosixOperation<RT> {
    /// Cooks the result of a Posix operation.
    pub fn expect_result(self) -> (FileDescriptor, OperationResult<RT>) {
        use PosixOperation::*;
        match self {
            // Success.
            Accept(ResultFuture {
                future,
                done: Some(Ok(fd)),
            }) => (future.fd(), OperationResult::Accept(fd)),
            Connect(ResultFuture {
                future,
                done: Some(Ok(())),
            }) => (future.fd(), OperationResult::Connect),
            Push(ResultFuture {
                future,
                done: Some(Ok(())),
            }) => (future.fd(), OperationResult::Push),
            Pop(ResultFuture {
                future,
                done: Some(Ok(bytes)),
            }) => (future.fd(), OperationResult::Pop(None, bytes)),

            // Fail.
            Accept(ResultFuture {
                future,
                done: Some(Err(e)),
            }) => (future.fd(), OperationResult::Failed(e)),
            Connect(ResultFuture {
                future,
                done: Some(Err(e)),
            }) => (future.fd(), OperationResult::Failed(e)),
            Push(ResultFuture {
                future,
                done: Some(Err(e)),
            }) => (future.fd(), OperationResult::Failed(e)),
            Pop(ResultFuture {
                future,
                done: Some(Err(e)),
            }) => (future.fd(), OperationResult::Failed(e)),

            _ => panic!("future not ready?"),
        }
    }
}
//==============================================================================
// Trait Implementations
//==============================================================================

/// Future trait implementation for [PosixOperation].
impl<RT: Runtime> Future for PosixOperation<RT> {
    type Output = ();

    /// Polls a Posix operation.
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<()> {
        // Dispatch polling to the right executor.
        match self.get_mut() {
            PosixOperation::Accept(ref mut f) => Future::poll(Pin::new(f), ctx),
            PosixOperation::Connect(ref mut f) => Future::poll(Pin::new(f), ctx),
            PosixOperation::Push(ref mut f) => Future::poll(Pin::new(f), ctx),
            PosixOperation::Pop(ref mut f) => Future::poll(Pin::new(f), ctx),
        }
    }
}
