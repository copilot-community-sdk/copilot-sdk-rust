// Copyright (c) 2026 Elias Bachaalany
// SPDX-License-Identifier: MIT

//! Session management for the Copilot SDK.
//!
//! A session represents a conversation with the Copilot CLI.

use crate::error::{CopilotError, Result};
use crate::events::{SessionEvent, SessionEventData};
use crate::types::{
    MessageOptions, PermissionRequest, PermissionRequestResult, Tool, ToolResultObject,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{RwLock, broadcast};

// =============================================================================
// Event Handler Types
// =============================================================================

/// Handler for session events.
pub type EventHandler = Arc<dyn Fn(&SessionEvent) + Send + Sync>;

/// Handler for permission requests.
pub type PermissionHandler =
    Arc<dyn Fn(&PermissionRequest) -> PermissionRequestResult + Send + Sync>;

/// Handler for tool invocations.
pub type ToolHandler = Arc<dyn Fn(&str, &Value) -> ToolResultObject + Send + Sync>;

/// Type alias for the invoke future.
pub type InvokeFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send>>;

type InvokeFn = dyn Fn(&str, Option<Value>) -> InvokeFuture + Send + Sync;

// =============================================================================
// Event Subscription
// =============================================================================

/// A subscription to session events.
///
/// Events are delivered via the broadcast channel receiver.
pub struct EventSubscription {
    pub receiver: broadcast::Receiver<SessionEvent>,
}

impl EventSubscription {
    /// Receive the next event.
    pub async fn recv(&mut self) -> std::result::Result<SessionEvent, broadcast::error::RecvError> {
        self.receiver.recv().await
    }
}

// =============================================================================
// Registered Tool
// =============================================================================

/// A tool registered with the session, including its handler.
#[derive(Clone)]
pub struct RegisteredTool {
    /// Tool definition.
    pub tool: Tool,
    /// Handler for tool invocations.
    pub handler: Option<ToolHandler>,
}

// =============================================================================
// Session
// =============================================================================

/// Shared session state.
struct SessionState {
    /// Registered tools.
    tools: HashMap<String, RegisteredTool>,
    /// Permission handler.
    permission_handler: Option<PermissionHandler>,
    /// Callback-based event handlers.
    event_handlers: HashMap<u64, EventHandler>,
    /// Next handler ID.
    next_handler_id: AtomicU64,
    /// Command patterns to automatically deny in permission requests.
    denied_command_patterns: Vec<String>,
}

/// A Copilot conversation session.
///
/// Sessions maintain conversation state, handle events, and manage tool execution.
///
/// # Example
///
/// ```no_run
/// use copilot_sdk::{Client, SessionConfig, SessionEventData};
///
/// #[tokio::main]
/// async fn main() -> copilot_sdk::Result<()> {
/// let client = Client::builder().build()?;
/// client.start().await?;
/// let session = client.create_session(SessionConfig::default()).await?;
///
/// // Subscribe to events
/// let mut events = session.subscribe();
///
/// // Send a message
/// session.send("Hello!").await?;
///
/// // Process events
/// while let Ok(event) = events.recv().await {
///     match &event.data {
///         SessionEventData::AssistantMessage(msg) => println!("{}", msg.content),
///         SessionEventData::SessionIdle(_) => break,
///         _ => {}
///     }
/// }
/// client.stop().await?;
/// # Ok(())
/// # }
/// ```
pub struct Session {
    /// Session ID.
    session_id: String,
    /// Workspace path for infinite sessions.
    workspace_path: Option<String>,
    /// Event broadcaster.
    event_tx: broadcast::Sender<SessionEvent>,
    /// Session state.
    state: Arc<RwLock<SessionState>>,
    /// JSON-RPC invoke function (injected by Client).
    invoke_fn: Arc<InvokeFn>,
}

impl Session {
    /// Create a new session.
    ///
    /// This is typically called by the Client when creating a session.
    pub fn new<F>(session_id: String, workspace_path: Option<String>, invoke_fn: F) -> Self
    where
        F: Fn(&str, Option<Value>) -> InvokeFuture + Send + Sync + 'static,
    {
        let (event_tx, _) = broadcast::channel(1024);

        Self {
            session_id,
            workspace_path,
            event_tx,
            state: Arc::new(RwLock::new(SessionState {
                tools: HashMap::new(),
                permission_handler: None,
                event_handlers: HashMap::new(),
                next_handler_id: AtomicU64::new(1),
                denied_command_patterns: Vec::new(),
            })),
            invoke_fn: Arc::new(invoke_fn),
        }
    }

    // =========================================================================
    // Session Properties
    // =========================================================================

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the workspace path for infinite sessions.
    ///
    /// Contains checkpoints/, plan.md, and files/ subdirectories.
    /// Returns None if infinite sessions are disabled.
    pub fn workspace_path(&self) -> Option<&str> {
        self.workspace_path.as_deref()
    }

    // =========================================================================
    // Event Handling
    // =========================================================================

    /// Subscribe to session events.
    ///
    /// Returns a receiver that will receive all session events.
    pub fn subscribe(&self) -> EventSubscription {
        EventSubscription {
            receiver: self.event_tx.subscribe(),
        }
    }

    /// Register a callback-based event handler.
    ///
    /// Returns a handler ID that can be used to unsubscribe.
    pub async fn on<F>(&self, handler: F) -> u64
    where
        F: Fn(&SessionEvent) + Send + Sync + 'static,
    {
        let mut state = self.state.write().await;
        let id = state.next_handler_id.fetch_add(1, Ordering::SeqCst);
        state.event_handlers.insert(id, Arc::new(handler));
        id
    }

    /// Unsubscribe a callback-based event handler.
    pub async fn off(&self, handler_id: u64) {
        let mut state = self.state.write().await;
        state.event_handlers.remove(&handler_id);
    }

    /// Dispatch an event to all subscribers.
    ///
    /// This is called by the Client when events are received.
    pub async fn dispatch_event(&self, event: SessionEvent) {
        // Send to broadcast channel
        let _ = self.event_tx.send(event.clone());

        // Call registered handlers
        let state = self.state.read().await;
        for handler in state.event_handlers.values() {
            handler(&event);
        }
    }

    // =========================================================================
    // Messaging
    // =========================================================================

    /// Send a message to the session.
    ///
    /// Returns the message ID.
    pub async fn send(&self, options: impl Into<MessageOptions>) -> Result<String> {
        let options = options.into();
        let params = serde_json::json!({
            "sessionId": self.session_id,
            "prompt": options.prompt,
            "attachments": options.attachments,
            "mode": options.mode,
        });

        let result = (self.invoke_fn)("session.send", Some(params)).await?;

        result
            .get("messageId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| CopilotError::Protocol("Missing messageId in response".into()))
    }

    /// Abort the current message processing.
    pub async fn abort(&self) -> Result<()> {
        let params = serde_json::json!({
            "sessionId": self.session_id,
        });

        (self.invoke_fn)("session.abort", Some(params)).await?;
        Ok(())
    }

    /// Get all messages in the session.
    pub async fn get_messages(&self) -> Result<Vec<SessionEvent>> {
        let params = serde_json::json!({
            "sessionId": self.session_id,
        });

        let result = (self.invoke_fn)("session.getMessages", Some(params)).await?;

        let events: Vec<SessionEvent> = result
            .get("events")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| SessionEvent::from_json(v).ok())
                    .collect()
            })
            .or_else(|| {
                result
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| SessionEvent::from_json(v).ok())
                            .collect()
                    })
            })
            .ok_or_else(|| {
                CopilotError::Protocol("Missing events in getMessages response".into())
            })?;

        Ok(events)
    }

    // =========================================================================
    // Tool Management
    // =========================================================================

    /// Register a tool with this session.
    pub async fn register_tool(&self, tool: Tool) {
        self.register_tool_with_handler(tool, None).await;
    }

    /// Register a tool with a handler.
    pub async fn register_tool_with_handler(&self, tool: Tool, handler: Option<ToolHandler>) {
        let mut state = self.state.write().await;
        let name = tool.name.clone();
        state.tools.insert(name, RegisteredTool { tool, handler });
    }

    /// Register multiple tools.
    pub async fn register_tools(&self, tools: Vec<Tool>) {
        let mut state = self.state.write().await;
        for tool in tools {
            let name = tool.name.clone();
            state.tools.insert(
                name,
                RegisteredTool {
                    tool,
                    handler: None,
                },
            );
        }
    }

    /// Get a registered tool by name.
    pub async fn get_tool(&self, name: &str) -> Option<Tool> {
        let state = self.state.read().await;
        state.tools.get(name).map(|rt| rt.tool.clone())
    }

    /// Get all registered tools.
    pub async fn get_tools(&self) -> Vec<Tool> {
        let state = self.state.read().await;
        state.tools.values().map(|rt| rt.tool.clone()).collect()
    }

    /// Invoke a tool handler.
    pub async fn invoke_tool(&self, name: &str, arguments: &Value) -> Result<ToolResultObject> {
        let state = self.state.read().await;
        let registered = state
            .tools
            .get(name)
            .ok_or_else(|| CopilotError::ToolNotFound(name.to_string()))?;

        let handler = registered
            .handler
            .as_ref()
            .ok_or_else(|| CopilotError::ToolError(format!("No handler for tool: {}", name)))?;

        Ok(handler(name, arguments))
    }

    // =========================================================================
    // Permission Handling
    // =========================================================================

    /// Register a permission handler.
    pub async fn register_permission_handler<F>(&self, handler: F)
    where
        F: Fn(&PermissionRequest) -> PermissionRequestResult + Send + Sync + 'static,
    {
        let mut state = self.state.write().await;
        state.permission_handler = Some(Arc::new(handler));
    }

    /// Set command patterns to automatically deny in permission requests.
    ///
    /// Each pattern is checked against the `fullCommandText` field in permission
    /// requests. If any pattern matches (case-insensitive, whole-word boundaries),
    /// the request is automatically denied before reaching the user's permission handler.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use copilot_sdk::{Client, SessionConfig};
    /// # async fn example() -> copilot_sdk::Result<()> {
    /// # let client = Client::builder().build()?;
    /// # let session = client.create_session(SessionConfig::default()).await?;
    /// session.set_denied_command_patterns(vec![
    ///     "git push".to_string(),
    ///     "git commit".to_string(),
    ///     "git add".to_string(),
    ///     "rm -rf".to_string(),
    /// ]).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_denied_command_patterns(&self, patterns: Vec<String>) {
        let mut state = self.state.write().await;
        state.denied_command_patterns = patterns;
    }

    /// Check if a command text matches any denied command pattern.
    ///
    /// For multi-word patterns (e.g., `"git push"`), checks that ALL words
    /// appear as whole words in the text. This handles CLI evasion tricks like
    /// `git -c commit.gpgsign=false commit` matching `"git commit"`.
    ///
    /// Uses case-insensitive matching with word boundary checks.
    fn matches_denied_pattern(text: &str, patterns: &[String]) -> Option<String> {
        let text_lower = text.to_lowercase();
        let text_bytes = text_lower.as_bytes();

        'pattern: for pattern in patterns {
            let pattern_lower = pattern.to_lowercase();
            let words: Vec<&str> = pattern_lower.split_whitespace().collect();

            if words.is_empty() {
                continue;
            }

            // Every word in the pattern must appear as a whole word in the text
            for word in &words {
                if !Self::contains_whole_word(text_bytes, &text_lower, word) {
                    continue 'pattern;
                }
            }

            return Some(pattern.clone());
        }

        None
    }

    /// Check if `text` contains `word` as a whole word (word boundary on both sides).
    fn contains_whole_word(text_bytes: &[u8], text_lower: &str, word: &str) -> bool {
        let word_len = word.len();
        let mut start = 0;

        while let Some(pos) = text_lower[start..].find(word) {
            let abs_pos = start + pos;

            // Check word boundary before
            let before_ok = abs_pos == 0 || {
                let b = text_bytes[abs_pos - 1];
                !b.is_ascii_alphanumeric() && b != b'_'
            };

            // Check word boundary after
            let end = abs_pos + word_len;
            let after_ok = end >= text_bytes.len() || {
                let b = text_bytes[end];
                !b.is_ascii_alphanumeric() && b != b'_'
            };

            if before_ok && after_ok {
                return true;
            }

            // Move past this occurrence to find next
            start = abs_pos + 1;
            if start >= text_lower.len() {
                break;
            }
        }

        false
    }

    /// Handle a permission request.
    ///
    /// First checks against `denied_command_patterns`. If a match is found,
    /// automatically denies with a reason. Otherwise, delegates to the
    /// registered permission handler (or denies by default if no handler is set).
    pub async fn handle_permission_request(
        &self,
        request: &PermissionRequest,
    ) -> PermissionRequestResult {
        let state = self.state.read().await;

        // Check denied command patterns first
        if !state.denied_command_patterns.is_empty() {
            // Check fullCommandText
            if let Some(full_cmd) = request.extension_data.get("fullCommandText") {
                if let Some(cmd_text) = full_cmd.as_str() {
                    if let Some(matched_pattern) =
                        Self::matches_denied_pattern(cmd_text, &state.denied_command_patterns)
                    {
                        tracing::info!(
                            pattern = %matched_pattern,
                            command = %cmd_text,
                            "Permission denied by denied_command_patterns"
                        );
                        return PermissionRequestResult::denied_with_reason(format!(
                            "Command denied by policy: matches pattern '{}'",
                            matched_pattern
                        ));
                    }
                }
            }

            // Also check commands[].identifier
            if let Some(commands) = request.extension_data.get("commands") {
                if let Some(cmds_array) = commands.as_array() {
                    for cmd in cmds_array {
                        if let Some(identifier) = cmd.get("identifier").and_then(|v| v.as_str()) {
                            if let Some(matched_pattern) = Self::matches_denied_pattern(
                                identifier,
                                &state.denied_command_patterns,
                            ) {
                                tracing::info!(
                                    pattern = %matched_pattern,
                                    identifier = %identifier,
                                    "Permission denied by denied_command_patterns (identifier)"
                                );
                                return PermissionRequestResult::denied_with_reason(format!(
                                    "Command denied by policy: matches pattern '{}'",
                                    matched_pattern
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Delegate to user-registered permission handler
        if let Some(handler) = &state.permission_handler {
            handler(request)
        } else {
            // Default: deny all permissions
            PermissionRequestResult::denied()
        }
    }

    // =========================================================================
    // Lifecycle
    // =========================================================================

    /// Destroy the session.
    pub async fn destroy(&self) -> Result<()> {
        let params = serde_json::json!({
            "sessionId": self.session_id,
        });

        (self.invoke_fn)("session.destroy", Some(params)).await?;
        Ok(())
    }
}

// =============================================================================
// Convenience methods for waiting on events
// =============================================================================

impl Session {
    /// Wait for the session to become idle.
    ///
    /// Collects all assistant messages and returns them when idle.
    pub async fn wait_for_idle(&self) -> Result<String> {
        let mut subscription = self.subscribe();
        let mut content = String::new();

        loop {
            match subscription.recv().await {
                Ok(event) => match &event.data {
                    SessionEventData::AssistantMessage(msg) => {
                        content.push_str(&msg.content);
                    }
                    SessionEventData::AssistantMessageDelta(delta) => {
                        content.push_str(&delta.delta_content);
                    }
                    SessionEventData::SessionIdle(_) => {
                        break;
                    }
                    SessionEventData::SessionError(err) => {
                        return Err(CopilotError::Protocol(format!(
                            "Session error: {}",
                            err.message
                        )));
                    }
                    _ => {}
                },
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(CopilotError::ConnectionClosed);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Continue - we missed some events but can recover
                }
            }
        }

        Ok(content)
    }

    /// Send a message and wait for the complete response.
    pub async fn send_and_wait(&self, options: impl Into<MessageOptions>) -> Result<String> {
        self.send(options).await?;
        self.wait_for_idle().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn mock_invoke(_method: &str, _params: Option<Value>) -> InvokeFuture {
        Box::pin(async { Ok(serde_json::json!({"messageId": "test-msg-123"})) })
    }

    fn mock_invoke_with_events(method: &str, _params: Option<Value>) -> InvokeFuture {
        let method = method.to_string();
        Box::pin(async move {
            if method == "session.getMessages" {
                return Ok(serde_json::json!({
                    "events": [{
                        "id": "evt-1",
                        "timestamp": "2024-01-01T00:00:00Z",
                        "type": "session.idle",
                        "data": {}
                    }]
                }));
            }
            Ok(serde_json::json!({"messageId": "test-msg-123"}))
        })
    }

    #[tokio::test]
    async fn test_session_id() {
        let session = Session::new("test-session-123".to_string(), None, mock_invoke);
        assert_eq!(session.session_id(), "test-session-123");
    }

    #[tokio::test]
    async fn test_workspace_path() {
        let session = Session::new(
            "test".to_string(),
            Some("/tmp/workspace".to_string()),
            mock_invoke,
        );
        assert_eq!(session.workspace_path(), Some("/tmp/workspace"));
    }

    #[tokio::test]
    async fn test_register_tool() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        let tool = Tool::new("my_tool").description("A test tool");

        session.register_tool(tool.clone()).await;

        let retrieved = session.get_tool("my_tool").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "my_tool");
    }

    #[tokio::test]
    async fn test_register_tool_with_handler() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        let tool = Tool::new("echo").description("Echo tool");
        let handler: ToolHandler = Arc::new(|_name, args| {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("empty");
            ToolResultObject::text(text)
        });

        session
            .register_tool_with_handler(tool, Some(handler))
            .await;

        let result = session
            .invoke_tool("echo", &serde_json::json!({"text": "hello"}))
            .await
            .unwrap();

        assert_eq!(result.text_result_for_llm, "hello");
    }

    #[tokio::test]
    async fn test_invoke_unknown_tool() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        let result = session.invoke_tool("unknown", &serde_json::json!({})).await;

        assert!(matches!(result, Err(CopilotError::ToolNotFound(_))));
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        let mut sub1 = session.subscribe();
        let mut sub2 = session.subscribe();

        // Dispatch an event
        let event = SessionEvent::from_json(&serde_json::json!({
            "id": "evt-1",
            "timestamp": "2024-01-01T00:00:00Z",
            "type": "session.idle",
            "data": {}
        }))
        .unwrap();

        session.dispatch_event(event).await;

        // Both subscribers should receive it
        let received1 = sub1.recv().await.unwrap();
        let received2 = sub2.recv().await.unwrap();

        assert_eq!(received1.id, "evt-1");
        assert_eq!(received2.id, "evt-1");
    }

    #[tokio::test]
    async fn test_callback_handler() {
        let session = Session::new("test".to_string(), None, mock_invoke);
        let call_count = Arc::new(AtomicUsize::new(0));

        let count_clone = Arc::clone(&call_count);
        let handler_id = session
            .on(move |_event| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        // Dispatch events
        let event = SessionEvent::from_json(&serde_json::json!({
            "id": "evt-callback-1",
            "timestamp": "2024-01-01T00:00:00Z",
            "type": "session.idle",
            "data": {}
        }))
        .unwrap();

        session.dispatch_event(event).await;

        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Unsubscribe
        session.off(handler_id).await;
    }

    #[tokio::test]
    async fn test_permission_handler() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        // Default handler denies
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: Some("call-123".to_string()),
            extension_data: HashMap::new(),
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.kind.contains("denied"));

        // Register custom handler that approves
        session
            .register_permission_handler(|_req| PermissionRequestResult::approved())
            .await;

        let result = session.handle_permission_request(&request).await;
        assert_eq!(result.kind, "approved");
    }

    #[tokio::test]
    async fn test_denied_command_patterns_fullcommandtext() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        // Set denied patterns
        session
            .set_denied_command_patterns(vec![
                "git push".to_string(),
                "git commit".to_string(),
                "git add".to_string(),
            ])
            .await;

        // Register an approve-all handler (should NOT be reached for denied commands)
        session
            .register_permission_handler(|_req| PermissionRequestResult::approved())
            .await;

        // Test: git push should be denied
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("git push origin main"),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: Some("call-1".to_string()),
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_denied());

        // Test: git commit should be denied
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("git -c commit.gpgsign=false commit -m 'test'"),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_denied());

        // Test: git add should be denied
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("/usr/local/bin/git -C /path add ."),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_denied());

        // Test: git status should be ALLOWED (not in denied patterns)
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("git status"),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_approved());

        // Test: ls -la should be ALLOWED
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("ls -la"),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_approved());
    }

    #[tokio::test]
    async fn test_denied_command_patterns_identifier() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        session
            .set_denied_command_patterns(vec!["git push".to_string()])
            .await;

        session
            .register_permission_handler(|_req| PermissionRequestResult::approved())
            .await;

        // Test: commands[].identifier match
        let mut ext = HashMap::new();
        ext.insert(
            "commands".to_string(),
            serde_json::json!([{"identifier": "git push"}]),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_denied());
    }

    #[tokio::test]
    async fn test_denied_command_patterns_word_boundary() {
        let session = Session::new("test".to_string(), None, mock_invoke);

        session
            .set_denied_command_patterns(vec!["rm".to_string()])
            .await;

        session
            .register_permission_handler(|_req| PermissionRequestResult::approved())
            .await;

        // "rm -rf /" should be denied
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("rm -rf /"),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_denied());

        // "form" should NOT be denied (rm is a substring but not a word)
        let mut ext = HashMap::new();
        ext.insert(
            "fullCommandText".to_string(),
            serde_json::json!("form submit"),
        );
        let request = PermissionRequest {
            kind: "tool_execution".to_string(),
            tool_call_id: None,
            extension_data: ext,
        };
        let result = session.handle_permission_request(&request).await;
        assert!(result.is_approved());
    }

    #[test]
    fn test_matches_denied_pattern() {
        // Basic match
        assert!(Session::matches_denied_pattern("git push origin", &["git push".to_string()]).is_some());

        // Case insensitive
        assert!(Session::matches_denied_pattern("GIT PUSH", &["git push".to_string()]).is_some());

        // No match
        assert!(Session::matches_denied_pattern("git status", &["git push".to_string()]).is_none());

        // Word boundary: "rm" should not match "form"
        assert!(Session::matches_denied_pattern("form submit", &["rm".to_string()]).is_none());

        // Word boundary: "rm" should match "rm -rf"
        assert!(Session::matches_denied_pattern("rm -rf /", &["rm".to_string()]).is_some());

        // Match at end of string
        assert!(Session::matches_denied_pattern("git add", &["git add".to_string()]).is_some());

        // Match with path prefix
        assert!(Session::matches_denied_pattern(
            "/usr/local/bin/git -C /tmp add .",
            &["git".to_string(), "add".to_string()]
        ).is_some());
    }

    #[tokio::test]
    async fn test_get_messages_with_events_field() {
        let session = Session::new("test".to_string(), None, mock_invoke_with_events);
        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            messages[0].data,
            crate::events::SessionEventData::SessionIdle(_)
        ));
    }
}
