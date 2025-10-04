use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, info};

use crate::StreamConfig;

pub struct WebSocketClient {
    sender: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    stream_id: Option<String>,
}

impl WebSocketClient {
    pub async fn new(url: &str, token: &str, config: &StreamConfig) -> Result<Self> {
        info!("Connecting to WebSocket: {}", url);

        // Build URL with stream parameters
        let mut ws_url = if token.is_empty() {
            url.to_string()
        } else {
            format!("{}?token={}", url, token)
        };

        // Add stream name and description to URL
        ws_url.push_str(&format!("&name={}", urlencoding::encode(&config.name)));
        if let Some(description) = &config.description {
            ws_url.push_str(&format!(
                "&description={}",
                urlencoding::encode(description)
            ));
        }

        debug!("Final WebSocket URL: {}", ws_url);

        let (ws_stream, response) = connect_async(&ws_url)
            .await
            .map_err(|e| anyhow!("Failed to connect to WebSocket: {}", e))?;
        if response.status() != 101 {
            return Err(anyhow!(
                "WebSocket connection failed with status: {}",
                response.status()
            ));
        }

        info!("WebSocket connection established");

        Ok(Self {
            sender: ws_stream,
            stream_id: None,
        })
    }

    pub async fn create_stream(&mut self, _config: &StreamConfig) -> Result<String> {
        debug!("Waiting for stream creation confirmation from backend...");

        // Backend creates stream automatically on connection based on URL params
        if let Some(response) = self.sender.next().await {
            match response? {
                Message::Text(text) => {
                    debug!("Received response: {}", text);
                    let response_json: serde_json::Value = serde_json::from_str(&text)?;

                    // Try both camelCase and snake_case field names
                    let stream_id = response_json
                        .get("streamId")
                        .or_else(|| response_json.get("stream_id"))
                        .and_then(|v| v.as_str());

                    if let Some(stream_id) = stream_id {
                        self.stream_id = Some(stream_id.to_string());
                        info!("Stream created with ID: {}", stream_id);
                        Ok(stream_id.to_string())
                    } else if let Some(error) = response_json.get("error") {
                        Err(anyhow!("Failed to create stream: {}", error))
                    } else {
                        Err(anyhow!("Unexpected response format: {}", text))
                    }
                }
                Message::Close(_) => {
                    Err(anyhow!("WebSocket connection closed while creating stream"))
                }
                _ => Err(anyhow!("Unexpected message type while creating stream")),
            }
        } else {
            Err(anyhow!("No response received for create_stream"))
        }
    }

    pub async fn send_line(&mut self, line: &str) -> Result<()> {
        let stream_id = self
            .stream_id
            .as_ref()
            .ok_or_else(|| anyhow!("Stream not created. Call create_stream first"))?;

        let line_message = json!({
            "type": "line",
            "stream_id": stream_id,
            "content": line
        });

        debug!("Sending line: {}", line);

        self.sender
            .send(Message::Text(line_message.to_string()))
            .await
            .map_err(|e| anyhow!("Failed to send line: {}", e))?;

        Ok(())
    }

    pub async fn end_stream(&mut self) -> Result<()> {
        let stream_id = self
            .stream_id
            .as_ref()
            .ok_or_else(|| anyhow!("Stream not created. Call create_stream first"))?;

        let end_message = json!({
            "type": "end_stream",
            "stream_id": stream_id
        });

        debug!("Sending end_stream message");

        self.sender
            .send(Message::Text(end_message.to_string()))
            .await
            .map_err(|e| anyhow!("Failed to send end_stream message: {}", e))?;

        // Wait for confirmation
        if let Some(response) = self.sender.next().await {
            match response? {
                Message::Text(text) => {
                    debug!("End stream response: {}", text);
                }
                Message::Close(_) => {
                    info!("WebSocket connection closed");
                }
                _ => {}
            }
        }

        info!("Stream ended successfully");
        Ok(())
    }
}
