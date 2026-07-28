// whambam - A high-performance HTTP load testing tool
//
// Copyright (c) 2025 Stephen Harrison
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{
        Axis, Block, Borders, Chart, Clear, Dataset, GraphType, Paragraph, Row, Table, Tabs,
    },
    Frame,
};

use super::app::UiState;
use crate::tester::TestState;

/// Helper function to create time axis labels
fn create_time_axis_labels(min: f64, max: f64, num_labels: usize) -> Vec<Span<'static>> {
    let mut labels = Vec::with_capacity(num_labels);
    let range = max - min;

    for i in 0..num_labels {
        let value = min + (range * i as f64) / (num_labels - 1) as f64;
        // Round to whole seconds for time
        let formatted = format!("{}", value.round() as i64);
        labels.push(Span::styled(formatted, Style::default().fg(Color::Gray)));
    }

    labels
}

/// Helper function to create throughput axis labels
fn create_throughput_axis_labels(min: f64, max: f64, num_labels: usize) -> Vec<Span<'static>> {
    let mut labels = Vec::with_capacity(num_labels);
    let range = max - min;

    for i in 0..num_labels {
        let value = min + (range * i as f64) / (num_labels - 1) as f64;
        // Use sensible rounding based on the value range
        let formatted = if max <= 10.0 {
            // For small values, show 1 decimal place
            format!("{value:.1}")
        } else if max <= 100.0 {
            // For medium values, round to whole numbers
            format!("{}", value.round() as i64)
        } else {
            // For large values, round to nearest 10
            format!("{}", ((value / 10.0).round() * 10.0) as i64)
        };

        labels.push(Span::styled(formatted, Style::default().fg(Color::Gray)));
    }

    labels
}

/// Helper function to create latency axis labels with appropriate units
fn create_latency_axis_labels(min: f64, max: f64, num_labels: usize) -> Vec<Span<'static>> {
    let mut labels = Vec::with_capacity(num_labels);
    let range = max - min;

    for i in 0..num_labels {
        let value = min + (range * i as f64) / (num_labels - 1) as f64;

        // Always display with 1 decimal place and appropriate units
        let (value_adj, unit) = if value < 1.0 {
            // Microseconds
            (value * 1000.0, "μs")
        } else if value < 1000.0 {
            // Milliseconds
            (value, "ms")
        } else {
            // Seconds
            (value / 1000.0, "s")
        };

        // Always use 1 decimal place
        let formatted = format!("{value_adj:.1}{unit}");

        labels.push(Span::styled(formatted, Style::default().fg(Color::Gray)));
    }

    labels
}

/// Configuration for chart creation
struct ChartConfig<'a> {
    data: &'a [(f64, f64)],
    title: &'a str,
    marker: symbols::Marker,
    x_min: f64,
    x_max: f64,
    y_max: f64,
    num_x_labels: usize,
    num_y_labels: usize,
}

/// Ensure axis ranges are valid (single-point / empty short runs collapse otherwise).
fn normalize_chart_bounds(x_min: f64, x_max: f64, y_max: f64) -> (f64, f64, f64) {
    let mut x0 = x_min;
    let mut x1 = x_max;
    if !x0.is_finite() {
        x0 = 0.0;
    }
    if !x1.is_finite() || x1 <= x0 {
        x1 = x0 + 0.1;
    }
    let y = if y_max.is_finite() && y_max > 0.0 {
        y_max
    } else {
        1.0
    };
    (x0, x1, y)
}

/// Create a throughput chart with the given parameters
fn create_throughput_chart<'a>(config: ChartConfig<'a>) -> Chart<'a> {
    let (x_min, x_max, y_max) = normalize_chart_bounds(config.x_min, config.x_max, config.y_max);

    let throughput_dataset = vec![Dataset::default()
        .name("Throughput (req/s)")
        .marker(config.marker)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Cyan))
        .data(config.data)];

    // Create axis labels
    let x_labels = create_time_axis_labels(x_min, x_max, config.num_x_labels);
    let y_labels = create_throughput_axis_labels(0.0, y_max, config.num_y_labels);

    // Create and return the chart
    Chart::new(throughput_dataset)
        .block(
            Block::default()
                .title(Span::styled(config.title, Style::default().fg(Color::Cyan)))
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("Req/s", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, y_max])
                .labels(y_labels),
        )
}

