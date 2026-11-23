//! Idle timeout shared by both directions of a forwarded connection.
use std::{
    io,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    time::{sleep_until, Instant},
};

/// Idle timeout that is reset whenever data is transferred in either direction.
pub(super) struct IdleTimeout {
    timeout: Duration,
    start: Instant,
    /// Time of the last activity in milliseconds since `start`.
    last_activity: AtomicU64,
}

impl IdleTimeout {
    /// Create a new idle timeout.
    pub(super) fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            start: Instant::now(),
            last_activity: AtomicU64::new(0),
        }
    }

    /// Wrap a stream so that any data transferred over it resets the idle timeout.
    pub(super) fn track<S>(&self, stream: S) -> Tracked<'_, S> {
        Tracked { stream, idle: self }
    }

    /// Wait until no data has been transferred for the duration of the timeout.
    pub(super) async fn expired(&self) {
        loop {
            let deadline = self.deadline();
            if Instant::now() >= deadline {
                return;
            }
            sleep_until(deadline).await;
        }
    }

    fn deadline(&self) -> Instant {
        let last_activity = Duration::from_millis(self.last_activity.load(Ordering::Relaxed));
        self.start + last_activity + self.timeout
    }

    fn record_activity(&self) {
        let elapsed = self
            .start
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        self.last_activity.fetch_max(elapsed, Ordering::Relaxed);
    }
}

/// A stream that records activity on its idle timeout.
pub(super) struct Tracked<'a, S> {
    stream: S,
    idle: &'a IdleTimeout,
}

impl<S: AsyncRead + Unpin> AsyncRead for Tracked<'_, S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let filled = buf.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(cx, buf);
        if buf.filled().len() > filled {
            self.idle.record_activity();
        }
        result
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Tracked<'_, S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.stream).poll_write(cx, data);
        if matches!(result, Poll::Ready(Ok(count)) if count > 0) {
            self.idle.record_activity();
        }
        result
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.stream).poll_write_vectored(cx, data);
        if matches!(result, Poll::Ready(Ok(count)) if count > 0) {
            self.idle.record_activity();
        }
        result
    }

    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
