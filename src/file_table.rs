// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use slab::Slab;
use std::{cell::RefCell, rc::Rc};

//==============================================================================
// Constants & Structures
//==============================================================================

/// File Descriptor
pub type FileDescriptor = u32;

/// File Table Data
struct Inner {
    table: Slab<File>,
}

/// File Table
#[derive(Clone)]
pub struct FileTable {
    inner: Rc<RefCell<Inner>>,
}

/// File Types
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum File {
    TcpSocket,
    UdpSocket,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [FileTable].
impl FileTable {
    /// Creates a file table.
    pub fn new() -> Self {
        let inner = Inner { table: Slab::new() };
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    /// Allocates a new entry in the target file descriptor table.
    pub fn alloc(&self, file: File) -> FileDescriptor {
        let mut inner = self.inner.borrow_mut();
        let ix = inner.table.insert(file);
        ix as u32 + 1
    }

    /// Gets the file associated with a file descriptor.
    pub fn get(&self, fd: FileDescriptor) -> Option<File> {
        let inner = self.inner.borrow();
        inner.table.get(ix).cloned()
    }

    /// Releases an entry in the target file descriptor table.
        let mut inner = self.inner.borrow_mut();
        inner.table.remove(ix)
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Default trait implementation for [FileTable].
impl Default for FileTable {
    fn default() -> Self {
        Self::new()
    }
}
