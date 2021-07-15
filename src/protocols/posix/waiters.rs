// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use std::task::Waker;

pub struct SomeWaker {
    waker: Option<Waker>,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [SomeWaker].
impl SomeWaker {
    /// Takes the waker of the target listener.
    pub fn take(&mut self) -> Option<Waker> {
        self.waker.take()
    }

    /// Places a waker in the target listener.
    pub fn put(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Default trait implementation for [SomeWaker].
impl Default for SomeWaker {
    /// Creates a waiter with default values.
    fn default() -> Self {
        Self { waker: None }
    }
}
