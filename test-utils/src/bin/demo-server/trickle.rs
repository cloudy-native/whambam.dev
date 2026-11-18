// SPDX-License-Identifier: MIT
//! A response body that arrives slowly: one chunk, then a pause, until it is all sent.

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use hyper::body::{Body, Frame};
use tokio::time::Sleep;

pub struct Trickle {
    chunk: Bytes,
    left: usize,
    gap: Duration,
    sleep: Pin<Box<Sleep>>,
}

impl Trickle {
    /// `chunks` copies of `chunk`, the first one after `gap` (so headers clearly arrive first).
    pub fn new(chunk: Bytes, chunks: usize, gap: Duration) -> Trickle {
        Trickle {
            chunk,
            left: chunks,
            gap,
            sleep: Box::pin(tokio::time::sleep(gap)),
        }
    }
}

impl Body for Trickle {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        if self.left == 0 {
            return Poll::Ready(None);
        }
        if self.sleep.as_mut().poll(cx).is_pending() {
            return Poll::Pending;
        }
        self.left -= 1;
        let next = tokio::time::Instant::now() + self.gap;
        self.sleep.as_mut().reset(next);
        Poll::Ready(Some(Ok(Frame::data(self.chunk.clone()))))
    }

    fn is_end_stream(&self) -> bool {
        self.left == 0
    }
}