/// Create a latency chart with the given parameters
fn create_latency_chart<'a>(config: ChartConfig<'a>) -> Chart<'a> {
    let (x_min, x_max, y_max) = normalize_chart_bounds(config.x_min, config.x_max, config.y_max);

    let latency_dataset = vec![Dataset::default()
        .name("Latency (ms)")
        .marker(config.marker)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Yellow))
        .data(config.data)];

    // Create axis labels
    let x_labels = create_time_axis_labels(x_min, x_max, config.num_x_labels);
    let y_labels = create_latency_axis_labels(0.0, y_max, config.num_y_labels);

    // Create and return the chart
    Chart::new(latency_dataset)
        .block(
            Block::default()
                .title(Span::styled(
                    config.title,
                    Style::default().fg(Color::Yellow),
                ))
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, y_max])
                .labels(y_labels),
        )
}

/// Format a short latency value for chart titles.
fn format_latency_short(latency_ms: f64) -> String {
    if latency_ms < 1.0 {
        format!("{:.0}μs", latency_ms * 1000.0)
    } else if latency_ms < 1000.0 {
        format!("{latency_ms:.1}ms")
    } else {
        format!("{:.2}s", latency_ms / 1000.0)
    }
}

/// Overall RPS from completed requests / elapsed wall time.
fn overall_throughput(app_state: &TestState) -> f64 {
    let elapsed = if app_state.is_complete && app_state.end_time.is_some() {
        app_state
            .end_time
            .unwrap()
            .duration_since(app_state.start_time)
            .as_secs_f64()
    } else {
        app_state.start_time.elapsed().as_secs_f64()
    };
    if elapsed > 0.0 && app_state.completed_requests > 0 {
        app_state.completed_requests as f64 / elapsed
    } else {
        0.0
    }
}

/// Last sampled chart y-value, or 0 if empty.
fn last_series_y(data: &[(f64, f64)]) -> f64 {
    data.last().map(|&(_, y)| y).unwrap_or(0.0)
}

/// Mean of series y-values (for latency chart title).
fn mean_series_y(data: &[(f64, f64)]) -> f64 {
    if data.is_empty() {
        0.0
    } else {
        data.iter().map(|&(_, y)| y).sum::<f64>() / data.len() as f64
    }
}

/// Max y in a series, floored to at least `floor`.
fn series_y_max(data: &[(f64, f64)], floor: f64) -> f64 {
    data.iter()
        .map(|&(_, y)| y)
        .fold(floor, |max, y| max.max(y))
}

/// Map latency (ms) into the throughput (req/s) drawing scale so both series share one Y axis.
fn normalize_latency_for_overlay(
    latency_data: &[(f64, f64)],
    lat_max: f64,
    thr_max: f64,
) -> Vec<(f64, f64)> {
    if lat_max <= 0.0 || thr_max <= 0.0 {
        return latency_data.iter().map(|&(t, _)| (t, 0.0)).collect();
    }
    let scale = thr_max / lat_max;
    latency_data
        .iter()
        .map(|&(t, lat)| (t, lat * scale))
        .collect()
}

/// Numeric tick for the right latency axis (unit is shown only on the axis title).
fn format_ms_tick(latency_ms: f64) -> String {
    if latency_ms < 0.01 {
        format!("{latency_ms:.3}")
    } else if latency_ms < 1.0 {
        format!("{latency_ms:.2}")
    } else if latency_ms < 100.0 {
        format!("{latency_ms:.1}")
    } else {
        format!("{:.0}", latency_ms.round())
    }
}

