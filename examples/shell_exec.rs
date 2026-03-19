// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Example: Execute shell commands in a session.

use copilot_sdk::{Client, SessionConfig, ShellExecOptions, ShellSignal};

#[tokio::main]
async fn main() -> copilot_sdk::Result<()> {
    let client = Client::builder().build()?;
    client.start().await?;

    let session = client.create_session(SessionConfig::default()).await?;

    // Execute a shell command
    let result = session
        .shell_exec(ShellExecOptions {
            command: "echo Hello from Copilot SDK".into(),
            cwd: None,
            env: None,
        })
        .await?;

    println!("Process ID: {}", result.process_id);

    // Kill it if needed
    session
        .shell_kill(&result.process_id, ShellSignal::SIGTERM)
        .await
        .ok(); // May already be finished

    client.stop().await;
    Ok(())
}
