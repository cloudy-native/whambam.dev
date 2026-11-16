// SPDX-License-Identifier: MIT
//! Interactive terminal UI.

mod app;
mod charts;
mod dashboard;
mod format;
mod responses;
mod theme;
mod view;

pub use app::run;

use std::sync::Arc;

use crate::report::Baseline;

#[derive(Debug, Clone)]
pub struct UiOptions {
    pub color: bool,
    pub fps: u16,
    /// Shown alongside live numbers when running with --baseline.
    pub baseline: Option<Arc<Baseline>>,
}
