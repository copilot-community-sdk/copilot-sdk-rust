// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Core types for the Copilot SDK.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

fn is_false(value: &bool) -> bool {
    !*value
}

// =============================================================================
// Protocol Version
// =============================================================================

/// SDK protocol version - must match copilot-agent-runtime server.
pub const SDK_PROTOCOL_VERSION: u32 = 2;

// =============================================================================
// Enums
// =============================================================================

/// Connection state of the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// System message mode for session configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemMessageMode {
    Append,
    Replace,
}

/// Attachment type for user messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    File,
    Directory,
}

/// Log level for the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

// =============================================================================
// Tool Types
// =============================================================================

/// Binary result from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolBinaryResult {
    pub data: String,
    pub mime_type: String,
    #[serde(rename = "type")]
    pub result_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result object returned from tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultObject {
    pub text_result_for_llm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_results_for_llm: Option<Vec<ToolBinaryResult>>,
    #[serde(default = "default_result_type")]
    pub result_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_log: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_telemetry: Option<HashMap<String, serde_json::Value>>,
}

fn default_result_type() -> String {
    "success".to_string()
}

impl ToolResultObject {
    /// Create a success result with text.
    pub fn text(result: impl Into<String>) -> Self {
        Self {
            text_result_for_llm: result.into(),
            binary_results_for_llm: None,
            result_type: "success".to_string(),
            error: None,
            session_log: None,
            tool_telemetry: None,
        }
    }

    /// Create an error result.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            text_result_for_llm: String::new(),
            binary_results_for_llm: None,
            result_type: "error".to_string(),
            error: Some(message.into()),
            session_log: None,
            tool_telemetry: None,
        }
    }
}

/// Convenient alias for tool results.
pub type ToolResult = ToolResultObject;

/// Information about a tool invocation from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInvocation {
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
}

impl ToolInvocation {
    /// Get an argument by name, deserializing to the specified type.
    pub fn arg<T: serde::de::DeserializeOwned>(&self, name: &str) -> crate::Result<T> {
        let args = self
            .arguments
            .as_ref()
            .ok_or_else(|| crate::CopilotError::ToolError("No arguments provided".into()))?;

        let value = args
            .get(name)
            .ok_or_else(|| crate::CopilotError::ToolError(format!("Missing argument: {}", name)))?;

        serde_json::from_value(value.clone()).map_err(|e| {
            crate::CopilotError::ToolError(format!("Invalid argument '{}': {}", name, e))
        })
    }
}

// =============================================================================
// Permission Types
// =============================================================================

/// Permission request from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(flatten)]
    pub extension_data: HashMap<String, serde_json::Value>,
}

/// Result of a permission request (response to CLI).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestResult {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<serde_json::Value>>,
}

impl PermissionRequestResult {
    /// Create an approved permission result.
    pub fn approved() -> Self {
        Self {
            kind: "approved".to_string(),
            rules: None,
        }
    }

    /// Create a denied permission result.
    pub fn denied() -> Self {
        Self {
            kind: "denied-no-approval-rule-and-could-not-request-from-user".to_string(),
            rules: None,
        }
    }

    /// Create a denied permission result with a human-readable reason.
    ///
    /// The reason is included in the rules so the CLI/LLM can understand
    /// why the request was denied and adapt its approach.
    pub fn denied_with_reason(reason: impl Into<String>) -> Self {
        Self {
            kind: "denied-no-approval-rule-and-could-not-request-from-user".to_string(),
            rules: Some(vec![serde_json::json!({
                "reason": reason.into()
            })]),
        }
    }

    /// Returns true if the permission was approved.
    pub fn is_approved(&self) -> bool {
        self.kind == "approved"
    }

    /// Returns true if the permission was denied.
    pub fn is_denied(&self) -> bool {
        self.kind.starts_with("denied")
    }
}

// =============================================================================
// Configuration Types
// =============================================================================

/// System message configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMessageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<SystemMessageMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Azure-specific provider options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AzureOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
}

/// Provider configuration for BYOK (Bring Your Own Key).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub provider_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure: Option<AzureOptions>,
}

// Environment variable names for BYOK configuration
impl ProviderConfig {
    /// Environment variable for API key
    pub const ENV_API_KEY: &'static str = "COPILOT_SDK_BYOK_API_KEY";
    /// Environment variable for base URL
    pub const ENV_BASE_URL: &'static str = "COPILOT_SDK_BYOK_BASE_URL";
    /// Environment variable for provider type
    pub const ENV_PROVIDER_TYPE: &'static str = "COPILOT_SDK_BYOK_PROVIDER_TYPE";
    /// Environment variable for model
    pub const ENV_MODEL: &'static str = "COPILOT_SDK_BYOK_MODEL";

