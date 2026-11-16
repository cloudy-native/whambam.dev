// SPDX-License-Identifier: MIT
//! Dashboard: three stat panels above the combined chart.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::charts::draw_combined;
use super::format::{bytes, compact, latency};
use super::theme::Theme;
use crate::stats::TestState;

pub fn draw(f: &mut Frame, area: Rect, s: &TestState, t: &Theme) {
    let [stats, chart] = Layout::vertical([Constraint::Length(9), Constraint::Min(6)]).areas(area);
    let [left, mid, right] = Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(stats);

    let failures_style = if s.failures() > 0 {
        t.bad()
    } else {
        Style::default()
    };
    let current = s.series.last().map_or(0.0, |x| x.rps);
    panel(
        f,
        left,
        " Throughput ",
        t.throughput(),
        vec![
            row("Requests", s.completed.to_string(), Style::default()),
            row(
                "Succeeded",
                format!("{:.1}%", s.success_rate()),
                Style::default(),
            ),
            row("Failed", s.failures().to_string(), failures_style),
            row(
                "Now",
                format!("{} req/s", compact(current)),
                Style::default(),
            ),
            row(
                "Overall",
                format!("{} req/s", compact(s.rps())),
                Style::default(),
            ),
            row(
                "Elapsed",
                format!("{:.1}s", s.elapsed().as_secs_f64()),
                Style::default(),
            ),
        ],
    );

    let lat = |ms: f64| (latency(ms), Style::default());
    let rows = [
        ("Fastest", s.min_ms()),
        ("p50", s.percentile_ms(0.5)),
        ("p90", s.percentile_ms(0.9)),
        ("p99", s.percentile_ms(0.99)),
        ("Slowest", s.max_ms()),
        ("Mean", s.mean_ms()),
        ("Std dev", s.stdev_ms()),
    ];
    let lines = rows
        .iter()
        .map(|&(label, ms)| {
            let (v, style) = lat(ms);
            row(label, v, style)
        })
        .collect();
    panel(f, mid, " Latency ", t.latency(), lines);

    let per = |total: u64| bytes(total.checked_div(s.completed as u64).unwrap_or(0));
    panel(
        f,
        right,
        " Data ",
        t.transfer(),
        vec![
            row("Received", bytes(s.bytes_received), Style::default()),
            row("Sent (approx)", bytes(s.bytes_sent), Style::default()),
            row("Per response", per(s.bytes_received), Style::default()),
            row("Per request", per(s.bytes_sent), Style::default()),
            row(
                "Errors",
                s.transport_errors().to_string(),
                if s.transport_errors() > 0 {
                    t.bad()
                } else {
                    Style::default()
                },
            ),
        ],
    );

    draw_combined(f, chart, s, t);
}

fn row(label: &str, value: String, style: Style) -> Line<'static> {
    Line::from(vec![
        Span::raw(format!("{label:<14}")),
        Span::styled(value, style),
    ])
}

fn panel(f: &mut Frame, area: Rect, title: &str, style: Style, lines: Vec<Line<'static>>) {
    let block = Block::bordered().title(Line::styled(title.to_string(), style));
    f.render_widget(Paragraph::new(lines).block(block), area);
}
