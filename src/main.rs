use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{stream, StreamExt};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Span, Text},
    widgets::{
        Axis, Block, Borders, Chart, Dataset, Paragraph, Row, Table, Tabs,
    },
    Frame, Terminal,
};
use reqwest::Client;
use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use url::Url;
use floating_duration::TimeAsFloat;
use hdrhistogram::Histogram;

#[derive(Parser, Clone)]
#[command(author, version, about = "Test the throughput of an HTTP(S) endpoint")]
struct Args {
    /// The URL to test
    #[arg(required = true)]
    url: String,

    /// Number of requests to send (0 for unlimited)
    #[arg(short, long, default_value = "1000")]
    requests: usize,

    /// Number of concurrent connections
    #[arg(short, long, default_value = "10")]
    concurrent: usize,
    
    /// Duration of the test in seconds (0 for unlimited)
    #[arg(short, long, default_value = "0")]
    duration: u64,
}

#[derive(Debug, Clone)]
struct RequestMetric {
    timestamp: f64,
    latency_ms: f64,
    status_code: u16,
    is_error: bool,
}

#[derive(Debug)]
struct AppState {
    url: String,
    target_requests: usize,
    concurrent_requests: usize,
    duration: u64,
    start_time: Instant,
    
    completed_requests: usize,
    error_count: usize,
    
    // Status code counts
    status_counts: HashMap<u16, usize>,
    
    // Recent metrics
    recent_latencies: VecDeque<f64>,
    recent_throughput: VecDeque<(f64, f64)>, // (timestamp, requests/sec)
    
    // Histograms
    latency_histogram: Histogram<u64>,
    
    // Chart data
    throughput_data: VecDeque<(f64, f64)>, // Rolling throughput over time
    latency_data: VecDeque<(f64, f64)>,    // Rolling latency over time
    
    // Running statistics
    min_latency: f64,
    max_latency: f64,
    p50_latency: f64,
    p90_latency: f64,
    p99_latency: f64,
    
    // Current throughput
    current_throughput: f64,
    
    // UI state
    show_help: bool,
    selected_tab: usize,
    
    // Test completion
    is_complete: bool,
    should_quit: bool,
}

impl AppState {
    fn new(args: &Args) -> Self {
        let now = Instant::now();
        AppState {
            url: args.url.clone(),
            target_requests: args.requests,
            concurrent_requests: args.concurrent,
            duration: args.duration,
            start_time: now,
            
            completed_requests: 0,
            error_count: 0,
            
            status_counts: HashMap::new(),
            
            recent_latencies: VecDeque::with_capacity(100),
            recent_throughput: VecDeque::with_capacity(30),
            
            latency_histogram: Histogram::<u64>::new(3).unwrap(),
            
            throughput_data: VecDeque::with_capacity(60),
            latency_data: VecDeque::with_capacity(60),
            
            min_latency: f64::MAX,
            max_latency: 0.0,
            p50_latency: 0.0,
            p90_latency: 0.0,
            p99_latency: 0.0,
            
            current_throughput: 0.0,
            
            show_help: false,
            selected_tab: 0,
            
            is_complete: false,
            should_quit: false,
        }
    }
    
    fn update(&mut self, metric: RequestMetric) {
        // Update counters
        self.completed_requests += 1;
        
        if metric.is_error {
            self.error_count += 1;
        } else {
            *self.status_counts.entry(metric.status_code).or_insert(0) += 1;
        }
        
        // Update latency stats
        let latency = metric.latency_ms;
        self.recent_latencies.push_back(latency);
        if self.recent_latencies.len() > 100 {
            self.recent_latencies.pop_front();
        }
        
        // Convert from f64 to u64 (milliseconds * 10 for sub-millisecond precision)
        self.latency_histogram.record((latency * 10.0) as u64).unwrap();
        
        // Update min/max
        if latency < self.min_latency {
            self.min_latency = latency;
        }
        if latency > self.max_latency {
            self.max_latency = latency;
        }
        
        // Update percentiles
        if self.completed_requests % 10 == 0 {
            self.p50_latency = self.latency_histogram.value_at_quantile(0.5) as f64 / 10.0;
            self.p90_latency = self.latency_histogram.value_at_quantile(0.9) as f64 / 10.0;
            self.p99_latency = self.latency_histogram.value_at_quantile(0.99) as f64 / 10.0;
        }
        
        // Update throughput calculations once per second
        let elapsed = self.start_time.elapsed().as_fractional_secs();
        let last_throughput_time = self.throughput_data.back().map(|&(t, _)| t).unwrap_or(0.0);
        
        if elapsed - last_throughput_time >= 1.0 || self.throughput_data.is_empty() {
            // Calculate current throughput (requests per second)
            if !self.recent_throughput.is_empty() {
                let window_size = self.recent_throughput.len().min(10) as f64;
                let sum: f64 = self.recent_throughput.iter().map(|&(_, tps)| tps).sum();
                self.current_throughput = sum / window_size;
            }
            
            // Add data points for charts
            self.throughput_data.push_back((elapsed, self.current_throughput));
            if self.throughput_data.len() > 60 {
                self.throughput_data.pop_front();
            }
            
            let avg_latency: f64 = if !self.recent_latencies.is_empty() {
                self.recent_latencies.iter().sum::<f64>() / self.recent_latencies.len() as f64
            } else {
                0.0
            };
            
            self.latency_data.push_back((elapsed, avg_latency));
            if self.latency_data.len() > 60 {
                self.latency_data.pop_front();
            }
        }
        
        // Add throughput data point
        let second_bucket = elapsed.floor();
        let last_entry = self.recent_throughput.back().cloned();
        
        match last_entry {
            Some((bucket, count)) if bucket == second_bucket => {
                // Update existing bucket
                self.recent_throughput.pop_back();
                self.recent_throughput.push_back((bucket, count + 1.0));
            }
            _ => {
                // Create new bucket
                self.recent_throughput.push_back((second_bucket, 1.0));
                if self.recent_throughput.len() > 30 {
                    self.recent_throughput.pop_front();
                }
            }
        }
        
        // Check if test is complete
        if (self.target_requests > 0 && self.completed_requests >= self.target_requests) || 
           (self.duration > 0 && elapsed >= self.duration as f64) {
            self.is_complete = true;
        }
    }
}

