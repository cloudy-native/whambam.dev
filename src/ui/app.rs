use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::widgets::ui;
use crate::tester::{print_final_report, SharedState, TestConfig};

/// The UI application
pub struct App {
    shared_state: SharedState,
    ui_state: UiState,
}

/// UI-specific state
pub struct UiState {
    pub show_help: bool,
    pub selected_tab: usize,
}

impl UiState {
    pub fn new() -> Self {
        UiState {
            show_help: false,
            selected_tab: 0,
        }
    }
}

impl App {
    /// Create a new UI application
    pub fn new(shared_state: SharedState) -> Self {
        App {
            shared_state,
            ui_state: UiState::new(),
        }
    }

    /// Run the UI
    pub fn run(&mut self) -> Result<()> {
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
            // Minimize the time we hold the lock - get a snapshot of the state
            let should_quit;

            {
                // CRITICAL: Lock for as little time as possible to avoid blocking the test runner
                let app_state = self.shared_state.state.lock().unwrap();

                // Just render with the current state snapshot
                terminal.draw(|f| ui(f, &app_state, &self.ui_state))?;

                // Store quit value for checking later
                should_quit = app_state.should_quit;
            }

            // Check for key events
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            // Mark as quit but only hold the lock briefly
                            {
                                let mut app_state = self.shared_state.state.lock().unwrap();
                                app_state.should_quit = true;
                            }
                            break;
                        }
                        KeyCode::Char('h') => {
                            self.ui_state.show_help = !self.ui_state.show_help;
                        }
                        KeyCode::Char('1') => {
                            self.ui_state.selected_tab = 0;
                        }
                        KeyCode::Char('2') => {
                            self.ui_state.selected_tab = 1;
                        }
                        KeyCode::Char('3') => {
                            self.ui_state.selected_tab = 2;
                        }
                        KeyCode::Char('r') => {
                            // Restart the test
                            let mut app_state = self.shared_state.state.lock().unwrap();
                            if app_state.is_complete {
                                // Reset test state for a new run
                                app_state.reset();

                                // Create and launch a new test runner
                                let config = TestConfig {
                                    url: app_state.url.clone(),
                                    method: app_state.method.clone(),
                                    requests: app_state.target_requests,
                                    concurrent: app_state.concurrent_requests,
                                    duration: app_state.duration,
                                    rate_limit: 0.0, // Default no rate limit
                                    headers: app_state.headers.clone(),
                                    timeout: 20, // Default timeout
                                    body: None, // No body
                                    content_type: "text/html".to_string(),
                                    basic_auth: None, // No auth
                                    proxy: None, // No proxy
                                    disable_compression: false,
                                    disable_keepalive: false,
                                    disable_redirects: false,
                                };

                                let state_clone = Arc::clone(&self.shared_state.state);

                                // Spawn a new test runner task
                                tokio::spawn(async move {
                                    // Create a test runner inside the task with the shared state
                                    let mut runner = crate::tester::TestRunner::with_state(
                                        config,
                                        crate::tester::SharedState { state: state_clone },
                                    );
                                    let _ = runner.start().await;
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }

            // Only exit on explicit quit, not on completion
            if should_quit {
                break;
            }

            // Small sleep to prevent UI from consuming 100% CPU
            std::thread::sleep(Duration::from_millis(10));
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
        let app_state = self.shared_state.state.lock().unwrap();
        if app_state.is_complete && !app_state.should_quit {
            print_final_report(&app_state);
        }

        Ok(())
    }
}
