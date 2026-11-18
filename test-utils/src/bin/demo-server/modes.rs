// SPDX-License-Identifier: MIT
//! The demo server's behaviours, one per realistic performance situation.

use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response};

use super::trickle::Trickle;

pub type Reply = Result<Response<BoxBody<Bytes, Infallible>>, Infallible>;

/// Stall period and length, in seconds.
const STALL_EVERY: f64 = 5.0;
const STALL_FOR: f64 = 1.0;
/// Rate limit: requests allowed per 100 ms window (1,500 per second).
const LIMIT_PER_WINDOW: u32 = 150;
/// Cold start: extra latency at start (ms) and how fast it decays (s).
const COLD_EXTRA_MS: f64 = 80.0;
const COLD_DECAY_S: f64 = 4.0;
/// Slow body: 16 chunks of 4 KiB, 20 ms apart.
const TRICKLE_CHUNKS: usize = 16;
const TRICKLE_GAP: Duration = Duration::from_millis(20);
/// The "new code" (--slower) adds this to every request of the steady server (ms).
const REGRESSION_MS: f64 = 12.0;

const BODY: &[u8] = b"{\"ok\":true,\"service\":\"demo\"}";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Wave,
    Fast,
    Stall,
    RateLimit,
    ColdStart,
    SlowBody,
    Steady,
    Slower,
}

impl Mode {
    /// Every mode with its flag, for --help.
    pub const ALL: [(&'static str, Mode); 8] = [
        ("wave", Mode::Wave),
        ("fast", Mode::Fast),
        ("stall", Mode::Stall),
        ("ratelimit", Mode::RateLimit),
        ("coldstart", Mode::ColdStart),
        ("slowbody", Mode::SlowBody),
        ("steady", Mode::Steady),
        ("slower", Mode::Slower),
    ];

    pub fn parse(flag: &str) -> Option<Mode> {
        Some(match flag {
            "fast" => Mode::Fast,
            "stall" => Mode::Stall,
            "ratelimit" => Mode::RateLimit,
            "coldstart" => Mode::ColdStart,
            "slowbody" => Mode::SlowBody,
            "steady" => Mode::Steady,
            "slower" => Mode::Slower,
            "wave" => Mode::Wave,
            _ => return None,
        })
    }

    pub fn flag(self) -> &'static str {
        Mode::ALL
            .iter()
            .find(|(_, m)| *m == self)
            .map_or("wave", |(f, _)| f)
    }

    pub fn describe(self) -> &'static str {
        match self {
            Mode::Wave => "3-20 ms on a slow wave, a 1% tail, a few 503s (the default)",
            Mode::Fast => "answers at once, no errors",
            Mode::Stall => "answers in ~2 ms, but freezes for 1 s every 5 s",
            Mode::RateLimit => "answers in a few ms; 429 above 1,500 req/s",
            Mode::ColdStart => "starts slow, settles over ~15 s",
            Mode::SlowBody => "headers at once, then a 64 KiB body over ~320 ms",
            Mode::Steady => "2-6 ms, no errors (the current code)",
            Mode::Slower => "steady + 12 ms (the new code)",
        }
    }
}

/// Shared per-server state: the mode, when the server started, and the rate limiter's window.
#[derive(Clone)]
pub struct State {
    mode: Mode,
    started: Instant,
    window: Arc<Mutex<(u64, u32)>>,
}

impl State {
    pub fn new(mode: Mode) -> State {
        State {
            mode,
            started: Instant::now(),
            window: Arc::new(Mutex::new((0, 0))),
        }
    }

    /// True if this request fits in the current 100 ms window.
    fn admit(&self) -> bool {
        let w = (self.started.elapsed().as_millis() / 100) as u64;
        let mut guard = self.window.lock().expect("window lock");
        if guard.0 != w {
            *guard = (w, 0);
        }
        guard.1 += 1;
        guard.1 <= LIMIT_PER_WINDOW
    }
}