enum Message {
    RequestComplete(RequestMetric),
    TestComplete,
}

struct App {
    state: Arc<Mutex<AppState>>,
}

impl App {
    fn new(args: Args) -> Self {
        App {
            state: Arc::new(Mutex::new(AppState::new(&args))),
        }
    }
    
    fn run_ui(&mut self) -> Result<()> {
        // Set up terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        
        // Start the event loop
        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();
        
        loop {
            let mut app_state = self.state.lock().unwrap();
            
            terminal.draw(|f| ui(f, &app_state))?;
            
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app_state.should_quit = true;
                            break;
                        }
                        KeyCode::Char('h') => {
                            app_state.show_help = !app_state.show_help;
                        }
                        KeyCode::Char('1') => {
                            app_state.selected_tab = 0;
                        }
                        KeyCode::Char('2') => {
                            app_state.selected_tab = 1;
                        }
                        KeyCode::Char('3') => {
                            app_state.selected_tab = 2;
                        }
                        _ => {}
                    }
                }
            }
            
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
            
            if app_state.is_complete || app_state.should_quit {
                break;
            }
            
            drop(app_state);
        }
        
        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        
        // Show final report
        let app_state = self.state.lock().unwrap();
        if app_state.is_complete && !app_state.should_quit {
            print_final_report(&app_state);
        }
        
        Ok(())
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app_state: &AppState) {
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
    
    // Title and status
    let elapsed = app_state.start_time.elapsed().as_secs_f64();
    let title = format!(
        "BLAMO Web Throughput Test - {} - Running for {:.1}s",
        app_state.url, elapsed
    );
    
    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());
    
    let title_text = Paragraph::new(title)
        .style(Style::default().fg(Color::Green))
        .block(title_block);
    
    f.render_widget(title_text, chunks[0]);
    
    // Tabs
    let tab_titles = vec!["Dashboard", "Charts", "Status Codes"];
    let tabs = Tabs::new(tab_titles)
    .block(Block::default().borders(Borders::ALL))
    .select(app_state.selected_tab)
    .style(Style::default().fg(Color::White))
    .highlight_style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    );
    
    f.render_widget(tabs, chunks[1]);
    
    // Main content based on selected tab
    match app_state.selected_tab {
        0 => render_dashboard(f, app_state, chunks[2]),
        1 => render_charts(f, app_state, chunks[2]),
        2 => render_status_codes(f, app_state, chunks[2]),
        _ => {}
    }
    
    // Help overlay if enabled
    if app_state.show_help {
        render_help(f, f.size());
    }
}

