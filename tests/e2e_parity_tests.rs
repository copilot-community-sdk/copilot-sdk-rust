// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! End-to-end tests for parity features (model/mode/plan/agent/workspace/shell/telemetry).
//!
//! Run with: `cargo test --features e2e -- --test-threads=1`

#![cfg(feature = "e2e")]

use copilot_sdk::{
    find_copilot_cli, Client, CustomAgentConfig, InfiniteSessionConfig, LogLevel, PlanData,
    SessionConfig, ShellExecOptions, ShellSignal, TelemetryConfig, Tool,
};
use std::sync::Once;
use std::time::Duration;

// =============================================================================
// Test Helpers (duplicated from e2e_tests.rs since test files are independent)
// =============================================================================

static BYOK_INIT: Once = Once::new();

fn load_byok_env_file() {
    BYOK_INIT.call_once(|| {
        let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let env_file = test_dir.join("byok.env");
        if !env_file.exists() {
            return;
        }
        let content = match std::fs::read_to_string(&env_file) {
            Ok(c) => c,
            Err(_) => return,
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                unsafe { std::env::set_var(key.trim(), value.trim()) };
            }
        }
    });
}

fn skip_if_no_cli() -> bool {
    find_copilot_cli().is_none()
}

macro_rules! skip_if_no_cli {
    () => {
        load_byok_env_file();
        if skip_if_no_cli() {
            eprintln!("Skipping: Copilot CLI not found");
            return;
        }
    };
}

async fn create_test_client() -> copilot_sdk::Result<Client> {
    let client = Client::builder()
        .use_stdio(true)
        .log_level(LogLevel::Info)
        .build()?;
    client.start().await?;
    Ok(client)
}

fn byok_session_config() -> SessionConfig {
    SessionConfig {
        auto_byok_from_env: true,
        ..Default::default()
    }
}

// =============================================================================
// Plan Management Tests
// =============================================================================

#[tokio::test]
async fn test_plan_read_empty() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    // Reading plan on a new session should return None or empty
    let plan = session.read_plan().await;
    println!("Initial plan: {:?}", plan);
    // May return Ok(None) or an error if plan feature isn't enabled for the session

    client.stop().await;
}

#[tokio::test]
async fn test_plan_update_and_read() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    // Update plan
    let update_result = session
        .update_plan(&PlanData {
            content: Some("1. Design API\n2. Write tests\n3. Implement".into()),
            title: Some("Development Plan".into()),
        })
        .await;

    println!("Plan update result: {:?}", update_result);

    if update_result.is_ok() {
        // Read back
        let plan = session.read_plan().await.expect("Failed to read plan");
        println!("Read back plan: {:?}", plan);
    }

    client.stop().await;
}

#[tokio::test]
async fn test_plan_delete() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    // Update then delete
    let _ = session
        .update_plan(&PlanData {
            content: Some("Temporary plan".into()),
            title: None,
        })
        .await;

    let result = session.delete_plan().await;
    println!("Plan delete result: {:?}", result);

    client.stop().await;
}

// =============================================================================
// Agent Management Tests
// =============================================================================

#[tokio::test]
async fn test_list_agents() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            custom_agents: Some(vec![CustomAgentConfig {
                name: "test-agent".into(),
                prompt: "You are a test agent.".into(),
                display_name: Some("Test Agent".into()),
                description: Some("For testing".into()),
                tools: None,
                mcp_servers: None,
                infer: Some(true),
            }]),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session");

    let agents = session.list_agents().await.expect("Failed to list agents");
    println!("Available agents: {:?}", agents);

    client.stop().await;
}

#[tokio::test]
async fn test_get_current_agent_none() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    // No agent should be selected initially
    let current = session.get_current_agent().await;
    println!("Current agent: {:?}", current);

    client.stop().await;
}

#[tokio::test]
async fn test_select_and_deselect_agent() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            custom_agents: Some(vec![CustomAgentConfig {
                name: "my-agent".into(),
                prompt: "You are helpful.".into(),
                display_name: Some("My Agent".into()),
                description: None,
                tools: None,
                mcp_servers: None,
                infer: Some(true),
            }]),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session");

    // Select
    let select_result = session.select_agent("my-agent").await;
    println!("Select result: {:?}", select_result);

    // Deselect
    let deselect_result = session.deselect_agent().await;
    println!("Deselect result: {:?}", deselect_result);

    client.stop().await;
}

// =============================================================================
// Shell Operation Tests
// =============================================================================

#[tokio::test]
async fn test_shell_exec() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    let result = session
        .shell_exec(ShellExecOptions {
            command: "echo hello_from_sdk".into(),
            cwd: None,
            env: None,
        })
        .await;

    println!("Shell exec result: {:?}", result);

    if let Ok(exec_result) = result {
        assert!(
            !exec_result.process_id.is_empty(),
            "Process ID should not be empty"
        );

        // Try to kill it (may already be finished)
        let kill_result = session
            .shell_kill(&exec_result.process_id, ShellSignal::SIGTERM)
            .await;
        println!("Shell kill result: {:?}", kill_result);
    }

    client.stop().await;
}

