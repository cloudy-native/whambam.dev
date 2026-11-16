// SPDX-License-Identifier: MIT
//! Colors, with a monochrome fallback for --no-color / NO_COLOR.

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    color: bool,
}

impl Theme {
    pub fn new(color: bool) -> Self {
        Theme { color }
    }

    pub fn fg(&self, color: Color) -> Style {
        if self.color {
            Style::default().fg(color)
        } else {
            Style::default()
        }
    }

    pub fn throughput(&self) -> Style {
        self.fg(Color::Cyan)
    }

    pub fn latency(&self) -> Style {
        self.fg(Color::Yellow)
    }

    pub fn transfer(&self) -> Style {
        self.fg(Color::Magenta)
    }

    pub fn muted(&self) -> Style {
        self.fg(Color::Gray)
    }

    pub fn ok(&self) -> Style {
        self.fg(Color::Green)
    }

    pub fn bad(&self) -> Style {
        self.fg(Color::Red).add_modifier(Modifier::BOLD)
    }

    pub fn highlight(&self) -> Style {
        self.fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }

    /// Color for an HTTP status code by class.
    pub fn status(&self, code: u16) -> Style {
        let style = self.fg(match code / 100 {
            2 => Color::Green,
            3 => Color::Blue,
            4 => Color::Yellow,
            5 => Color::Red,
            _ => Color::White,
        });
        if code >= 400 {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }
}
