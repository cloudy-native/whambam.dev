use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Span, Text},
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tester::TestState;
use super::app::UiState;

/// Main UI render function
pub fn ui<B: Backend>(f: &mut Frame<B>, app_state: &TestState, ui_state: &UiState) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Title and status
            Constraint::Length(3),  // Tabs
            Constraint::Min(0),     // Content
        ])
        .split(f.size());

    // Title and status with correct elapsed time
    let elapsed = if app_state.is_complete && app_state.end_time.is_some() {
        // For completed tests, use the frozen end time
        app_state.end_time.unwrap().duration_since(app_state.start_time).as_secs_f64()
    } else {
        // For running tests, use current elapsed time
        app_state.start_time.elapsed().as_secs_f64()
    };
    let status = if app_state.is_complete { "COMPLETED" } else { "RUNNING" };
    let title = format!(
        "BLAMO Web Throughput Test - {} - {} for {:.1}s",
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

    let full_title = format!("{}{}", title, key_help);
    let color = if app_state.is_complete { Color::Blue } else { Color::Green };
    let title_text = Paragraph::new(full_title.as_str())
        .style(Style::default().fg(color))
        .block(title_block);

    f.render_widget(title_text, chunks[0]);

    // Tabs
    let tab_titles = vec!["Dashboard (1)", "Charts (2)", "Status Codes (3)"];
    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL))
        .select(ui_state.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
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
            Constraint::Percentage(40),  // Stats
            Constraint::Percentage(60),  // Mini charts
        ])
        .split(area);

    // Stats section
    let stat_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),  // Throughput stats
            Constraint::Percentage(50),  // Latency stats
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
        app_state.end_time.unwrap().duration_since(app_state.start_time).as_secs_f64()
    } else {
        app_state.start_time.elapsed().as_secs_f64()
    };
    
    let overall_tps = if elapsed > 0.0 {
        completed as f64 / elapsed
    } else {
        0.0
    };

    let throughput_stats = vec![
        format!("Completed Requests: {}", completed),
        format!("Error Count: {}", errors),
        format!("Success Rate: {:.1}%", success_rate),
        format!("Current Throughput: {:.1} req/s", app_state.current_throughput),
        format!("Overall Throughput: {:.1} req/s", overall_tps),
        format!("Elapsed Time: {:.1}s", elapsed),
    ];

    let throughput_block = Block::default()
        .title("Throughput")
        .borders(Borders::ALL);

    let throughput_stats_str = throughput_stats.join("\n");
    let throughput_text = Paragraph::new(throughput_stats_str.as_str())
        .style(Style::default().fg(Color::White))
        .block(throughput_block);

    f.render_widget(throughput_text, stat_chunks[0]);

    // Latency stats
    let min = if app_state.min_latency == f64::MAX { 0.0 } else { app_state.min_latency };

    let latency_stats = vec![
        format!("Min Latency: {:.1} ms", min),
        format!("Max Latency: {:.1} ms", app_state.max_latency),
        format!("P50 Latency: {:.1} ms", app_state.p50_latency),
        format!("P90 Latency: {:.1} ms", app_state.p90_latency),
        format!("P99 Latency: {:.1} ms", app_state.p99_latency),
    ];

    let latency_block = Block::default()
        .title("Latency")
        .borders(Borders::ALL);

    let latency_stats_str = latency_stats.join("\n");
    let latency_text = Paragraph::new(latency_stats_str.as_str())
        .style(Style::default().fg(Color::White))
        .block(latency_block);

    f.render_widget(latency_text, stat_chunks[1]);

    // Mini charts
    let chart_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),  // Throughput chart
            Constraint::Percentage(50),  // Latency chart
        ])
        .split(chunks[1]);

    // Throughput mini chart
    let throughput_data: Vec<(f64, f64)> = app_state.throughput_data.clone().into();

    let throughput_dataset = vec![Dataset::default()
        .name("Throughput (req/s)")
        .marker(symbols::Marker::Dot)
        .style(Style::default().fg(Color::Cyan))
        .data(&throughput_data)];

    let max_throughput = throughput_data.iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    let throughput_chart = Chart::new(throughput_dataset)
        .block(
            Block::default()
                .title("Throughput over time")
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([
                    throughput_data.first().map(|&(x, _)| x).unwrap_or(0.0),
                    throughput_data.last().map(|&(x, _)| x).unwrap_or(60.0),
                ]),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("Req/s", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, max_throughput * 1.1]),
        );

    f.render_widget(throughput_chart, chart_chunks[0]);

    // Latency mini chart
    let latency_data: Vec<(f64, f64)> = app_state.latency_data.clone().into();

    let latency_dataset = vec![Dataset::default()
        .name("Latency (ms)")
        .marker(symbols::Marker::Dot)
        .style(Style::default().fg(Color::Yellow))
        .data(&latency_data)];

    let max_latency = latency_data.iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    let latency_chart = Chart::new(latency_dataset)
        .block(
            Block::default()
                .title("Latency over time")
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([
                    latency_data.first().map(|&(x, _)| x).unwrap_or(0.0),
                    latency_data.last().map(|&(x, _)| x).unwrap_or(60.0),
                ]),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("ms", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, max_latency * 1.1]),
        );

    f.render_widget(latency_chart, chart_chunks[1]);
}

