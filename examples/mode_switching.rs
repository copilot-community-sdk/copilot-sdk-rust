// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Example: Switch between interactive, plan, and autopilot modes.

use copilot_sdk::{Client, SessionConfig, SessionMode};

#[tokio::main]
async fn main() -> copilot_sdk::Result<()> {
    let client = Client::builder().build()?;
    client.start().await?;

    let session = client.create_session(SessionConfig::default()).await?;

    // Check current mode
    let mode = session.get_mode().await?;
    println!("Current mode: {:?}", mode);

    // Switch to plan mode
    session.set_mode(SessionMode::Plan).await?;
    println!("Switched to plan mode");

    // Switch to autopilot mode
    session.set_mode(SessionMode::Autopilot).await?;
    println!("Switched to autopilot mode");

    // Switch back to interactive
    session.set_mode(SessionMode::Interactive).await?;
    println!("Back to interactive mode");

    client.stop().await;
    Ok(())
}
