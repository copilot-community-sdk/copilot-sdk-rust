// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Example: Configure OpenTelemetry for the Copilot CLI.

use copilot_sdk::{Client, SessionConfig, SessionEventData, TelemetryConfig};

#[tokio::main]
async fn main() -> copilot_sdk::Result<()> {
    let client = Client::builder()
        .telemetry(TelemetryConfig {
            otlp_endpoint: Some("http://localhost:4318".into()),
            exporter_type: Some("otlp-http".into()),
            source_name: Some("my-rust-app".into()),
            capture_content: Some(true),
            file_path: None,
        })
        .build()?;

    client.start().await?;

    let session = client.create_session(SessionConfig::default()).await?;

    let mut events = session.subscribe();
    session.send("Hello with telemetry!").await?;

    while let Ok(event) = events.recv().await {
        match &event.data {
            SessionEventData::AssistantMessage(msg) => println!("{}", msg.content),
            SessionEventData::SessionIdle(_) => break,
            _ => {}
        }
    }

    client.stop().await;
    Ok(())
}