/// Right-hand latency scale: bare numeric ticks (unit is in the header).
fn render_right_latency_axis<B: Backend>(f: &mut Frame<B>, area: Rect, lat_max: f64) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Align ticks with chart plot area (leave space for bottom X-axis labels)
    let ticks_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.saturating_sub(1),
    };
    if ticks_area.height == 0 {
        return;
    }

    let label_rows: Vec<u16> = if ticks_area.height == 1 {
        vec![0]
    } else if ticks_area.height == 2 {
        vec![0, 1]
    } else {
        vec![
            0,
            ticks_area.height / 2,
            ticks_area.height.saturating_sub(1),
        ]
    };

    for &row in &label_rows {
        let t = if ticks_area.height <= 1 {
            1.0
        } else {
            1.0 - (row as f64) / (ticks_area.height.saturating_sub(1) as f64)
        };
        let lat = lat_max * t;
        let cell = Rect {
            x: ticks_area.x,
            y: ticks_area.y + row,
            width: ticks_area.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(format_ms_tick(lat))
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Right),
            cell,
        );
    }
}

/// Dashboard-only combined throughput + latency chart (shared X, dual Y via normalization).
fn render_dashboard_combined_chart<B: Backend>(
    f: &mut Frame<B>,
    app_state: &TestState,
    area: Rect,
) {
    let throughput_data: Vec<(f64, f64)> = app_state.throughput_data.clone().into();
    let latency_data: Vec<(f64, f64)> = app_state.latency_data.clone().into();

    let mut thr_max = series_y_max(&throughput_data, 1.0) * 1.1;
    let lat_max = {
        let m = series_y_max(&latency_data, 1.0) * 1.1;
        if m.is_finite() && m > 0.0 {
            m
        } else {
            1.0
        }
    };

    let x_min = throughput_data
        .first()
        .or(latency_data.first())
        .map(|&(x, _)| x)
        .unwrap_or(0.0);
    let x_max = throughput_data
        .last()
        .map(|&(x, _)| x)
        .into_iter()
        .chain(latency_data.last().map(|&(x, _)| x))
        .fold(x_min, f64::max);
    let (x_min, x_max, y) = normalize_chart_bounds(x_min, x_max, thr_max);
    thr_max = y;

    let latency_scaled = normalize_latency_for_overlay(&latency_data, lat_max, thr_max);

    let overall_tps = overall_throughput(app_state);
    let last_tps = last_series_y(&throughput_data);
    let last_lat = last_series_y(&latency_data);
    let avg_lat = mean_series_y(&latency_data);

    // Outer frame; header row owns units so Y ticks stay bare numbers
    let outer = Block::default().borders(Borders::ALL);
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    // Header: left = throughput + req/s, right = latency + ms (right-aligned)
    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(vertical[0]);

    let thr_header = format!("last {last_tps:.0} · overall {overall_tps:.0} req/s");
    f.render_widget(
        Paragraph::new(Span::styled(thr_header, Style::default().fg(Color::Cyan))),
        header[0],
    );

    let lat_header = format!(
        "last {} · avg {} ms",
        format_ms_tick(last_lat),
        format_ms_tick(avg_lat)
    );
    f.render_widget(
        Paragraph::new(Span::styled(lat_header, Style::default().fg(Color::Yellow)))
            .alignment(Alignment::Right),
        header[1],
    );

    // Body: chart | right latency ticks
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(12), Constraint::Length(7)])
        .split(vertical[1]);

    let datasets = vec![
        Dataset::default()
            .name("throughput")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&throughput_data),
        Dataset::default()
            .name("latency")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
            .data(&latency_scaled),
    ];

    let x_labels = create_time_axis_labels(x_min, x_max, 5);
    let y_labels = create_throughput_axis_labels(0.0, thr_max, 4);

    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([x_min, x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                // No unit title here — it collides with the top tick (e.g. "15130req/s")
                .style(Style::default().fg(Color::Cyan))
                .bounds([0.0, thr_max])
                .labels(y_labels),
        )
        .hidden_legend_constraints((Constraint::Length(0), Constraint::Length(0)));

    f.render_widget(chart, body[0]);
    render_right_latency_axis(f, body[1], lat_max);
}

