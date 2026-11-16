// SPDX-License-Identifier: MIT
//! The interactive UI loop: draw, handle keys, start and restart runs.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::UiOptions;
use super::theme::Theme;
use super::view::{self, Tab, ViewCtx};
use crate::config::TestConfig;
use crate::engine::{self, Control, Settings};
use crate::report::{Baseline, CsvSink};
use crate::stats::SharedState;

type RunTask = JoinHandle<Result<Option<CsvSink>>>;

/// How long to wait for in-flight requests after the user quits.
const QUIT_GRACE: Duration = Duration::from_secs(2);

/// Run the UI until the user quits. Returns the CSV sink, if any, so the
/// caller can flush it.
pub fn run(
    config: Arc<TestConfig>,
    state: SharedState,
    sink: Option<CsvSink>,
    options: UiOptions,
) -> Result<Option<CsvSink>> {
    let handle = Handle::current();
    let app = App {
        control: Control::new(Settings::from_config(&config)),
        config,
        state,
        stop: CancellationToken::new(),
        tab: Tab::Dashboard,
        help: false,
        theme: Theme::new(options.color),
        baseline: options.baseline.clone(),
        frame: Duration::from_secs_f64(1.0 / f64::from(options.fps)),
    };
    tokio::task::block_in_place(|| app.run(&handle, sink))
}

struct App {
    config: Arc<TestConfig>,
    control: Control,
    state: SharedState,
    stop: CancellationToken,
    tab: Tab,
    help: bool,
    theme: Theme,
    frame: Duration,
    baseline: Option<Arc<Baseline>>,
}

enum Action {
    None,
    Quit,
    Restart,
}

impl App {
    fn run(mut self, handle: &Handle, sink: Option<CsvSink>) -> Result<Option<CsvSink>> {
        let mut terminal = ratatui::try_init()?;
        let mut task = Some(self.spawn(handle, sink));
        let mut sink = None;
        let result = self.event_loop(&mut terminal, handle, &mut task, &mut sink);
        ratatui::restore();
        self.stop.cancel();
        if let Some(t) = task.take() {
            sink = finish_within(handle, t, QUIT_GRACE)?;
        }
        self.state.lock().mark_complete(true);
        result.map(|()| sink)
    }

    fn spawn(&self, handle: &Handle, sink: Option<CsvSink>) -> RunTask {
        let run = engine::run(
            Arc::clone(&self.config),
            self.state.clone(),
            self.stop.clone(),
            sink,
            self.control.clone(),
        );
        handle.spawn(run)
    }

    fn event_loop(
        &mut self,
        terminal: &mut DefaultTerminal,
        handle: &Handle,
        task: &mut Option<RunTask>,
        sink: &mut Option<CsvSink>,
    ) -> Result<()> {
        loop {
            if let Some(t) = task.take_if(|t| t.is_finished()) {
                *sink = handle.block_on(t)??;
            }
            terminal.draw(|frame| {
                let mut state = self.state.lock();
                state.tick();
                let ctx = ViewCtx {
                    config: &self.config,
                    settings: self.control.get(),
                    baseline: self.baseline.as_deref(),
                    theme: self.theme,
                    tab: self.tab,
                    help: self.help,
                };
                view::draw(frame, &state, &ctx);
            })?;
            if !event::poll(self.frame)? {
                continue;
            }
            if let Event::Key(key) = event::read()? {
                match self.on_key(key) {
                    Action::Quit => return Ok(()),
                    Action::Restart if task.is_none() => {
                        *task = Some(self.spawn(handle, sink.take()));
                    }
                    _ => {}
                }
            }
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> Action {
        if key.kind != KeyEventKind::Press {
            return Action::None;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
            KeyCode::Char('r') => Action::Restart,
            KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Up => {
                self.control.step_workers(1);
                Action::None
            }
            KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::Down => {
                self.control.step_workers(-1);
                Action::None
            }
            KeyCode::Char(']') => {
                self.control.step_rate(1);
                Action::None
            }
            KeyCode::Char('[') => {
                self.control.step_rate(-1);
                Action::None
            }
            KeyCode::Char('h') | KeyCode::Char('?') => {
                self.help = !self.help;
                Action::None
            }
            KeyCode::Tab | KeyCode::Right => {
                self.tab = self.tab.next();
                Action::None
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.tab = self.tab.prev();
                Action::None
            }
            KeyCode::Char(c) => {
                if let Some(tab) = Tab::from_key(c) {
                    self.tab = tab;
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}

/// Wait briefly for a run to wind down; abort it if it does not.
fn finish_within(handle: &Handle, task: RunTask, grace: Duration) -> Result<Option<CsvSink>> {
    let abort = task.abort_handle();
    match handle.block_on(tokio::time::timeout(grace, task)) {
        Ok(joined) => joined?,
        Err(_) => {
            abort.abort();
            Ok(None)
        }
    }
}