    /// Check if BYOK environment variables are configured.
    ///
    /// Returns true if `COPILOT_SDK_BYOK_API_KEY` is set and non-empty.
    pub fn is_env_configured() -> bool {
        std::env::var(Self::ENV_API_KEY)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Load ProviderConfig from `COPILOT_SDK_BYOK_*` environment variables.
    ///
    /// Returns `Some(ProviderConfig)` if API key is set, `None` otherwise.
    ///
    /// Environment variables:
    /// - `COPILOT_SDK_BYOK_API_KEY` (required): API key for the provider
    /// - `COPILOT_SDK_BYOK_BASE_URL` (optional): Base URL (defaults to OpenAI)
    /// - `COPILOT_SDK_BYOK_PROVIDER_TYPE` (optional): Provider type (defaults to "openai")
    pub fn from_env() -> Option<Self> {
        if !Self::is_env_configured() {
            return None;
        }

        let api_key = std::env::var(Self::ENV_API_KEY).ok();
        let base_url = std::env::var(Self::ENV_BASE_URL)
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let provider_type = std::env::var(Self::ENV_PROVIDER_TYPE)
            .ok()
            .or_else(|| Some("openai".to_string()));

        Some(Self {
            base_url,
            provider_type,
            api_key,
            wire_api: None,
            bearer_token: None,
            azure: None,
        })
    }

    /// Load model from `COPILOT_SDK_BYOK_MODEL` environment variable.
    ///
    /// Returns `Some(model)` if set and non-empty, `None` otherwise.
    pub fn model_from_env() -> Option<String> {
        std::env::var(Self::ENV_MODEL)
            .ok()
            .filter(|v| !v.is_empty())
    }
}

// =============================================================================
// MCP Server Configuration
// =============================================================================

/// Configuration for a local/stdio MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpLocalServerConfig {
    pub tools: Vec<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub server_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// Configuration for a remote MCP server (HTTP or SSE).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRemoteServerConfig {
    pub tools: Vec<String>,
    pub url: String,
    #[serde(default = "default_mcp_type", rename = "type")]
    pub server_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

fn default_mcp_type() -> String {
    "http".to_string()
}

/// MCP server configuration (either local or remote).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    Local(McpLocalServerConfig),
    Remote(McpRemoteServerConfig),
}

// =============================================================================
// Custom Agent Configuration
// =============================================================================

/// Configuration for a custom agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAgentConfig {
    pub name: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infer: Option<bool>,
}

// =============================================================================
// Attachment Types
// =============================================================================

/// Attachment item for user messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMessageAttachment {
    #[serde(rename = "type")]
    pub attachment_type: AttachmentType,
    pub path: String,
    pub display_name: String,
}

// =============================================================================
// Tool Definition (SDK-side)
// =============================================================================

/// Tool definition for registration with a session.
///
/// Use the builder pattern to create tools:
/// ```no_run
/// use copilot_sdk::{Client, SessionConfig, Tool, ToolHandler, ToolResultObject};
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> copilot_sdk::Result<()> {
/// let client = Client::builder().build()?;
/// client.start().await?;
///
/// let tool = Tool::new("get_weather")
///     .description("Get weather for a city")
///     .schema(serde_json::json!({
///         "type": "object",
///         "properties": { "city": { "type": "string" } },
///         "required": ["city"]
///     }));
///
/// let session = client.create_session(SessionConfig {
///     tools: vec![tool.clone()],
///     ..Default::default()
/// }).await?;
///
/// let handler: ToolHandler = Arc::new(|_name, args| {
///     let city = args.get("city").and_then(|v| v.as_str()).unwrap_or("unknown");
///     ToolResultObject::text(format!("Weather in {}: sunny", city))
/// });
/// session.register_tool_with_handler(tool, Some(handler)).await;
/// client.stop().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
    // Handler is stored separately in Session since it's not Clone-friendly
}

impl Tool {
    /// Create a new tool with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            parameters_schema: serde_json::json!({}),
        }
    }

    /// Set the tool description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the parameters JSON schema.
    pub fn schema(mut self, schema: serde_json::Value) -> Self {
        self.parameters_schema = schema;
        self
    }

    /// Derive the parameters JSON schema from a Rust type (requires the `schemars` feature).
    #[cfg(feature = "schemars")]
    pub fn typed_schema<T: schemars::JsonSchema>(mut self) -> Self {
        let schema = schemars::schema_for!(T);
        match serde_json::to_value(&schema) {
            Ok(value) => self.parameters_schema = value,
            Err(err) => {
                tracing::warn!("Failed to serialize schemars schema: {err}");
                self.parameters_schema = serde_json::json!({});
            }
        }
        self
    }
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

