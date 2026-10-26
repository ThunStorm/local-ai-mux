pub mod cli;
pub mod process;
pub mod provider_http;
use async_trait::async_trait;
use futures_util::{stream, Stream, StreamExt};
use localmux_domain::{AdapterKind, HealthStatus, ModelDescriptor};
use localmux_quota::{QuotaBucket, QuotaSnapshot};
use serde::Serialize;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct NormalizedRequest {
    /// Local execution snapshot, never forwarded as a provider parameter.
    pub cli_idle_timeout: std::time::Duration,
    pub model: String,
    pub prompt: String,
    pub messages: Vec<NormalizedMessage>,
    pub parameters: ChatParameters,
}
#[derive(Clone, Debug, Default, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}
#[derive(Clone, Debug, serde::Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResponseFormat {
    Text,
    JsonObject,
    JsonSchema { json_schema: JsonSchemaFormat },
}
#[derive(Clone, Debug, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JsonSchemaFormat {
    pub name: String,
    pub schema: serde_json::Value,
    pub strict: Option<bool>,
}
impl ChatParameters {
    pub fn validate(
        &self,
        capabilities: &localmux_domain::ChatCapabilities,
    ) -> Result<(), AdapterError> {
        if self.max_tokens.is_some() && self.max_completion_tokens.is_some() {
            return Err(parameter_error(
                "max_completion_tokens",
                "conflict",
                "gateway",
            ));
        }
        for (name, value, capability) in [
            ("temperature", self.temperature, &capabilities.temperature),
            ("top_p", self.top_p, &capabilities.top_p),
            (
                "max_tokens",
                self.max_tokens.map(f64::from),
                &capabilities.max_tokens,
            ),
            (
                "max_completion_tokens",
                self.max_completion_tokens.map(f64::from),
                &capabilities.max_completion_tokens,
            ),
        ] {
            if let Some(value) = value {
                if !capability.accepts(value) {
                    let detail = capabilities.details.get(name);
                    let reason = if capability.support == localmux_domain::Support::Supported {
                        "out_of_range"
                    } else {
                        detail
                            .and_then(|d| d.reason_code.as_deref())
                            .unwrap_or("unknown_constraints")
                    };
                    return Err(parameter_error(
                        name,
                        reason,
                        detail
                            .and_then(|d| d.limiting_layer.as_deref())
                            .unwrap_or("adapter"),
                    ));
                }
            }
        }
        if let Some(effort) = &self.reasoning_effort {
            if !capabilities.reasoning_efforts.contains(effort) {
                return Err(parameter_error(
                    "reasoning_effort",
                    if capabilities.reasoning_efforts.is_empty() {
                        "unknown_constraints"
                    } else {
                        "out_of_range"
                    },
                    "adapter",
                ));
            }
        }
        if self
            .response_format
            .as_ref()
            .is_some_and(|format| !matches!(format, ResponseFormat::Text))
        {
            return Err(parameter_error(
                "response_format",
                "not_implemented",
                "adapter",
            ));
        }
        Ok(())
    }
}
pub fn parameter_error(param: &str, reason: &str, layer: &str) -> AdapterError {
    AdapterError::InvalidParameter {
        param: param.into(),
        reason: reason.into(),
        layer: layer.into(),
    }
}
#[derive(Clone, Debug, Default)]
pub struct TextInstructions {
    pub system: Option<String>,
    pub developer: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NormalizedMessage {
    pub role: String,
    pub content: String,
}
impl NormalizedRequest {
    pub fn single_user_prompt(&self) -> Result<&str, AdapterError> {
        self.parameters
            .validate(&localmux_domain::ChatCapabilities::single_turn())?;
        if self.messages.is_empty()
            || (self.messages.len() == 1
                && self.messages[0].role == "user"
                && self.messages[0].content == self.prompt)
        {
            Ok(&self.prompt)
        } else {
            Err(AdapterError::UnsupportedMessages)
        }
    }
    /// Only leading instructions plus one user are normalized. History is never flattened.
    pub fn text_instructions(
        &self,
        developer_supported: bool,
    ) -> Result<TextInstructions, AdapterError> {
        let messages = self.chat_messages();
        if messages
            .last()
            .is_none_or(|m| m.role != "user" || m.content != self.prompt)
        {
            return Err(AdapterError::UnsupportedMessages);
        }
        let mut result = TextInstructions::default();
        let mut saw_developer = false;
        for message in &messages[..messages.len() - 1] {
            let target = match message.role.as_str() {
                "system" if !saw_developer => &mut result.system,
                "developer" if developer_supported => {
                    saw_developer = true;
                    &mut result.developer
                }
                _ => return Err(AdapterError::UnsupportedMessages),
            };
            if let Some(text) = target {
                // A visible separator preserves same-role instruction order without inventing roles.
                text.push_str("\n\n--- next instruction ---\n\n");
                text.push_str(&message.content);
            } else {
                *target = Some(message.content.clone());
            }
        }
        Ok(result)
    }
    pub fn chat_messages(&self) -> Vec<NormalizedMessage> {
        if self.messages.is_empty() {
            vec![NormalizedMessage {
                role: "user".into(),
                content: self.prompt.clone(),
            }]
        } else {
            self.messages.clone()
        }
    }
}
#[derive(Clone, Debug)]
pub struct NormalizedResponse {
    pub text: String,
    pub provider_session_id: Option<String>,
    pub metadata: ResponseMetadata,
}
pub type NormalizedStream = Pin<Box<dyn Stream<Item = Result<String, AdapterError>> + Send>>;
pub struct SessionStream {
    pub stream: NormalizedStream,
    pub provider_session_id: Arc<Mutex<Option<String>>>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    Cancelled,
    #[default]
    Unknown,
}
impl FinishReason {
    pub fn openai(self) -> Option<&'static str> {
        match self {
            Self::Stop => Some("stop"),
            Self::Length => Some("length"),
            Self::ContentFilter => Some("content_filter"),
            Self::Cancelled | Self::Unknown => None,
        }
    }
    pub fn from_public(value: Option<&str>) -> Self {
        match value {
            Some("stop" | "end_turn") => Self::Stop,
            Some("length" | "max_tokens") => Self::Length,
            Some("content_filter" | "refusal") => Self::ContentFilter,
            Some("cancelled" | "interrupted") => Self::Cancelled,
            _ => Self::Unknown,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub source: String,
    pub scope: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseMetadata {
    pub finish_reason: FinishReason,
    pub usage: Option<TokenUsage>,
    pub actual_model: Option<String>,
    pub normalization: Option<String>,
    pub actual_model_source: Option<String>,
    pub finish_source: Option<String>,
}
impl TokenUsage {
    pub fn validated(mut self) -> Option<Self> {
        // Cached tokens are a subset of input, never an additional token total.
        self.cached_input_tokens = self
            .cached_input_tokens
            .filter(|cached| self.input_tokens.is_some_and(|input| *cached <= input));
        (self.source == "provider_reported"
            && matches!(self.scope.as_str(), "request" | "last_turn"))
        .then_some(self)
    }
}
/// Whitelisted provenance and bounded identifiers, never raw CLI events or model self-report prose.
impl ResponseMetadata {
    pub fn validated(mut self) -> Self {
        self.usage = self.usage.and_then(TokenUsage::validated);
        if self.finish_source.as_deref() != Some("provider_reported") {
            self.finish_reason = FinishReason::Unknown;
            self.finish_source = None;
        }
        if self.finish_reason == FinishReason::Unknown {
            self.finish_source = None;
        }
        if !matches!(
            self.actual_model_source.as_deref(),
            Some("provider_reported" | "app_server_thread" | "app_server_rerouted")
        ) || !self.actual_model.as_deref().is_some_and(public_model_id)
        {
            self.actual_model = None;
            self.actual_model_source = None;
        }
        self.normalization = self
            .normalization
            .filter(|v| v == "leading_instructions_v1");
        self
    }
}
pub fn public_model_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && !value.starts_with("lmx_")
        && !value.starts_with("sk-")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_.:-".contains(&b))
}
#[derive(Clone, Debug)]
pub enum StreamEvent {
    Delta(String),
    Usage(TokenUsage),
    Completed(ResponseMetadata),
}
pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, AdapterError>> + Send>>;
pub struct SessionEventStream {
    pub stream: EventStream,
    pub provider_session_id: Arc<Mutex<Option<String>>>,
}
pub fn text_stream(mut events: EventStream) -> NormalizedStream {
    Box::pin(async_stream::try_stream! {
        while let Some(event) = events.next().await {
            if let StreamEvent::Delta(text) = event? { yield text; }
        }
    })
}
// Compatibility for third-party text-only Adapters: EOF says nothing about the upstream finish reason.
pub fn legacy_events(mut text: NormalizedStream) -> EventStream {
    Box::pin(async_stream::try_stream! {
        while let Some(delta) = text.next().await { yield StreamEvent::Delta(delta?); }
        yield StreamEvent::Completed(ResponseMetadata::default());
    })
}
pub async fn collect_events(mut events: EventStream) -> Result<NormalizedResponse, AdapterError> {
    let mut response = NormalizedResponse {
        text: String::new(),
        provider_session_id: None,
        metadata: ResponseMetadata::default(),
    };
    let mut completed = false;
    while let Some(event) = events.next().await {
        if completed {
            return Err(AdapterError::ProcessFailed);
        }
        match event? {
            StreamEvent::Delta(text) => {
                if completed || response.text.len() + text.len() > 8 * 1024 * 1024 {
                    return Err(AdapterError::ProcessFailed);
                }
                response.text.push_str(&text);
            }
            StreamEvent::Usage(usage) => {
                if let Some(usage) = usage.validated() {
                    response.metadata.usage = Some(usage);
                }
            }
            StreamEvent::Completed(mut metadata) => {
                if completed {
                    return Err(AdapterError::ProcessFailed);
                }
                metadata = metadata.validated();
                metadata.usage = metadata.usage.or(response.metadata.usage.take());
                response.metadata = metadata;
                completed = true;
            }
        }
    }
    if !completed {
        return Err(AdapterError::ProcessFailed);
    }
    Ok(response)
}
#[derive(Clone, Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("authentication required")]
    AuthRequired,
    #[error("provider unavailable")]
    ProviderUnavailable,
    #[error("model unavailable")]
    ModelUnavailable,
    #[error("quota exhausted")]
    QuotaExhausted,
    #[error("rate limited")]
    RateLimited,
    #[error("adapter unsupported")]
    Unsupported,
    #[error("message roles or history unsupported")]
    UnsupportedMessages,
    #[error("parameter unsupported or outside reported range")]
    UnsupportedParameter,
    #[error("invalid parameter: {param} ({reason}, {layer})")]
    InvalidParameter {
        param: String,
        reason: String,
        layer: String,
    },
    #[error("input exceeds the upstream context limit")]
    ContextLimit,
    #[error("upstream refused this request")]
    Refusal,
    #[error("provider output exceeds the local size limit")]
    OutputTooLarge,
    #[error("provider request failed")]
    ProcessFailed,
    #[error("provider request timed out")]
    Timeout,
    #[error("local timeout at {layer:?}")]
    TimeoutAt {
        layer: TimeoutLayer,
        limit: std::time::Duration,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum TimeoutLayer {
    CliIdle,
    CliRpc,
    CliCommand,
    CliExit,
    GatewayStartup,
    GatewayStreamIdle,
    GatewayOverall,
    ProviderHttp,
}
impl TimeoutLayer {
    pub fn code(self) -> &'static str {
        match self {
            Self::CliIdle => "cli_idle",
            Self::CliRpc => "cli_rpc",
            Self::CliCommand => "cli_command",
            Self::CliExit => "cli_exit",
            Self::GatewayStartup => "gateway_startup",
            Self::GatewayStreamIdle => "gateway_stream_idle",
            Self::GatewayOverall => "gateway_overall",
            Self::ProviderHttp => "provider_http",
        }
    }
    pub fn limiting_layer(self) -> &'static str {
        match self {
            Self::GatewayStartup | Self::GatewayStreamIdle | Self::GatewayOverall => "gateway",
            _ => "adapter",
        }
    }
}

#[async_trait]
pub trait Adapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> AdapterKind;
    async fn health(&self) -> HealthStatus;
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError>;
    async fn get_quota(&self) -> Result<Option<QuotaSnapshot>, AdapterError> {
        Ok(None)
    }
    async fn get_quotas(&self) -> Result<Vec<QuotaBucket>, AdapterError> {
        Ok(self
            .get_quota()
            .await?
            .into_iter()
            .map(|snapshot| QuotaBucket {
                connection_id: snapshot.connection_id,
                bucket_id: "legacy".into(),
                label: "上游额度".into(),
                remaining: snapshot.remaining,
                limit: snapshot.limit,
                unit: snapshot.unit,
                reset_at: snapshot.reset_at,
                source: snapshot.source,
                observed_at: snapshot.observed_at,
            })
            .collect())
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError>;
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        let response = self.invoke(request).await?;
        Ok(Box::pin(stream::once(async move { Ok(response.text) })))
    }
    async fn invoke_events(&self, request: NormalizedRequest) -> Result<EventStream, AdapterError> {
        Ok(legacy_events(self.invoke_stream(request).await?))
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionEventStream, AdapterError> {
        let legacy = self.invoke_session_stream(request, session).await?;
        Ok(SessionEventStream {
            stream: legacy_events(legacy.stream),
            provider_session_id: legacy.provider_session_id,
        })
    }
    async fn resume(
        &self,
        _request: NormalizedRequest,
        _provider_session_id: &str,
    ) -> Result<NormalizedResponse, AdapterError> {
        Err(AdapterError::Unsupported)
    }
    async fn resume_stream(
        &self,
        _request: NormalizedRequest,
        _provider_session_id: &str,
    ) -> Result<NormalizedStream, AdapterError> {
        Err(AdapterError::Unsupported)
    }
    async fn invoke_session_stream(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<SessionStream, AdapterError> {
        if session.is_some() {
            return Err(AdapterError::Unsupported);
        }
        Ok(SessionStream {
            stream: self.invoke_stream(request).await?,
            provider_session_id: Arc::new(Mutex::new(None)),
        })
    }
}

pub struct MockEcho;
#[async_trait]
impl Adapter for MockEcho {
    fn id(&self) -> &'static str {
        "mock"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Mock
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Ok(vec![ModelDescriptor {
            id: "mock/echo".into(),
            provider_id: "mock".into(),
            connection_id: None,
            cost_mode: localmux_domain::CostMode::Unknown,
            supports_streaming: true,
            supports_session: false,
            chat: localmux_domain::ChatCapabilities::http_text(),
        }])
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        if request.model != "mock/echo" {
            return Err(AdapterError::ModelUnavailable);
        }
        Ok(NormalizedResponse {
            text: if request.messages.is_empty()
                || (request.messages.len() == 1 && request.messages[0].role == "user")
            {
                request.prompt
            } else {
                request
                    .messages
                    .iter()
                    .map(|message| format!("{}: {}", message.role, message.content))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            provider_session_id: None,
            metadata: ResponseMetadata::default(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::{AdapterError, NormalizedMessage, NormalizedRequest};

    #[test]
    fn agent_role_boundary_rejects_history() {
        let request = NormalizedRequest {
            cli_idle_timeout: crate::cli::CLI_IDLE_TIMEOUT,
            model: "codex/model".into(),
            prompt: "next".into(),
            messages: vec![
                NormalizedMessage {
                    role: "system".into(),
                    content: "rules".into(),
                },
                NormalizedMessage {
                    role: "user".into(),
                    content: "next".into(),
                },
            ],
            parameters: Default::default(),
        };
        assert!(matches!(
            request.single_user_prompt(),
            Err(AdapterError::UnsupportedMessages)
        ));
    }
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[tokio::test]
    async fn collector_retains_last_turn_cache_when_later_usage_is_cumulative() {
        let last = TokenUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            total_tokens: Some(14),
            cached_input_tokens: Some(3),
            source: "provider_reported".into(),
            scope: "last_turn".into(),
            ..Default::default()
        };
        let mut total = last.clone();
        total.scope = "session_total".into();
        total.total_tokens = Some(999);
        let response = collect_events(Box::pin(futures_util::stream::iter(vec![
            Ok(StreamEvent::Usage(last)),
            Ok(StreamEvent::Usage(total.clone())),
            Ok(StreamEvent::Completed(ResponseMetadata {
                usage: Some(total),
                ..Default::default()
            })),
        ])))
        .await
        .unwrap();
        let usage = response.metadata.usage.unwrap();
        assert_eq!(usage.total_tokens, Some(14));
        assert_eq!(usage.cached_input_tokens, Some(3));
    }

    #[test]
    fn unverified_cumulative_or_oversized_metadata_is_not_advertised_as_a_fact() {
        let metadata = ResponseMetadata {
            finish_reason: FinishReason::Stop,
            actual_model: Some("I am a better model".repeat(1000)),
            normalization: Some("Bearer PRIVATE".into()),
            usage: Some(TokenUsage {
                total_tokens: Some(999),
                source: "provider_reported".into(),
                scope: "session_total".into(),
                ..Default::default()
            }),
            ..Default::default()
        }
        .validated();
        assert_eq!(metadata.finish_reason, FinishReason::Unknown);
        assert!(
            metadata.actual_model.is_none()
                && metadata.usage.is_none()
                && metadata.normalization.is_none()
        );
        let metadata = ResponseMetadata {
            actual_model: Some("sk-secret".into()),
            actual_model_source: Some("provider_reported".into()),
            ..Default::default()
        }
        .validated();
        assert!(metadata.actual_model.is_none());
    }
}
