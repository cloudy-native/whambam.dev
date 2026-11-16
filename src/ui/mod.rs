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

#[derive(Debug, Clone, Copy)]
pub struct UiOptions {
    pub color: bool,
    pub fps: u16,
}