// Serialization for sending tool definitions to the CLI
impl Serialize for Tool {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Tool", 3)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("description", &self.description)?;
        state.serialize_field("parametersSchema", &self.parameters_schema)?;
        state.end()
    }
}

// =============================================================================
// Infinite Session Configuration
// =============================================================================

/// Configuration for infinite sessions (automatic context compaction).
///
/// When enabled, the SDK will automatically manage conversation context to prevent
/// buffer exhaustion. Thresholds are expressed as fractions (0.0 to 1.0).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfiniteSessionConfig {
    /// Enable infinite sessions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Threshold for background compaction (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_compaction_threshold: Option<f64>,
    /// Threshold for buffer exhaustion handling (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_exhaustion_threshold: Option<f64>,
}

impl InfiniteSessionConfig {
    /// Create an enabled infinite session config with default thresholds.
    pub fn enabled() -> Self {
        Self {
            enabled: Some(true),
            background_compaction_threshold: None,
            buffer_exhaustion_threshold: None,
        }
    }

    /// Create an infinite session config with custom thresholds.
    pub fn with_thresholds(background: f64, exhaustion: f64) -> Self {
        Self {
            enabled: Some(true),
            background_compaction_threshold: Some(background),
            buffer_exhaustion_threshold: Some(exhaustion),
        }
    }
}

// =============================================================================
// Session Configuration
// =============================================================================

/// Configuration for creating a new session.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<SystemMessageConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub streaming: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_agents: Option<Vec<CustomAgentConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_directories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "requestPermission")]
    pub request_permission: Option<bool>,
    /// Infinite session configuration for automatic context compaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infinite_sessions: Option<InfiniteSessionConfig>,

    /// If true and provider/model not explicitly set, load from `COPILOT_SDK_BYOK_*` env vars.
    ///
    /// Default: false (explicit configuration preferred over environment variables)
    #[serde(skip)]
    pub auto_byok_from_env: bool,

    /// Command patterns to automatically deny in permission requests.
    ///
    /// Each pattern is checked against the `fullCommandText` field in permission
    /// requests. If any pattern matches (case-insensitive whole-word), the request
    /// is automatically denied before reaching the user's permission handler.
    ///
    /// Example patterns:
    /// - `"git push"` — deny any command containing "git push"
    /// - `"git commit"` — deny any command containing "git commit"
    /// - `"rm -rf"` — deny recursive force removal
    ///
    /// This is a safety net that works in addition to `deny_tools` on `ClientOptions`.
    /// While `deny_tools` prevents the CLI from even attempting the tool call,
    /// `denied_command_patterns` catches anything that slips through at the
    /// permission handler level.
    #[serde(skip)]
    pub denied_command_patterns: Option<Vec<String>>,
}

/// Configuration for resuming an existing session.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeSessionConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderConfig>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub streaming: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_agents: Option<Vec<CustomAgentConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_directories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "requestPermission")]
    pub request_permission: Option<bool>,

    /// If true and provider not explicitly set, load from `COPILOT_SDK_BYOK_*` env vars.
    ///
    /// Default: false (explicit configuration preferred over environment variables)
    #[serde(skip)]
    pub auto_byok_from_env: bool,

    /// Command patterns to automatically deny in permission requests.
    ///
    /// See [`SessionConfig::denied_command_patterns`] for details.
    #[serde(skip)]
    pub denied_command_patterns: Option<Vec<String>>,
}

/// Options for sending a message.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageOptions {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<UserMessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

impl From<&str> for MessageOptions {
    fn from(prompt: &str) -> Self {
        Self {
            prompt: prompt.to_string(),
            attachments: None,
            mode: None,
        }
    }
}

impl From<String> for MessageOptions {
    fn from(prompt: String) -> Self {
        Self {
            prompt,
            attachments: None,
            mode: None,
        }
    }
}

// =============================================================================
// Client Options
// =============================================================================