/// Main UI render function
pub fn ui<B: Backend>(f: &mut Frame<B>, app_state: &TestState, ui_state: &UiState) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title and status
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Content
        ])
        .split(f.size());

    // Title and status with correct elapsed time
    let elapsed = if app_state.is_complete && app_state.end_time.is_some() {
        // For completed tests, use the frozen end time
        app_state
            .end_time
            .unwrap()
            .duration_since(app_state.start_time)
            .as_secs_f64()
    } else {
        // For running tests, use current elapsed time
        app_state.start_time.elapsed().as_secs_f64()
    };
    let status = if app_state.is_complete {
        "COMPLETED"
    } else {
        "RUNNING"
    };
    let title = format!(
        "WHAMBAM - {} - {} for {:.1}s",
        app_state.url, status, elapsed
    );

    // Add key help
    let key_help = if app_state.is_complete {
        " (Press 'r' to restart, 'q' to quit)"
    } else {
        " (Press 'q' to quit)"
    };

    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let full_title = format!("{title}{key_help}");
    let color = if app_state.is_complete {
        Color::Blue
    } else {
        Color::Green
    };
    let title_text = Paragraph::new(full_title.as_str())
        .style(Style::default().fg(color))
        .block(title_block);

    f.render_widget(title_text, chunks[0]);

    // Tabs
    let tab_titles = vec!["Dashboard ('1')", "Charts ('2')", "Status Codes ('3')"];
    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL))
        .select(ui_state.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, chunks[1]);

    // Main content based on selected tab
    match ui_state.selected_tab {
        0 => render_dashboard(f, app_state, chunks[2]),
        1 => render_charts(f, app_state, chunks[2]),
        2 => render_status_codes(f, app_state, chunks[2]),
        _ => {}
    }

    // Help overlay if enabled
    if ui_state.show_help {
        render_help(f, f.size());
    }
}