static SEED: AtomicU64 = AtomicU64::new(0x2545_f491_4f6c_dd1d);

/// xorshift: good enough for jitter, no extra dependency.
fn random() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x % 10_000) as f64 / 10_000.0
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

async fn sleep_ms(ms: f64) {
    tokio::time::sleep(Duration::from_secs_f64(ms / 1e3)).await;
}

fn respond(status: u16) -> Reply {
    Ok(Response::builder()
        .status(status)
        .body(Full::new(Bytes::from_static(BODY)).boxed())
        .expect("response"))
}

/// The default: a slow wave every ~10 s plus jitter, an occasional long tail and a few 503s.
fn wave_ms() -> f64 {
    let wave = 1.0 + (now_secs() * 0.6).sin().max(0.0) * 6.0;
    let tail = if random() < 0.01 { 40.0 } else { 0.0 };
    2.0 + wave * (1.0 + random() * 2.0) + tail
}

async fn wave() -> Reply {
    sleep_ms(wave_ms()).await;
    respond(if random() < 0.004 { 503 } else { 200 })
}

async fn stall() -> Reply {
    // Requests that arrive during a stall all wait until it ends.
    let phase = now_secs() % STALL_EVERY;
    if phase < STALL_FOR {
        sleep_ms((STALL_FOR - phase) * 1e3).await;
    }
    sleep_ms(2.0).await;
    respond(200)
}

async fn rate_limited(state: &State) -> Reply {
    if !state.admit() {
        return respond(429);
    }
    sleep_ms(2.0 + random() * 3.0).await;
    respond(200)
}

async fn cold_start(state: &State) -> Reply {
    let warm = (-state.started.elapsed().as_secs_f64() / COLD_DECAY_S).exp();
    sleep_ms(3.0 + random() * 2.0 + COLD_EXTRA_MS * warm * (0.6 + random() * 0.8)).await;
    respond(200)
}

async fn steady(extra_ms: f64) -> Reply {
    sleep_ms(2.0 + random() * 4.0 + extra_ms).await;
    respond(200)
}

fn slow_body() -> Reply {
    let body = Trickle::new(Bytes::from(vec![b'x'; 4096]), TRICKLE_CHUNKS, TRICKLE_GAP);
    Ok(Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .body(body.boxed())
        .expect("response"))
}

pub async fn handle(state: State, _req: Request<hyper::body::Incoming>) -> Reply {
    match state.mode {
        Mode::Fast => respond(200),
        Mode::Stall => stall().await,
        Mode::Wave => wave().await,
        Mode::Steady => steady(0.0).await,
        Mode::Slower => steady(REGRESSION_MS).await,
        Mode::RateLimit => rate_limited(&state).await,
        Mode::ColdStart => cold_start(&state).await,
        Mode::SlowBody => slow_body(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_mode_flag() {
        for (flag, mode) in [
            ("fast", Mode::Fast),
            ("stall", Mode::Stall),
            ("ratelimit", Mode::RateLimit),
            ("coldstart", Mode::ColdStart),
            ("slowbody", Mode::SlowBody),
            ("steady", Mode::Steady),
            ("slower", Mode::Slower),
        ] {
            assert_eq!(Mode::parse(flag), Some(mode));
        }
        assert_eq!(Mode::parse("nope"), None);
    }

    #[test]
    fn every_listed_mode_parses_back() {
        for (flag, mode) in Mode::ALL {
            assert_eq!(Mode::parse(flag), Some(mode));
        }
    }

    #[test]
    fn rate_limit_admits_one_window_then_refuses() {
        let s = State::new(Mode::RateLimit);
        let admitted = (0..LIMIT_PER_WINDOW + 50).filter(|_| s.admit()).count();
        // All calls land in the first 100 ms window on any sane machine.
        assert_eq!(admitted, LIMIT_PER_WINDOW as usize);
    }
}
