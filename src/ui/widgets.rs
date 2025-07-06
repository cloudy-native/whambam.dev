use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, Paragraph, Row, Table, Tabs},
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

/// Create a throughput chart with the given parameters
fn create_throughput_chart<'a>(config: ChartConfig<'a>) -> Chart<'a> {
    let throughput_dataset = vec![Dataset::default()
        .name("Throughput (req/s)")
        .marker(config.marker)
        .style(Style::default().fg(Color::Cyan))
        .data(config.data)];

    // Create axis labels
    let x_labels = create_time_axis_labels(config.x_min, config.x_max, config.num_x_labels);
    let y_labels = create_throughput_axis_labels(0.0, config.y_max, config.num_y_labels);

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
                .bounds([config.x_min, config.x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("Req/s", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, config.y_max])
                .labels(y_labels),
        )
}

/// Create a latency chart with the given parameters
fn create_latency_chart<'a>(config: ChartConfig<'a>) -> Chart<'a> {
    let latency_dataset = vec![Dataset::default()
        .name("Latency (ms)")
        .marker(config.marker)
        .style(Style::default().fg(Color::Yellow))
        .data(config.data)];

    // Create axis labels
    let x_labels = create_time_axis_labels(config.x_min, config.x_max, config.num_x_labels);
    let y_labels = create_latency_axis_labels(0.0, config.y_max, config.num_y_labels);

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
                .bounds([config.x_min, config.x_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, config.y_max])
                .labels(y_labels),
        )
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

    // Mini charts
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Throughput chart
            Constraint::Percentage(50), // Latency chart
        ])
        .split(chunks[1]);

    // Throughput mini chart
    let throughput_data: Vec<(f64, f64)> = app_state.throughput_data.clone().into();
    let max_throughput = throughput_data
        .iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    // Create axis labels for mini chart (fewer labels for smaller space)
    let mini_x_min = throughput_data.first().map(|&(x, _)| x).unwrap_or(0.0);
    let mini_x_max = throughput_data.last().map(|&(x, _)| x).unwrap_or(60.0);
    let mini_y_max = max_throughput * 1.1;

    // Create throughput chart with Braille markers and fewer labels
    let throughput_chart = create_throughput_chart(ChartConfig {
        data: &throughput_data,
        title: "Throughput over time",
        marker: symbols::Marker::Braille,
        x_min: mini_x_min,
        x_max: mini_x_max,
        y_max: mini_y_max,
        num_x_labels: 3, // Fewer x-axis labels for mini chart
        num_y_labels: 3, // Fewer y-axis labels for mini chart
    });

    f.render_widget(throughput_chart, chart_chunks[0]);

    // Latency mini chart
    let latency_data: Vec<(f64, f64)> = app_state.latency_data.clone().into();
    let max_latency = latency_data
        .iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    // Create axis labels for mini latency chart
    let mini_lat_x_min = latency_data.first().map(|&(x, _)| x).unwrap_or(0.0);
    let mini_lat_x_max = latency_data.last().map(|&(x, _)| x).unwrap_or(60.0);
    let mini_lat_y_max = max_latency * 1.1;

    // Create latency chart with Braille markers and fewer labels
    let latency_chart = create_latency_chart(ChartConfig {
        data: &latency_data,
        title: "Latency over time",
        marker: symbols::Marker::Braille,
        x_min: mini_lat_x_min,
        x_max: mini_lat_x_max,
        y_max: mini_lat_y_max,
        num_x_labels: 3, // Fewer x-axis labels for mini chart
        num_y_labels: 3, // Fewer y-axis labels for mini chart
    });

    f.render_widget(latency_chart, chart_chunks[1]);
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

    // Create axis labels with more detail for the full-size chart
    let x_min = throughput_data.first().map(|&(x, _)| x).unwrap_or(0.0);
    let x_max = throughput_data.last().map(|&(x, _)| x).unwrap_or(60.0);
    let y_max = max_throughput * 1.1;

    // Create throughput chart with Braille markers and more labels
    let throughput_chart = create_throughput_chart(ChartConfig {
        data: &throughput_data,
        title: "Throughput over time",
        marker: symbols::Marker::Braille,
        x_min,
        x_max,
        y_max,
        num_x_labels: 6, // More x-axis labels for full chart
        num_y_labels: 6, // More y-axis labels for full chart
    });

    f.render_widget(throughput_chart, chunks[0]);

    // Latency chart (full size)
    let latency_data: Vec<(f64, f64)> = app_state.latency_data.clone().into();
    let max_latency = latency_data
        .iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    // Create axis labels with more detail for the full-size chart
    let l_x_min = latency_data.first().map(|&(x, _)| x).unwrap_or(0.0);
    let l_x_max = latency_data.last().map(|&(x, _)| x).unwrap_or(60.0);
    let l_y_max = max_latency * 1.1;

    // Create latency chart with Braille markers and more labels
    let latency_chart = create_latency_chart(ChartConfig {
        data: &latency_data,
        title: "Latency over time",
        marker: symbols::Marker::Braille,
        x_min: l_x_min,
        x_max: l_x_max,
        y_max: l_y_max,
        num_x_labels: 6, // More x-axis labels for full chart
        num_y_labels: 6, // More y-axis labels for full chart
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
