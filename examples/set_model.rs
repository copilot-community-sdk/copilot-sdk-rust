// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Example: Switch models mid-session.

use copilot_sdk::{Client, SessionConfig, SessionEventData, SetModelOptions};

#[tokio::main]
async fn main() -> copilot_sdk::Result<()> {
    let client = Client::builder().build()?;
    client.start().await?;

    let session = client
        .create_session(SessionConfig {
            model: Some("gpt-4.1".into()),
            streaming: true,
            ..Default::default()
        })
        .await?;

    // Check current model
    let model = session.get_model().await?;
    println!("Current model: {model}");

    // Switch to a different model with reasoning effort
    session
        .set_model(
            "claude-sonnet-4",
            Some(SetModelOptions {
                reasoning_effort: Some("high".into()),
            }),
        )
        .await?;

    println!("Switched model to claude-sonnet-4");

    // Send a message with the new model
    let mut events = session.subscribe();
    session.send("What is 2+2?").await?;

    while let Ok(event) = events.recv().await {
        match &event.data {
            SessionEventData::AssistantMessage(msg) => println!("Response: {}", msg.content),
            SessionEventData::SessionIdle(_) => break,
            _ => {}
        }
    }

    client.stop().await;
    Ok(())
}
