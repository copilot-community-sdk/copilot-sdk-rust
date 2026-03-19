// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Example: Plan management operations.

use copilot_sdk::{Client, PlanData, SessionConfig};

#[tokio::main]
async fn main() -> copilot_sdk::Result<()> {
    let client = Client::builder().build()?;
    client.start().await?;

    let session = client.create_session(SessionConfig::default()).await?;

    // Read the plan (initially empty)
    let plan = session.read_plan().await?;
    println!("Current plan: {:?}", plan);

    // Update the plan
    session
        .update_plan(&PlanData {
            content: Some("Step 1: Implement feature\nStep 2: Write tests".into()),
            title: Some("Implementation Plan".into()),
        })
        .await?;
    println!("Plan updated");

    // Read back
    let plan = session.read_plan().await?;
    println!("Updated plan: {:?}", plan);

    // Delete the plan
    session.delete_plan().await?;
    println!("Plan deleted");

    client.stop().await;
    Ok(())
}
