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
use crate::report::{Baseline, Metric, Metrics};
use crate::stats::TestState;

/// One stat row: label, value, value style, and the metric to compare.
struct Stat {
    label: &'static str,
    value: String,
    style: Style,
    metric: Option<(Metric, f64)>,
}

fn stat(label: &'static str, value: String) -> Stat {
    Stat {
        label,
        value,
        style: Style::default(),
        metric: None,
    }
}

pub fn draw(f: &mut Frame, area: Rect, s: &TestState, t: &Theme, baseline: Option<&Baseline>) {
    let [stats, chart] = Layout::vertical([Constraint::Length(9), Constraint::Min(6)]).areas(area);
    let [left, mid, right] = Layout::horizontal([Constraint::Ratio(1, 3); 3]).areas(stats);
    let bad_if = |n: usize| if n > 0 { t.bad() } else { Style::default() };
    let now = s.series.last().map_or(0.0, |x| x.rps);

    let throughput = vec![
        stat("Requests", s.completed.to_string()),
        Stat {
            metric: Some((Metric::SuccessRate, s.success_rate())),
            ..stat("Succeeded", format!("{:.1}%", s.success_rate()))
        },
        Stat {
            style: bad_if(s.failures()),
            ..stat("Failed", s.failures().to_string())
        },
        stat("Now", format!("{} req/s", compact(now))),
        Stat {
            metric: Some((Metric::Rps, s.rps())),
            ..stat("Overall", format!("{} req/s", compact(s.rps())))
        },
        stat("Elapsed", format!("{:.1}s", s.elapsed().as_secs_f64())),
    ];
    panel(
        f,
        left,
        " Throughput ",
        t.throughput(),
        throughput,
        baseline,
        t,
    );

    let lat = |label, ms: f64, metric: Option<Metric>| Stat {
        metric: metric.map(|m| (m, ms)),
        ..stat(label, latency(ms))
    };
    let latencies = vec![
        lat("Fastest", s.min_ms(), Some(Metric::Min)),
        lat("p50", s.percentile_ms(0.5), Some(Metric::P50)),
        lat("p90", s.percentile_ms(0.9), Some(Metric::P90)),
        lat("p99", s.percentile_ms(0.99), Some(Metric::P99)),
        lat("Slowest", s.max_ms(), Some(Metric::Max)),
        lat("Mean", s.mean_ms(), Some(Metric::Mean)),
        lat("Std dev", s.stdev_ms(), None),
    ];
    panel(f, mid, " Latency ", t.latency(), latencies, baseline, t);

    let per = |total: u64| bytes(total.checked_div(s.completed as u64).unwrap_or(0));
    let errors = s.transport_errors();
    let data = vec![
        stat("Received", bytes(s.bytes_received)),
        stat("Sent (approx)", bytes(s.bytes_sent)),
        stat("Per response", per(s.bytes_received)),
        stat("Per request", per(s.bytes_sent)),
        Stat {
            style: bad_if(errors),
            ..stat("Errors", errors.to_string())
        },
    ];
    panel(f, right, " Data ", t.transfer(), data, None, t);

    draw_combined(f, chart, s, t);
}

/// " +8%" against the baseline, red when worse.
fn delta(metric: Metric, current: f64, baseline: &Baseline, t: &Theme) -> Option<Span<'static>> {
    let base = baseline.metric(metric);
    if base == 0.0 || current == 0.0 {
        return None;
    }
    let pct = (current - base) / base * 100.0;
    let worse = if metric.higher_is_better() {
        pct < 0.0
    } else {
        pct > 0.0
    };
    let style = if worse && pct.abs() >= 1.0 {
        t.bad()
    } else {
        t.muted()
    };
    let text = if pct.abs() < 0.5 {
        " ±0%".to_string()
    } else {
        format!(" {pct:+.0}%")
    };
    Some(Span::styled(text, style))
}

fn panel(
    f: &mut Frame,
    area: Rect,
    title: &str,
    style: Style,
    stats: Vec<Stat>,
    baseline: Option<&Baseline>,
    t: &Theme,
) {
    let lines: Vec<Line> = stats
        .into_iter()
        .map(|st| {
            let mut spans = vec![
                Span::raw(format!("{:<14}", st.label)),
                Span::styled(st.value, st.style),
            ];
            if let (Some((metric, value)), Some(b)) = (st.metric, baseline) {
                spans.extend(delta(metric, value, b, t));
            }
            Line::from(spans)
        })
        .collect();
    let mut block = Block::bordered().title(Line::styled(title.to_string(), style));
    if baseline.is_some() {
        block = block.title_bottom(Line::styled(" vs baseline ", t.muted()));
    }
    f.render_widget(Paragraph::new(lines).block(block), area);
}