/// Render the dashboard tab
fn render_dashboard<B: Backend>(f: &mut Frame<B>, app_state: &TestState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Stats
            Constraint::Percentage(60), // Mini charts
        ])
        .split(area);

    // Stats section
    let stat_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33), // Throughput stats
            Constraint::Percentage(33), // Latency stats
            Constraint::Percentage(34), // Byte stats
        ])
        .split(chunks[0]);

    // Throughput stats
    let completed = app_state.completed_requests;
    let errors = app_state.error_count;
    let success_rate = if completed > 0 {
        100.0 * (completed - errors) as f64 / completed as f64
    } else {
        100.0
    };

    // Get elapsed time - same as title calculation for consistency
    let elapsed = if app_state.is_complete && app_state.end_time.is_some() {
        app_state
            .end_time
            .unwrap()
            .duration_since(app_state.start_time)
            .as_secs_f64()
    } else {
        app_state.start_time.elapsed().as_secs_f64()
    };

    let overall_tps = if elapsed > 0.0 {
        completed as f64 / elapsed
    } else {
        0.0
    };

    let throughput_stats = [
        format!("Completed Requests: {completed}"),
        format!("Error Count: {errors}"),
        format!("Success Rate: {success_rate:.1}%"),
        format!(
            "Current Throughput: {:.1} req/s",
            app_state.current_throughput
        ),
        format!("Overall Throughput: {overall_tps:.1} req/s"),
        format!("Elapsed Time: {elapsed:.1}s"),
    ];

    let throughput_block = Block::default()
        .title(Span::styled(
            "Throughput snapshot",
            Style::default().fg(Color::Cyan),
        ))
        .borders(Borders::ALL);

    let throughput_stats_str = throughput_stats.join("\n");
    let throughput_text = Paragraph::new(throughput_stats_str.as_str())
        .style(Style::default().fg(Color::White))
        .block(throughput_block);

    f.render_widget(throughput_text, stat_chunks[0]);

    // Latency stats
    let min = if app_state.min_latency == f64::MAX {
        0.0
    } else {
        app_state.min_latency
    };

    // Helper function to format latency with appropriate units and hide trailing zeros
    let format_latency = |latency_ms: f64| -> String {
        let (value, unit) = if latency_ms < 1.0 {
            // Microseconds
            (latency_ms * 1000.0, "μs")
        } else if latency_ms < 1000.0 {
            // Milliseconds
            (latency_ms, "ms")
        } else {
            // Seconds
            (latency_ms / 1000.0, "s")
        };

        // Check if the fractional part is zero
        if value.fract() == 0.0 {
            format!("{} {}", value as i64, unit)
        } else {
            format!("{value:.3} {unit}")
        }
    };

    let latency_stats = [
        format!("Min Latency: {}", format_latency(min)),
        format!("Max Latency: {}", format_latency(app_state.max_latency)),
        format!("P50 Latency: {}", format_latency(app_state.p50_latency)),
        format!("P90 Latency: {}", format_latency(app_state.p90_latency)),
        format!("P95 Latency: {}", format_latency(app_state.p95_latency)),
        format!("P99 Latency: {}", format_latency(app_state.p99_latency)),
    ];

    let latency_block = Block::default()
        .title(Span::styled(
            "Latency snapshot",
            Style::default().fg(Color::Yellow),
        ))
        .borders(Borders::ALL);

    let latency_stats_str = latency_stats.join("\n");
    let latency_text = Paragraph::new(latency_stats_str.as_str())
        .style(Style::default().fg(Color::White))
        .block(latency_block);

    f.render_widget(latency_text, stat_chunks[1]);

    // Byte stats
    let format_bytes = |bytes: u64| -> String {
        if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    };

    let byte_stats = [
        format!("Bytes Sent: {}", format_bytes(app_state.total_bytes_sent)),
        format!(
            "Bytes Received: {}",
            format_bytes(app_state.total_bytes_received)
        ),
        format!(
            "Total Bytes: {}",
            format_bytes(app_state.total_bytes_sent + app_state.total_bytes_received)
        ),
        format!(
            "Avg Req Size: {}",
            if completed > 0 {
                format_bytes(app_state.total_bytes_sent / completed as u64)
            } else {
                "0 B".to_string()
            }
        ),
        format!(
            "Avg Resp Size: {}",
            if completed > 0 {
                format_bytes(app_state.total_bytes_received / completed as u64)
            } else {
                "0 B".to_string()
            }
        ),
    ];

    let byte_block = Block::default()
        .title(Span::styled(
            "Data Transfer snapshot",
            Style::default().fg(Color::Magenta),
        ))
        .borders(Borders::ALL);

    let byte_stats_str = byte_stats.join("\n");
    let byte_text = Paragraph::new(byte_stats_str.as_str())
        .style(Style::default().fg(Color::White))
        .block(byte_block);

    f.render_widget(byte_text, stat_chunks[2]);

    // Combined throughput + latency (shared time axis; latency on right scale)
    render_dashboard_combined_chart(f, app_state, chunks[1]);
}

/// Render the charts tab
fn render_charts<B: Backend>(f: &mut Frame<B>, app_state: &TestState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Throughput chart
            Constraint::Percentage(50), // Latency chart
        ])
        .split(area);

    // Throughput chart (full size)
    let throughput_data: Vec<(f64, f64)> = app_state.throughput_data.clone().into();
    let max_throughput = throughput_data
        .iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    let x_min = throughput_data.first().map(|&(x, _)| x).unwrap_or(0.0);
    let x_max = throughput_data.last().map(|&(x, _)| x).unwrap_or(30.0);
    let y_max = max_throughput * 1.1;

    let overall_tps = overall_throughput(app_state);
    let last_tps = last_series_y(&throughput_data);
    let throughput_title =
        format!("Throughput over time · last {last_tps:.0} r/s · overall {overall_tps:.0} r/s");

    let throughput_chart = create_throughput_chart(ChartConfig {
        data: &throughput_data,
        title: &throughput_title,
        marker: symbols::Marker::Braille,
        x_min,
        x_max,
        y_max,
        num_x_labels: 6,
        num_y_labels: 6,
    });

    f.render_widget(throughput_chart, chunks[0]);

    // Latency chart (full size)
    let latency_data: Vec<(f64, f64)> = app_state.latency_data.clone().into();
    let max_latency = latency_data
        .iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    let l_x_min = latency_data.first().map(|&(x, _)| x).unwrap_or(0.0);
    let l_x_max = latency_data.last().map(|&(x, _)| x).unwrap_or(30.0);
    let l_y_max = max_latency * 1.1;

    let last_lat = last_series_y(&latency_data);
    let avg_lat = mean_series_y(&latency_data);
    let latency_title = format!(
        "Latency over time · last {} · avg {}",
        format_latency_short(last_lat),
        format_latency_short(avg_lat)
    );

    let latency_chart = create_latency_chart(ChartConfig {
        data: &latency_data,
        title: &latency_title,
        marker: symbols::Marker::Braille,
        x_min: l_x_min,
        x_max: l_x_max,
        y_max: l_y_max,
        num_x_labels: 6,
        num_y_labels: 6,
    });

    f.render_widget(latency_chart, chunks[1]);
}

