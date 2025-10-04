use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, info, warn};

use crate::{websocket::WebSocketClient, StreamConfig};

pub struct StreamProcessor {
    config: StreamConfig,
    line_count: u64,
    buffer: Vec<String>,
    buffer_size: usize,
}

impl StreamProcessor {
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            line_count: 0,
            buffer: Vec::new(),
            buffer_size: 10, // Smaller batch size for better performance
        }
    }

    pub async fn process_stdin(&mut self, client: &mut WebSocketClient) -> Result<()> {
        info!("Starting stdin processing for stream: {}", self.config.name);

        // Create the stream
        let stream_id = client.create_stream(&self.config).await?;
        info!("Created stream: {}", stream_id);

        // Set up stdin reader
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        let mut last_batch_time = std::time::Instant::now();
        let mut adaptive_delay = 100_u64; // Start with 100ms delay
        const MIN_DELAY_MS: u64 = 50;
        const MAX_DELAY_MS: u64 = 2000;
        const MAX_LINES_PER_SECOND: u64 = 30; // More conservative limit

        // Process lines from stdin
        while let Some(line) = lines.next_line().await? {
            self.line_count += 1;
            debug!(
                "Processing line {}: {}",
                self.line_count,
                line.chars().take(50).collect::<String>()
            );

            // Add to buffer
            self.buffer.push(line.clone());

            // Send batch if buffer is full or enough time has passed
            let should_send_batch = self.buffer.len() >= self.buffer_size
                || last_batch_time.elapsed().as_millis() >= adaptive_delay as u128;

            if should_send_batch {
                match self.send_batch(client).await {
                    Ok(_) => {
                        // Success - reduce delay slightly
                        adaptive_delay = std::cmp::max(MIN_DELAY_MS, adaptive_delay - 10);
                        last_batch_time = std::time::Instant::now();
                    }
                    Err(e) => {
                        // Error - increase delay and retry logic
                        adaptive_delay = std::cmp::min(MAX_DELAY_MS, adaptive_delay * 2);
                        warn!(
                            "Send batch failed, increasing delay to {}ms: {}",
                            adaptive_delay, e
                        );

                        // Sleep before retrying
                        tokio::time::sleep(tokio::time::Duration::from_millis(adaptive_delay))
                            .await;
                        return Err(e);
                    }
                }

                // Rate limiting: sleep if we're processing too fast
                if self.line_count % MAX_LINES_PER_SECOND == 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(adaptive_delay)).await;
                }
            }

            // Progress logging for large streams
            if self.line_count % 100 == 0 {
                info!(
                    "Processed {} lines (delay: {}ms)",
                    self.line_count, adaptive_delay
                );
            }

            // Safety limit: prevent infinite streams from crashing server
            if self.line_count > 10000 {
                warn!("Reached maximum line limit (10,000). Stopping stream.");
                break;
            }
        }

        // Send any remaining buffered lines
        if !self.buffer.is_empty() {
            self.send_batch(client).await?;
        }

        // End the stream
        client.end_stream().await?;

        info!("Completed processing {} lines", self.line_count);
        Ok(())
    }

    async fn send_batch(&mut self, client: &mut WebSocketClient) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        debug!("Sending batch of {} lines", self.buffer.len());

        for line in &self.buffer {
            if let Err(e) = client.send_line(line).await {
                error!("Failed to send line: {}", e);
                return Err(e);
            }
        }

        self.buffer.clear();
        Ok(())
    }
}
