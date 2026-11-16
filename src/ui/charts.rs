// SPDX-License-Identifier: MIT
//! Throughput and latency charts.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph};

use super::format::{compact, latency, seconds, ticks};
use super::theme::Theme;
use crate::report::{Baseline, Metric, Metrics};
use crate::stats::TestState;

const HEADROOM: f64 = 1.1;

struct Bounds {
    x: [f64; 2],
    y_max: f64,
}

fn bounds(points: &[(f64, f64)], elapsed: f64) -> Bounds {
    let x0 = points.first().map_or(0.0, |p| p.0);
    let x1 = points.last().map_or(elapsed, |p| p.0).max(x0 + 0.1);
    let y = points.iter().map(|p| p.1).fold(0.0, f64::max) * HEADROOM;
    Bounds {
        x: [x0, x1],
        y_max: if y > 0.0 { y } else { 1.0 },
    }
}

fn time_axis(b: &Bounds, t: &Theme) -> Axis<'static> {
    Axis::default()
        .style(t.muted())
        .bounds(b.x)
        .labels(ticks(b.x[0], b.x[1], 5, |v| seconds(v, b.x[1] - b.x[0])))
}

fn line<'a>(points: &'a [(f64, f64)], style: Style) -> Dataset<'a> {
    Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(style)
        .data(points)
}

/// Vertical lines where the load was changed live.
fn markers(state: &TestState, x: [f64; 2], y_max: f64) -> Vec<Vec<(f64, f64)>> {
    state
        .adjustments
        .iter()
        .filter(|a| a.t >= x[0] && a.t <= x[1])
        .map(|a| vec![(a.t, 0.0), (a.t, y_max)])
        .collect()
}

/// A horizontal reference line across the chart.
fn hline(x: [f64; 2], y: f64) -> Vec<(f64, f64)> {
    vec![(x[0], y), (x[1], y)]
}

/// Datasets for markers and an optional baseline level, drawn under the data.
fn overlays<'a>(
    marks: &'a [Vec<(f64, f64)>],
    base: &'a [(f64, f64)],
    t: &Theme,
) -> Vec<Dataset<'a>> {
    let mut sets: Vec<Dataset> = marks.iter().map(|m| line(m, t.muted())).collect();
    if !base.is_empty() {
        sets.push(line(base, t.muted()).name("baseline"));
    }
    sets
}

/// Full-size throughput chart above a latency chart, with baseline levels.
pub fn draw_split(
    f: &mut Frame,
    area: Rect,
    state: &TestState,
    t: &Theme,
    baseline: Option<&Baseline>,
) {
    let [top, bottom] = Layout::vertical([Constraint::Percentage(50); 2]).areas(area);
    let elapsed = state.elapsed().as_secs_f64();
    let last = state.series.last();

    let rps = state.series.points(|s| s.rps);
    let mut b = bounds(&rps, elapsed);
    let base_rps = baseline.map(|bl| bl.metric(Metric::Rps));
    b.y_max = b.y_max.max(base_rps.unwrap_or(0.0) * HEADROOM);
    let base_line = base_rps.map(|y| hline(b.x, y)).unwrap_or_default();
    let marks = markers(state, b.x, b.y_max);
    let title = format!(
        " Throughput · now {} · overall {} req/s{} ",
        compact(last.map_or(0.0, |s| s.rps)),
        compact(state.rps()),
        base_rps
            .map(|y| format!(" · baseline {}", compact(y)))
            .unwrap_or_default()
    );
    let mut sets = overlays(&marks, &base_line, t);
    sets.push(line(&rps, t.throughput()));
    let chart = Chart::new(sets)
        .block(Block::bordered().title(Line::styled(title, t.throughput())))
        .x_axis(time_axis(&b, t))
        .y_axis(
            Axis::default()
                .style(t.muted())
                .bounds([0.0, b.y_max])
                .labels(ticks(0.0, b.y_max, 5, compact)),
        );
    f.render_widget(chart, top);

    let lat = state.series.points(|s| s.latency_ms);
    let mut b = bounds(&lat, elapsed);
    let base_mean = baseline.map(|bl| bl.metric(Metric::Mean));
    b.y_max = b.y_max.max(base_mean.unwrap_or(0.0) * HEADROOM);
    let base_line = base_mean.map(|y| hline(b.x, y)).unwrap_or_default();
    let marks = markers(state, b.x, b.y_max);
    let title = format!(
        " Mean latency · now {} · p99 {}{} ",
        latency(last.map_or(0.0, |s| s.latency_ms)),
        latency(state.percentile_ms(0.99)),
        base_mean
            .map(|y| format!(" · baseline mean {}", latency(y)))
            .unwrap_or_default()
    );
    let mut sets = overlays(&marks, &base_line, t);
    sets.push(line(&lat, t.latency()));
    let chart = Chart::new(sets)
        .block(Block::bordered().title(Line::styled(title, t.latency())))
        .x_axis(time_axis(&b, t))
        .y_axis(
            Axis::default()
                .style(t.muted())
                .bounds([0.0, b.y_max])
                .labels(ticks(0.0, b.y_max, 5, latency)),
        );
    f.render_widget(chart, bottom);
}