/// Render the status codes tab
fn render_status_codes<B: Backend>(f: &mut Frame<B>, app_state: &TestState, area: Rect) {
    // Create a table of status codes
    let mut status_rows = Vec::new();
    let mut status_codes: Vec<u16> = app_state.status_counts.keys().cloned().collect();
    status_codes.sort();

    let total_requests = app_state.completed_requests as f64;

    for status in status_codes {
        let count = *app_state.status_counts.get(&status).unwrap_or(&0);
        let percentage = if total_requests > 0.0 {
            (count as f64 / total_requests) * 100.0
        } else {
            0.0
        };

        let status_class = status / 100;
        let color = match status_class {
            2 => Color::Green,
            3 => Color::Blue,
            4 => Color::Yellow,
            5 => Color::Red,
            _ => Color::White,
        };

        // Mark non-2xx status codes as errors in the UI as well
        let is_error = status_class != 2;
        let style = if is_error {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        let status_text = Span::styled(format!("{status}"), style);

        status_rows.push(Row::new(vec![
            status_text.content.to_string(),
            format!("{}", count),
            format!("{:.1}%", percentage),
        ]));
    }

    // Add error row if there were any errors
    if app_state.error_count > 0 {
        let error_percentage = if total_requests > 0.0 {
            (app_state.error_count as f64 / total_requests) * 100.0
        } else {
            0.0
        };

        let error_text = Span::styled(
            "Connection Error",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        );

        status_rows.push(Row::new(vec![
            error_text.content.to_string(),
            format!("{}", app_state.error_count),
            format!("{:.1}%", error_percentage),
        ]));
    }

    let header_cells = ["Status Code", "Count", "Percentage"]
        .iter()
        .map(|h| (*h).to_string());

    let header = Row::new(header_cells).style(Style::default()).height(1);

    let table = Table::new(status_rows)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    "HTTP Status Codes",
                    Style::default().fg(Color::White),
                ))
                .borders(Borders::ALL),
        )
        .widths(&[
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .column_spacing(1);

    f.render_widget(table, area);
}

/// Render the help overlay
fn render_help<B: Backend>(f: &mut Frame<B>, area: Rect) {
    // Calculate centered box area
    let help_area = centered_rect(50, 40, area);

    // Create a simple block with a clean look
    let help_block = Block::default()
        .title(Span::styled(" Help ", Style::default().fg(Color::White)))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

    // Help text content
    let help_text = [
        "Press 'q' to quit",
        "Press 'r' to restart completed test",
        "Press 'h' to toggle this help overlay",
        "Press '1' to view Dashboard",
        "Press '2' to view Charts",
        "Press '3' to view Status Codes",
    ]
    .join("\n");

    // Create paragraph inside the block
    let help_paragraph = Paragraph::new(help_text)
        .block(help_block)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .alignment(ratatui::layout::Alignment::Center);

    // Clear the area with black background first
    f.render_widget(Clear, help_area);

    // Then render the help text with block
    f.render_widget(help_paragraph, help_area);
}

/// Helper function to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
