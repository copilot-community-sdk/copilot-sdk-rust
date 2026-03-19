// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Example: Agent management — list, select, and deselect agents.

use copilot_sdk::{Client, CustomAgentConfig, SessionConfig};

#[tokio::main]
async fn main() -> copilot_sdk::Result<()> {
    let client = Client::builder().build()?;
    client.start().await?;

    let session = client
        .create_session(SessionConfig {
            custom_agents: Some(vec![CustomAgentConfig {
                name: "code-reviewer".into(),
                prompt: "You are a code review expert.".into(),
                display_name: Some("Code Reviewer".into()),
                description: Some("Reviews code for bugs and improvements".into()),
                tools: None,
                mcp_servers: None,
                infer: None,
            }]),
            ..Default::default()
        })
        .await?;

    // List available agents
    let agents = session.list_agents().await?;
    println!("Available agents: {:?}", agents);

    // Check current agent
    let current = session.get_current_agent().await?;
    println!("Current agent: {:?}", current);

    // Select the code reviewer agent
    session.select_agent("code-reviewer").await?;
    println!("Selected code-reviewer agent");

    // Deselect
    session.deselect_agent().await?;
    println!("Deselected agent");

    client.stop().await;
    Ok(())
}
