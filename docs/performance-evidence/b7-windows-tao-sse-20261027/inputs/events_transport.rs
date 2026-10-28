use super::*;
use std::{future::Future, io, time::Instant};

impl PermissionEvents {
    pub(super) async fn wait_for_io<T>(
        &self,
        generation: u64,
        phase: &str,
        timeout: Duration,
        future: impl Future<Output = Result<T, reqwest::Error>>,
    ) -> Result<T, String> {
        let deadline = Instant::now() + timeout;
        let mut pending = Box::pin(future);
        loop {
            if self.stopped.load(Ordering::Relaxed) {
                return Err("permission_stream_stopped".into());
            }
            if (self.current_generation)()? != generation {
                return Err("permission_generation_changed".into());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("permission_stream_{phase}_timeout"));
            }
            // Keep the same I/O future alive while checking lifecycle cancellation.
            if let Ok(result) =
                tokio::time::timeout(remaining.min(Duration::from_millis(100)), &mut pending).await
            {
                return result.map_err(|error| {
                    format!("permission_stream_{phase}: {}", error.without_url())
                });
            }
        }
    }
}

pub(super) struct ResponseReader {
    events: PermissionEvents,
    response: reqwest::Response,
    generation: u64,
    chunk: Vec<u8>,
    offset: usize,
}

impl ResponseReader {
    pub(super) fn new(
        events: PermissionEvents,
        response: reqwest::Response,
        generation: u64,
    ) -> Self {
        Self {
            events,
            response,
            generation,
            chunk: Vec::new(),
            offset: 0,
        }
    }
}

impl Read for ResponseReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        while self.offset == self.chunk.len() {
            let chunk = tauri::async_runtime::block_on(self.events.wait_for_io(
                self.generation,
                "body",
                Duration::from_secs(45),
                self.response.chunk(),
            ))
            .map_err(io::Error::other)?;
            let Some(chunk) = chunk else {
                return Ok(0);
            };
            self.chunk.clear();
            self.chunk.extend_from_slice(&chunk);
            self.offset = 0;
        }
        let count = output.len().min(self.chunk.len() - self.offset);
        output[..count].copy_from_slice(&self.chunk[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}
