// SPDX-License-Identifier: MIT
//! whambam: HTTP(S) load testing with a live terminal UI.

pub mod cli;
pub mod config;
pub mod engine;
pub mod http;
pub mod report;
pub mod stats;
pub mod ui;

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use tokio_util::sync::CancellationToken;

use cli::{Cli, OutputFormat};
use config::TestConfig;
use report::{CsvSink, write_report};
use stats::SharedState;

/// Run whambam with parsed command-line arguments.
pub async fn run(cli: Cli) -> Result<()> {
    let config = Arc::new(TestConfig::from_cli(&cli)?);
    let out = &cli.output;
    if out.debug {
        let mut buf = Vec::new();
        http::run_debug(&config, &mut buf).await?;
        return Ok(io::stdout().write_all(&buf)?);
    }

    let interactive = !out.no_ui && io::stdout().is_terminal();
    if interactive && out.output_format == OutputFormat::Csv && out.output_file.is_none() {
        bail!("CSV output needs --output FILE while the UI is running (or add --no-ui)");
    }
    let sink = match out.output_format {
        OutputFormat::Csv => Some(CsvSink::create(out.output_file.as_deref())?),
        _ => None,
    };

    let state = SharedState::default();
    let sink = if interactive {
        let options = ui::UiOptions {
            color: !out.no_color,
            fps: out.fps,
        };
        ui::run(Arc::clone(&config), state.clone(), sink, options)?
    } else {
        run_headless(Arc::clone(&config), state.clone(), sink).await?
    };
    if let Some(sink) = sink {
        sink.finish()?;
    }
    write_report(
        out.output_format,
        &state.snapshot(),
        &config,
        out.output_file.as_deref(),
    )
}

/// Run without the UI, showing progress on stderr. Ctrl-C stops the run and
/// still prints the report.
async fn run_headless(
    config: Arc<TestConfig>,
    state: SharedState,
    sink: Option<CsvSink>,
) -> Result<Option<CsvSink>> {
    let stop = CancellationToken::new();
    let ctrl_c = stop.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrl_c.cancel();
        }
    });
    let progress = io::stderr()
        .is_terminal()
        .then(|| spawn_progress(state.clone()));
    let result = engine::run(config, state, stop, sink).await;
    if let Some(p) = progress {
        p.abort();
        eprint!("\r\x1b[2K");
    }
    result
}

fn spawn_progress(state: SharedState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let (done, rps) = {
                let s = state.lock();
                (s.completed, s.rps())
            };
            eprint!("\rRequests completed: {done} ({rps:.0} req/s)   ");
            let _ = io::stderr().flush();
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
}