fn render_dashboard<B: Backend>(f: &mut Frame<B>, app_state: &AppState, area: Rect) {
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
    
    let elapsed = app_state.start_time.elapsed().as_secs_f64();
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

fn render_charts<B: Backend>(f: &mut Frame<B>, app_state: &AppState, area: Rect) {
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

fn render_status_codes<B: Backend>(f: &mut Frame<B>, app_state: &AppState, area: Rect) {
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

fn render_help<B: Backend>(f: &mut Frame<B>, area: Rect) {
    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    
    let help_text = Text::from("Press 'q' to quit\nPress 'h' to toggle help\nPress '1' to view Dashboard\nPress '2' to view Charts\nPress '3' to view Status Codes");
    
    let help_area = centered_rect(60, 40, area);
    
    let help_paragraph = Paragraph::new(help_text.clone())
        .block(block)
        .style(Style::default().fg(Color::White))
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(help_paragraph, help_area);
}

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

fn print_final_report(app_state: &AppState) {
    let elapsed = app_state.start_time.elapsed().as_secs_f64();
    let overall_tps = if elapsed > 0.0 {
        app_state.completed_requests as f64 / elapsed
    } else {
        0.0
    };
    
    println!("\n===== Blamo Web Throughput Test Results =====");
    println!("URL: {}", app_state.url);
    println!("Total Requests: {}", app_state.completed_requests);
    println!("Total Time: {:.2}s", elapsed);
    println!("Average Throughput: {:.2} req/s", overall_tps);
    println!("Error Count: {} ({:.2}%)", 
             app_state.error_count, 
             100.0 * app_state.error_count as f64 / app_state.completed_requests.max(1) as f64);
    
    println!("\nLatency Statistics:");
    println!("  Min: {:.2} ms", if app_state.min_latency == f64::MAX { 0.0 } else { app_state.min_latency });
    println!("  Max: {:.2} ms", app_state.max_latency);
    println!("  P50: {:.2} ms", app_state.p50_latency);
    println!("  P90: {:.2} ms", app_state.p90_latency);
    println!("  P99: {:.2} ms", app_state.p99_latency);
    
    println!("\nStatus Code Distribution:");
    let mut status_codes: Vec<u16> = app_state.status_counts.keys().cloned().collect();
    status_codes.sort();
    
    for status in status_codes {
        let count = *app_state.status_counts.get(&status).unwrap_or(&0);
        let percentage = 100.0 * count as f64 / app_state.completed_requests.max(1) as f64;
        println!("  HTTP {}: {} ({:.2}%)", status, count, percentage);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let url = Url::parse(&args.url).context("Invalid URL")?;
    
    println!("Starting throughput test for: {}", url);
    println!("Requests: {}, Concurrent: {}", 
             if args.requests > 0 { args.requests.to_string() } else { "Unlimited".to_string() },
             args.concurrent);
    println!("Duration: {} seconds", 
             if args.duration > 0 { args.duration.to_string() } else { "Unlimited".to_string() });
    println!("Press Ctrl+C to stop the test\n");
    
    // Set up app and channels
    let mut app = App::new(args.clone());
    let state = Arc::clone(&app.state);
    
    let (tx, mut rx) = mpsc::channel::<Message>(100);
    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = Arc::clone(&is_running);
    
    // Spawn load test thread
    let load_test_handle = tokio::spawn(async move {
        let client = Client::new();
        let url = url.clone();
        let requests_count = Arc::new(AtomicUsize::new(0));
        
        let start_time = Instant::now();
        let max_requests = if args.requests > 0 { args.requests } else { usize::MAX };
        let max_duration = if args.duration > 0 {
            Some(Duration::from_secs(args.duration))
        } else {
            None
        };
        
        let stream = stream::iter(0..max_requests)
            .map(|_| {
                let client = &client;
                let url = url.clone();
                let tx = tx.clone();
                let requests_count = Arc::clone(&requests_count);
                let is_running = Arc::clone(&is_running_clone);
                
                async move {
                    // Check if we should stop due to duration
                    if let Some(max_dur) = max_duration {
                        if start_time.elapsed() >= max_dur {
                            is_running.store(false, Ordering::SeqCst);
                            return;
                        }
                    }
                    
                    // Check if we should stop due to user cancellation
                    if !is_running.load(Ordering::SeqCst) {
                        return;
                    }
                    
                    // Make the request
                    let request_start = Instant::now();
                    let result = client.get(url.clone()).send().await;
                    let duration = request_start.elapsed();
                    
                    // Create metric
                    let metric = match result {
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            RequestMetric {
                                timestamp: start_time.elapsed().as_fractional_secs(),
                                latency_ms: duration.as_fractional_millis(),
                                status_code: status,
                                is_error: false,
                            }
                        }
                        Err(_) => RequestMetric {
                            timestamp: start_time.elapsed().as_fractional_secs(),
                            latency_ms: duration.as_fractional_millis(),
                            status_code: 0,
                            is_error: true,
                        },
                    };
                    
                    // Send metric update
                    let _ = tx.send(Message::RequestComplete(metric)).await;
                    requests_count.fetch_add(1, Ordering::SeqCst);
                }
            })
            .buffer_unordered(args.concurrent);
        
        stream.for_each(|_| async {}).await;
        
        // Signal that we're done
        let _ = tx.send(Message::TestComplete).await;
    });
    
    // Spawn metrics processing thread
    let metrics_handle = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            match message {
                Message::RequestComplete(metric) => {
                    let mut app_state = state.lock().unwrap();
                    app_state.update(metric);
                }
                Message::TestComplete => {
                    break;
                }
            }
        }
    });
    
    // Run the UI
    app.run_ui()?;
    
    // Clean up
    is_running.store(false, Ordering::SeqCst);
    
    // Wait for threads to finish
    let _ = tokio::join!(load_test_handle, metrics_handle);
    
    Ok(())
}