/// Options for creating a CopilotClient.
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub cli_path: Option<PathBuf>,
    pub cli_args: Option<Vec<String>>,
    pub cwd: Option<PathBuf>,
    pub port: u16,
    pub use_stdio: bool,
    pub cli_url: Option<String>,
    pub log_level: LogLevel,
    pub auto_start: bool,
    pub auto_restart: bool,
    pub environment: Option<HashMap<String, String>>,

    /// Tool specifications to deny (passed as `--deny-tool` arguments to the CLI).
    ///
    /// Each entry follows the CLI's tool specification format:
    /// - `"shell(git push)"` — deny a specific shell command
    /// - `"shell(git)"` — deny all git commands
    /// - `"shell(rm)"` — deny rm commands
    /// - `"shell"` — deny all shell commands
    /// - `"write"` — deny file write operations
    /// - `"MCP_SERVER(tool_name)"` — deny a specific MCP tool
    ///
    /// `--deny-tool` takes precedence over `--allow-tool` and `--allow-all-tools`.
    pub deny_tools: Option<Vec<String>>,

    /// Tool specifications to allow without manual approval
    /// (passed as `--allow-tool` arguments to the CLI).
    ///
    /// Each entry follows the same format as `deny_tools`.
    pub allow_tools: Option<Vec<String>>,

    /// If true, passes `--allow-all-tools` to the CLI.
    ///
    /// This allows Copilot to use any tool without asking for approval.
    /// Use `deny_tools` in combination to create an allowlist with exceptions.
    pub allow_all_tools: bool,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            cli_path: None,
            cli_args: None,
            cwd: None,
            port: 0,
            use_stdio: true,
            cli_url: None,
            log_level: LogLevel::Info,
            auto_start: true,
            auto_restart: true,
            environment: None,
            deny_tools: None,
            allow_tools: None,
            allow_all_tools: false,
        }
    }
}

// =============================================================================
// Response Types
// =============================================================================

/// Metadata about a session.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub session_id: String,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub is_remote: bool,
}

/// Response from a ping request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub message: String,
    pub timestamp: i64,
    #[serde(default)]
    pub protocol_version: Option<u32>,
}

/// Response from status.get request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetStatusResponse {
    pub version: String,
    pub protocol_version: u32,
}

/// Response from auth.getStatus request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusResponse {
    pub is_authenticated: bool,
    #[serde(default)]
    pub auth_type: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub status_message: Option<String>,
}

/// Model capabilities - what the model supports.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    #[serde(default)]
    pub supports: ModelSupports,
    #[serde(default)]
    pub limits: ModelLimits,
}

/// What features a model supports.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelSupports {
    #[serde(default)]
    pub vision: bool,
}

/// Model limits.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLimits {
    #[serde(default)]
    pub max_prompt_tokens: Option<u32>,
    #[serde(default)]
    pub max_context_window_tokens: u32,
}

/// Model policy state.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelPolicy {
    pub state: String,
    #[serde(default)]
    pub terms: String,
}

/// Model billing information.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelBilling {
    #[serde(default)]
    pub multiplier: f64,
}

/// Information about an available model.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub policy: Option<ModelPolicy>,
    #[serde(default)]
    pub billing: Option<ModelBilling>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_text() {
        let result = ToolResult::text("Hello, world!");
        assert_eq!(result.text_result_for_llm, "Hello, world!");
        assert_eq!(result.result_type, "success");
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("Something went wrong");
        assert_eq!(result.result_type, "error");
        assert_eq!(result.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_permission_result() {
        let approved = PermissionRequestResult::approved();
        assert_eq!(approved.kind, "approved");
        assert!(approved.is_approved());
        assert!(!approved.is_denied());

        let denied = PermissionRequestResult::denied();
        assert!(denied.kind.starts_with("denied"));
        assert!(denied.is_denied());
        assert!(!denied.is_approved());
    }

    #[test]
    fn test_permission_result_denied_with_reason() {
        let result = PermissionRequestResult::denied_with_reason("git push is not allowed");
        assert!(result.is_denied());
        assert!(result.rules.is_some());
        let rules = result.rules.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["reason"], "git push is not allowed");
    }

    #[test]
    fn test_message_options_from_str() {
        let opts: MessageOptions = "Hello".into();
        assert_eq!(opts.prompt, "Hello");
    }

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert!(config.model.is_none());
        assert!(config.tools.is_empty());
    }

    #[test]
    fn test_session_config_serialization_with_new_fields() {
        let config = SessionConfig {
            session_id: Some("sess-1".into()),
            model: Some("gpt-4.1".into()),
            config_dir: Some(PathBuf::from("/tmp/copilot")),
            streaming: true,
            skill_directories: Some(vec!["skills".into()]),
            disabled_skills: Some(vec!["legacy_skill".into()]),
            request_permission: Some(true),
            ..Default::default()
        };

        let value = serde_json::to_value(&config).unwrap();
        assert_eq!(value["sessionId"], "sess-1");
        assert_eq!(value["model"], "gpt-4.1");
        assert_eq!(value["configDir"], "/tmp/copilot");
        assert_eq!(value["streaming"], true);
        assert_eq!(value["skillDirectories"][0], "skills");
        assert_eq!(value["disabledSkills"][0], "legacy_skill");
        assert_eq!(value["requestPermission"], true);
    }

    #[test]
    fn test_tool_builder() {
        let tool = Tool::new("my_tool")
            .description("A test tool")
            .schema(serde_json::json!({"type": "object"}));

        assert_eq!(tool.name, "my_tool");
        assert_eq!(tool.description, "A test tool");
    }
}
