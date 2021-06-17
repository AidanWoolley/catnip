// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod threadunsafe;
mod threadsafe;

pub use self::threadunsafe::{SharedWaker, WakerU64};