// =============================================================================
// Workspace Operation Tests
// =============================================================================

#[tokio::test]
async fn test_workspace_list_files() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            infinite_sessions: Some(InfiniteSessionConfig::enabled()),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session");

    // Workspace may be empty initially
    let result = session.workspace_list_files().await;
    println!("Workspace files: {:?}", result);

    client.stop().await;
}

#[tokio::test]
async fn test_workspace_create_and_read_file() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            infinite_sessions: Some(InfiniteSessionConfig::enabled()),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session");

    // Only test if workspace is available
    if session.workspace_path().is_none() {
        println!("Skipping - workspace path not available");
        client.stop().await;
        return;
    }

    // Create a file
    let create_result = session
        .workspace_create_file("test_file.txt", "Hello from SDK tests")
        .await;
    println!("Create file result: {:?}", create_result);

    if create_result.is_ok() {
        // Read it back
        let content = session
            .workspace_read_file("test_file.txt")
            .await
            .expect("Failed to read file");
        assert_eq!(content, "Hello from SDK tests");
    }

    client.stop().await;
}

// =============================================================================
// Fleet Management Tests
// =============================================================================

#[tokio::test]
async fn test_start_fleet() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    // Fleet start may or may not be supported depending on CLI version
    let result = session.start_fleet(None).await;
    println!("Fleet start (no prompt): {:?}", result);

    client.stop().await;
}

// =============================================================================
// Session Config Fields Tests
// =============================================================================

#[tokio::test]
async fn test_session_with_client_name() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            client_name: Some("copilot-sdk-rust-tests".into()),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session with client_name");

    assert!(!session.session_id().is_empty());

    client.stop().await;
}

#[tokio::test]
async fn test_session_with_agent_option() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            custom_agents: Some(vec![CustomAgentConfig {
                name: "starter-agent".into(),
                prompt: "You are a starter agent.".into(),
                display_name: None,
                description: None,
                tools: None,
                mcp_servers: None,
                infer: Some(true),
            }]),
            agent: Some("starter-agent".into()),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session with agent");

    assert!(!session.session_id().is_empty());

    client.stop().await;
}

// =============================================================================
// Tool Options Tests
// =============================================================================

#[tokio::test]
async fn test_tool_with_overrides_built_in() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let tool = Tool::new("read_file")
        .description("Custom read_file that overrides built-in")
        .overrides_built_in_tool(true)
        .skip_permission(true)
        .parameter("path", "string", "File path to read", true);

    let session = client
        .create_session(SessionConfig {
            tools: vec![tool],
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session with override tool");

    assert!(!session.session_id().is_empty());

    // Verify the tool was registered
    let registered_tool = session.get_tool("read_file").await;
    assert!(registered_tool.is_some(), "Tool should be registered");

    client.stop().await;
}

// =============================================================================
// Telemetry Configuration Tests
// =============================================================================

#[tokio::test]
async fn test_telemetry_config_on_client() {
    skip_if_no_cli!();

    let client = Client::builder()
        .use_stdio(true)
        .log_level(LogLevel::Info)
        .telemetry(TelemetryConfig {
            // Use file exporter to avoid needing an OTLP endpoint
            file_path: Some(
                std::env::temp_dir()
                    .join("copilot-sdk-test-traces.jsonl")
                    .to_string_lossy()
                    .to_string(),
            ),
            exporter_type: Some("file".into()),
            source_name: Some("copilot-sdk-rust-tests".into()),
            capture_content: Some(false),
            otlp_endpoint: None,
        })
        .build()
        .expect("Failed to build client with telemetry");

    client
        .start()
        .await
        .expect("Failed to start with telemetry");

    let session = client
        .create_session(byok_session_config())
        .await
        .expect("Failed to create session");

    assert!(!session.session_id().is_empty());

    client.stop().await;
}

// =============================================================================
// Manual Compaction Tests
// =============================================================================

#[tokio::test]
async fn test_compact_with_infinite_session() {
    skip_if_no_cli!();

    let client = create_test_client().await.expect("Failed to create client");

    let session = client
        .create_session(SessionConfig {
            infinite_sessions: Some(InfiniteSessionConfig::enabled()),
            ..byok_session_config()
        })
        .await
        .expect("Failed to create session");

    // Send a few messages to build up context
    let _ = tokio::time::timeout(
        Duration::from_secs(30),
        session.send_and_collect("Say hello briefly.", None),
    )
    .await;

    // Attempt compaction
    let result = session.compact().await;
    println!("Compaction after messages: {:?}", result);
    // May succeed or fail depending on context size - we just verify the RPC works

    client.stop().await;
}