/// Throughput (left scale) and latency (right scale) on one time axis.
pub fn draw_combined(f: &mut Frame, area: Rect, state: &TestState, t: &Theme) {
    let block = Block::bordered()
        .title(Line::styled(" Throughput (req/s) ", t.throughput()))
        .title(Line::styled(" Mean latency ", t.latency()).alignment(Alignment::Right));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [plot, right] =
        Layout::horizontal([Constraint::Min(10), Constraint::Length(8)]).areas(inner);

    let elapsed = state.elapsed().as_secs_f64();
    let rps = state.series.points(|s| s.rps);
    let lat = state.series.points(|s| s.latency_ms);
    let rb = bounds(&rps, elapsed);
    let lb = bounds(&lat, elapsed);
    let scale = rb.y_max / lb.y_max;
    let lat_scaled: Vec<(f64, f64)> = lat.iter().map(|&(x, y)| (x, y * scale)).collect();
    let marks = markers(state, rb.x, rb.y_max);

    let mut sets = overlays(&marks, &[], t);
    sets.push(line(&rps, t.throughput()));
    sets.push(line(&lat_scaled, t.latency()));
    let chart = Chart::new(sets)
        .x_axis(time_axis(&rb, t))
        .y_axis(
            Axis::default()
                .style(t.throughput())
                .bounds([0.0, rb.y_max])
                .labels(ticks(0.0, rb.y_max, 4, compact)),
        )
        .hidden_legend_constraints((Constraint::Length(0), Constraint::Length(0)));
    f.render_widget(chart, plot);
    draw_right_axis(f, right, lb.y_max, t);
}

/// Latency labels down the right edge, aligned with the plot rows.
fn draw_right_axis(f: &mut Frame, area: Rect, max_ms: f64, t: &Theme) {
    // The bottom row is the x-axis label row.
    let rows = area.height.saturating_sub(2);
    if rows == 0 {
        return;
    }
    let labels: Vec<Line> = (0..=rows)
        .map(|r| match r {
            0 => latency(max_ms),
            r if r == rows / 2 => latency(max_ms / 2.0),
            r if r == rows => "0".to_string(),
            _ => String::new(),
        })
        .map(|s| Line::styled(s, t.latency()).alignment(Alignment::Right))
        .collect();
    f.render_widget(Paragraph::new(labels), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_have_headroom_and_never_collapse() {
        let b = bounds(&[], 0.0);
        assert!(b.x[1] > b.x[0]);
        assert_eq!(b.y_max, 1.0);
        let b = bounds(&[(1.0, 10.0), (2.0, 20.0)], 2.0);
        assert_eq!(b.x, [1.0, 2.0]);
        assert!((b.y_max - 22.0).abs() < 1e-9);
    }

    #[test]
    fn markers_only_inside_the_window() {
        let mut s = TestState::default();
        s.adjustments.push(crate::stats::Adjustment {
            t: 1.0,
            workers: 2,
            rate: None,
        });
        s.adjustments.push(crate::stats::Adjustment {
            t: 9.0,
            workers: 3,
            rate: None,
        });
        let m = markers(&s, [0.0, 5.0], 10.0);
        assert_eq!(m, vec![vec![(1.0, 0.0), (1.0, 10.0)]]);
        assert_eq!(hline([0.0, 2.0], 3.0), vec![(0.0, 3.0), (2.0, 3.0)]);
    }
}
