//! OpenAI-compatible HTTP model adapter.
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.2。
//!
//! v1 范围：
//! - `complete()`：非流式 POST 到 `/v1/chat/completions`，单次返回完整 Decision。
//! - `stream()`：默认实现调用 `complete()` 后包成单事件流；Phase 4+ 评估升级为真正的 SSE。
//!
//! 不在 v1 范围（明确不做）：
//! - 多 provider 路由（Anthropic / Gemini 等）；其它 provider 由 trait 实现变体处理。
//! - 自动重试、退避、熔断——调用方负责。
//! - 工具调用协议以外的 provider-specific 扩展（OpenAI 兼容即可）。

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

use crate::domain::{Decision, Message, ToolArgument, ToolCall};
use crate::error::AgentError;
use crate::model::{ModelGateway, ModelRequest, ModelResponse, ModelStream};
use crate::tool::ToolSchema;

/// OpenAI-compatible HTTP 适配器配置。
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// API endpoint（不含路径），如 `https://api.openai.com`。
    pub endpoint: String,
    /// 完整路径，默认为 `/v1/chat/completions`。
    pub path: String,
    /// 模型名。
    pub model: String,
    /// API key。
    pub api_key: String,
}

impl HttpConfig {
    /// 构造默认配置（路径 `/v1/chat/completions`）。
    #[must_use]
    pub fn new(endpoint: impl Into<String>, model: String, api_key: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            path: "/v1/chat/completions".to_string(),
            model,
            api_key: api_key.into(),
        }
    }

    /// 自定义路径。
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    fn url(&self) -> String {
        format!(
            "{}/{}",
            self.endpoint.trim_end_matches('/'),
            self.path.trim_start_matches('/')
        )
    }
}

/// HTTP model adapter。`Send + Sync` 是因为 trait 上限要求。
#[derive(Clone)]
pub struct HttpModelAdapter {
    config: HttpConfig,
    client: reqwest::Client,
}

impl HttpModelAdapter {
    /// 用默认 `reqwest::Client` 构造。
    #[must_use]
    pub fn new(config: HttpConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 注入自定义 `reqwest::Client`（测试用）。
    #[must_use]
    pub fn with_client(config: HttpConfig, client: reqwest::Client) -> Self {
        Self { config, client }
    }
}

// OpenAI wire format
#[derive(Debug, Serialize)]
struct WireRequest {
    model: String,
    messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WireTool>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct WireMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<WireToolCall>,
}

#[derive(Debug, Serialize)]
struct WireTool {
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireToolFunction,
}

#[derive(Debug, Serialize)]
struct WireToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireToolCallFunction,
}

#[derive(Debug, Serialize)]
struct WireToolCallFunction {
    name: String,
    arguments: String, // JSON string
}

#[derive(Debug, Deserialize)]
struct WireResponse {
    choices: Vec<WireChoice>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WireChoice {
    message: WireResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WireResponseMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<WireResponseToolCall>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WireResponseToolCall {
    id: String,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    function: WireResponseFunction,
}

#[derive(Debug, Deserialize)]
struct WireResponseFunction {
    name: String,
    /// JSON 字符串，调用方解析。
    arguments: String,
}

fn convert_messages(messages: &[Message]) -> Vec<WireMessage> {
    messages
        .iter()
        .map(|m| match m {
            Message::System { content } => WireMessage {
                role: "system".into(),
                content: Some(content.clone()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            Message::User { content } => WireMessage {
                role: "user".into(),
                content: Some(content.clone()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            Message::Assistant { content } => WireMessage {
                role: "assistant".into(),
                content: Some(content.clone()),
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            Message::Tool {
                call_id,
                output,
                success: _,
            } => WireMessage {
                role: "tool".into(),
                content: Some(output.clone()),
                tool_call_id: Some(call_id.clone()),
                tool_calls: Vec::new(),
            },
        })
        .collect()
}

fn convert_schemas(schemas: &[ToolSchema]) -> Vec<WireTool> {
    schemas
        .iter()
        .map(|s| WireTool {
            kind: "function",
            function: WireToolFunction {
                name: s.name.clone(),
                description: s.description.clone(),
                parameters: s.parameters.clone(),
            },
        })
        .collect()
}

fn parse_decision(message: &WireResponseMessage) -> Result<Decision, AgentError> {
    if let Some(first) = message.tool_calls.first() {
        let args: serde_json::Value = serde_json::from_str(&first.function.arguments)
            .map_err(|e| AgentError::UnparseableDecision(format!("tool args not JSON: {e}")))?;
        let call = ToolCall::new(
            first.id.clone(),
            first.function.name.clone(),
            ToolArgument::new(args),
        )?;
        return Ok(Decision::Call(call));
    }
    let content = message.content.clone().unwrap_or_default();
    Ok(Decision::Done { summary: content })
}

impl ModelGateway for HttpModelAdapter {
    fn complete(
        &self,
        request: ModelRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ModelResponse, AgentError>> + Send + '_>> {
        let url = self.config.url();
        let model = self.config.model.clone();
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();

        let wire = WireRequest {
            model,
            messages: convert_messages(&request.messages),
            tools: convert_schemas(&request.tool_schemas),
            stream: false,
        };

        Box::pin(async move {
            let resp = client
                .post(&url)
                .bearer_auth(api_key)
                .json(&wire)
                .send()
                .await
                .map_err(|e| AgentError::Context(format!("HTTP request failed: {e}")))?;

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(AgentError::Context(format!("HTTP {status}: {body}")));
            }

            let wire_resp: WireResponse = resp
                .json()
                .await
                .map_err(|e| AgentError::UnparseableDecision(format!("response not JSON: {e}")))?;

            let choice = wire_resp
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| AgentError::UnparseableDecision("empty choices".into()))?;

            let decision = parse_decision(&choice.message)?;
            let raw = format!("{decision:?}");
            Ok(ModelResponse { decision, raw })
        })
    }

    #[allow(clippy::manual_async_fn)]
    fn stream(
        &self,
        request: ModelRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ModelStream, AgentError>> + Send + '_>> {
        Box::pin(async move {
            let resp = self.complete(request).await?;
            Ok(ModelStream::from_response(resp))
        })
    }
}