/// Render the charts tab
fn render_charts<B: Backend>(f: &mut Frame<B>, app_state: &TestState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),  // Throughput chart
            Constraint::Percentage(50),  // Latency chart
        ])
        .split(area);

    // Throughput chart (full size)
    let throughput_data: Vec<(f64, f64)> = app_state.throughput_data.clone().into();

    let throughput_dataset = vec![Dataset::default()
        .name("Throughput (req/s)")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .data(&throughput_data)];

    let max_throughput = throughput_data.iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    let throughput_chart = Chart::new(throughput_dataset)
        .block(
            Block::default()
                .title("Throughput over time")
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([
                    throughput_data.first().map(|&(x, _)| x).unwrap_or(0.0),
                    throughput_data.last().map(|&(x, _)| x).unwrap_or(60.0),
                ]),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("Req/s", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, max_throughput * 1.1]),
        );

    f.render_widget(throughput_chart, chunks[0]);

    // Latency chart (full size)
    let latency_data: Vec<(f64, f64)> = app_state.latency_data.clone().into();

    let latency_dataset = vec![Dataset::default()
        .name("Latency (ms)")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Yellow))
        .data(&latency_data)];

    let max_latency = latency_data.iter()
        .map(|&(_, y)| y)
        .fold(1.0f64, |max, y| max.max(y));

    let latency_chart = Chart::new(latency_dataset)
        .block(
            Block::default()
                .title("Latency over time")
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .title(Span::styled("Time (s)", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([
                    latency_data.first().map(|&(x, _)| x).unwrap_or(0.0),
                    latency_data.last().map(|&(x, _)| x).unwrap_or(60.0),
                ]),
        )
        .y_axis(
            Axis::default()
                .title(Span::styled("ms", Style::default().fg(Color::Gray)))
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, max_latency * 1.1]),
        );

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

        let status_text = Span::styled(
            format!("{}", status),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        );

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
        .map(|h| {
            (*h).to_string()
        });

    let header = Row::new(header_cells)
        .style(Style::default())
        .height(1);

    let table = Table::new(status_rows)
        .header(header)
        .block(
            Block::default()
                .title("HTTP Status Codes")
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
        .title(" Help ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    
    // Help text content
    let help_text = vec![
        "Press 'q' to quit",
        "Press 'r' to restart completed test",
        "Press 'h' to toggle this help overlay",
        "Press '1' to view Dashboard", 
        "Press '2' to view Charts",
        "Press '3' to view Status Codes"
    ].join("\n");
    
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