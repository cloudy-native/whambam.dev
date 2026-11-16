// SPDX-License-Identifier: MIT
//! HTTP client construction, request execution and error classification.

mod client;
mod debug;
mod errors;
mod execute;
mod resolver;

pub use client::build_clients;
pub use debug::run_debug;
pub use errors::{ErrorKind, error_chain};
pub use execute::{Outcome, Sender};
