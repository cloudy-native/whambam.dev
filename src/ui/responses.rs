// SPDX-License-Identifier: MIT
//! Status codes and transport errors side by side.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Row, Table};

use super::theme::Theme;
use crate::stats::TestState;

pub fn draw(f: &mut Frame, area: Rect, s: &TestState, t: &Theme) {
    let [left, right] = Layout::horizontal([Constraint::Percentage(50); 2]).areas(area);
    let total = s.completed;

    let status_rows = s
        .status_counts
        .iter()
        .map(|(&code, &n)| counted_row(code.to_string(), n, total, t.status(code)));
    f.render_widget(table(status_rows, " HTTP status codes ", "Status", t), left);

    let mut error_rows: Vec<Row> = s
        .error_counts
        .iter()
        .map(|(kind, &n)| counted_row(kind.label().to_string(), n, total, t.bad()))
        .collect();
    if error_rows.is_empty() {
        error_rows.push(Row::new([Cell::from("none")]).style(t.ok()));
    }
    f.render_widget(table(error_rows, " Transport errors ", "Error", t), right);
}

fn counted_row(label: String, count: usize, total: usize, style: Style) -> Row<'static> {
    let pct = if total > 0 {
        100.0 * count as f64 / total as f64
    } else {
        0.0
    };
    Row::new([
        Cell::from(label).style(style),
        Cell::from(count.to_string()),
        Cell::from(format!("{pct:.1}%")),
    ])
}

fn table<'a>(
    rows: impl IntoIterator<Item = Row<'a>>,
    title: &'a str,
    first: &'a str,
    t: &Theme,
) -> Table<'a> {
    let widths = [
        Constraint::Percentage(50),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];
    Table::new(rows, widths)
        .header(Row::new([first, "Count", "Share"]).style(t.muted().add_modifier(Modifier::BOLD)))
        .block(Block::bordered().title(Line::raw(title)))
        .column_spacing(1)
}
