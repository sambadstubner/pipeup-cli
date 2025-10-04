use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

mod stream_processor;
mod websocket;

use stream_processor::StreamProcessor;
use websocket::WebSocketClient;

#[derive(Parser, Debug)]
#[command(name = "pipeup")]
#[command(about = "Stream CLI output to Pipeup dashboard in real-time")]
#[command(version = "0.2.0")]
pub struct Args {
    /// Name for the stream
    #[arg(short, long, env = "PIPEUP_STREAM_NAME")]
    pub name: Option<String>,

    /// Description for the stream
    #[arg(short, long, env = "PIPEUP_DESCRIPTION")]
    pub description: Option<String>,

    /// API token for authentication
    #[arg(short, long, env = "PIPEUP_TOKEN")]
    pub token: Option<String>,

    /// Backend URL
    #[arg(long, env = "PIPEUP_URL", default_value = "ws://localhost:3001")]
    pub url: String,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamConfig {
    pub name: String,
    pub description: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .init();

    // Validate required arguments
    let token = args.token.ok_or_else(|| {
        anyhow::anyhow!(
            "API token is required. Set PIPEUP_TOKEN environment variable or use --token"
        )
    })?;

    let stream_name = args
        .name
        .unwrap_or_else(|| format!("stream-{}", Uuid::new_v4().to_string()[..8].to_string()));

    let stream_config = StreamConfig {
        name: stream_name.clone(),
        description: args.description.clone(),
    };

    info!("Starting Pipeup CLI - Stream: {}", stream_name);
    if let Some(ref desc) = args.description {
        info!("Description: {}", desc);
    }
    info!("Connecting to: {}", args.url);

    // Create WebSocket URL with token
    let ws_url = format!("{}/api/stream/ws", args.url.trim_end_matches('/'));
    let full_ws_url = format!("{}?token={}", ws_url, token);

    let mut client = WebSocketClient::new(&full_ws_url, "", &stream_config).await?;

    // Create stream processor
    let mut processor = StreamProcessor::new(stream_config);

    // Start the streaming process
    processor.process_stdin(&mut client).await?;

    info!("Stream completed successfully");
    Ok(())
}
