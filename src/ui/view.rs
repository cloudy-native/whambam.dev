// SPDX-License-Identifier: MIT
//! Top-level layout: header, tabs, the selected tab, footer and help.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Tabs};

use super::theme::Theme;
use super::{charts, dashboard, responses};
use crate::cli::HttpVersion;
use crate::config::{Pacing, TestConfig};
use crate::stats::TestState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Charts,
    Responses,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Dashboard, Tab::Charts, Tab::Responses];

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn next(self) -> Tab {
        Tab::ALL[(self.index() + 1) % Tab::ALL.len()]
    }

    pub fn prev(self) -> Tab {
        Tab::ALL[(self.index() + Tab::ALL.len() - 1) % Tab::ALL.len()]
    }

    pub fn from_key(c: char) -> Option<Tab> {
        c.to_digit(10)
            .and_then(|d| (d as usize).checked_sub(1))
            .and_then(|i| Tab::ALL.get(i).copied())
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Dashboard => "1 Dashboard",
            Tab::Charts => "2 Charts",
            Tab::Responses => "3 Status & errors",
        }
    }
}

pub struct ViewCtx<'a> {
    pub config: &'a TestConfig,
    pub theme: Theme,
    pub tab: Tab,
    pub help: bool,
}

pub fn draw(f: &mut Frame, state: &TestState, ctx: &ViewCtx) {
    let [header, tabs, body, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_header(f, header, state, ctx);
    let titles = Tab::ALL.iter().map(|t| t.title());
    f.render_widget(
        Tabs::new(titles)
            .select(ctx.tab.index())
            .highlight_style(ctx.theme.highlight())
            .style(ctx.theme.muted()),
        tabs,
    );
    match ctx.tab {
        Tab::Dashboard => dashboard::draw(f, body, state, &ctx.theme),
        Tab::Charts => charts::draw_split(f, body, state, &ctx.theme),
        Tab::Responses => responses::draw(f, body, state, &ctx.theme),
    }
    let keys = if state.is_complete() {
        " r restart · ←/→ or 1-3 tabs · h help · q quit"
    } else {
        " ←/→ or 1-3 tabs · h help · q quit"
    };
    f.render_widget(Paragraph::new(keys).style(ctx.theme.muted()), footer);
    if ctx.help {
        draw_help(f, f.area(), &ctx.theme);
    }
}

fn draw_header(f: &mut Frame, area: Rect, state: &TestState, ctx: &ViewCtx) {
    let t = &ctx.theme;
    let (status, style) = match (state.is_complete(), state.interrupted) {
        (false, _) => ("RUNNING", t.ok()),
        (true, false) => ("DONE", t.throughput()),
        (true, true) => ("STOPPED", t.latency()),
    };
    let req = &ctx.config.request;
    let mut first = vec![
        Span::styled(status, style.add_modifier(Modifier::BOLD)),
        Span::raw(format!(" {} {}", req.method, req.display_url)),
        Span::styled(
            format!("  {:.1}s", state.elapsed().as_secs_f64()),
            t.muted(),
        ),
    ];
    if let Some(p) = progress(state, ctx.config) {
        first.push(Span::styled(format!("  {:.0}%", p * 100.0), t.highlight()));
    }
    let second = Line::styled(describe_load(ctx.config), t.muted());
    let block = Block::bordered().title(" whambam ").border_style(t.muted());
    f.render_widget(
        Paragraph::new(vec![Line::from(first), second]).block(block),
        area,
    );
}

/// Fraction complete for request- or time-bounded runs.
fn progress(state: &TestState, config: &TestConfig) -> Option<f64> {
    if state.is_complete() {
        return None;
    }
    let p = match (config.requests, config.duration) {
        (Some(n), _) if n > 0 => state.completed as f64 / n as f64,
        (_, Some(d)) => state.elapsed().as_secs_f64() / d.as_secs_f64(),
        _ => return None,
    };
    Some(p.clamp(0.0, 1.0))
}

pub fn describe_load(c: &TestConfig) -> String {
    let version = match c.net.http_version {
        HttpVersion::Http11 => "HTTP/1.1".to_string(),
        HttpVersion::Http2 => format!("HTTP/2 ×{} streams", c.http2_parallel),
    };
    let pacing = match c.pacing {
        Pacing::Unlimited => "unthrottled".to_string(),
        Pacing::PerWorker(q) => format!("{q} req/s per worker"),
        Pacing::Global(q) => format!("{q} req/s total, latency corrected"),
    };
    let limit = match (c.requests, c.duration) {
        (_, Some(d)) => format!("for {:.0?}", d),
        (Some(n), None) => format!("{n} requests"),
        (None, None) => "until stopped".to_string(),
    };
    format!("{} workers · {version} · {pacing} · {limit}", c.concurrency)
}

fn draw_help(f: &mut Frame, area: Rect, t: &Theme) {
    let lines = [
        "q / Esc / Ctrl-C   quit and print the report",
        "r                  restart a finished test",
        "1 2 3 / ← →        switch tabs",
        "h / ?              toggle this help",
    ];
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(6),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, popup, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(50),
        Constraint::Fill(1),
    ])
    .areas(mid);
    f.render_widget(Clear, popup);
    let text: Vec<Line> = lines.iter().map(|l| Line::raw(*l)).collect();
    f.render_widget(
        Paragraph::new(text).block(
            Block::bordered()
                .title(" Help ")
                .border_style(t.highlight()),
        ),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_cycle_and_map_keys() {
        assert_eq!(Tab::Dashboard.next(), Tab::Charts);
        assert_eq!(Tab::Dashboard.prev(), Tab::Responses);
        assert_eq!(Tab::from_key('3'), Some(Tab::Responses));
        assert_eq!(Tab::from_key('0'), None);
        assert_eq!(Tab::from_key('9'), None);
    }
}
