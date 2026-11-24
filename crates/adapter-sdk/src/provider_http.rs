use crate::{
    text_stream, Adapter, AdapterError, EventStream, FinishReason, NormalizedRequest,
    NormalizedResponse, NormalizedStream, ResponseMetadata, StreamEvent, TokenUsage,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use localmux_domain::{ChatCapabilities, ParameterCapability, Support};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use std::time::Duration;
use std::{collections::HashMap, sync::RwLock};

const MAX_CATALOG_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_RESPONSE_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_RESPONSE_TEXT_BYTES: usize = 8 * 1024 * 1024;
async fn bounded_json(response: reqwest::Response, limit: usize) -> Result<Value, AdapterError> {
    if response
        .content_length()
        .is_some_and(|len| len > limit as u64)
    {
        return Err(AdapterError::OutputTooLarge);
    }
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| AdapterError::ProviderUnavailable)?;
        if chunk.len() > limit.saturating_sub(buffer.len()) {
            return Err(AdapterError::OutputTooLarge);
        }
        buffer.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&buffer).map_err(|_| AdapterError::ProcessFailed)
}

pub struct HttpProvider {
    id: &'static str,
    base: String,
    key: SecretString,
    client: reqwest::Client,
    capabilities: RwLock<HashMap<String, ChatCapabilities>>,
}
impl HttpProvider {
    pub fn new(
        id: &'static str,
        base: impl Into<String>,
        key: SecretString,
    ) -> Result<Self, AdapterError> {
        let base = base.into();
        let url = reqwest::Url::parse(&base).map_err(|_| AdapterError::Unsupported)?;
        if url.scheme() != "https"
            && !(url.scheme() == "http"
                && url
                    .host_str()
                    .is_some_and(|host| host == "127.0.0.1" || host == "localhost"))
        {
            return Err(AdapterError::Unsupported);
        }
        if url.username() != ""
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(AdapterError::Unsupported);
        }
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none());
        if matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
            builder = builder.no_proxy();
        }
        let client = builder
            .build()
            .map_err(|_| AdapterError::ProviderUnavailable)?;
        Ok(Self {
            id,
            base: base.trim_end_matches('/').into(),
            key,
            client,
            capabilities: RwLock::new(HashMap::new()),
        })
    }
    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base, path)
    }
    fn request_body(
        &self,
        request: &NormalizedRequest,
        stream: bool,
    ) -> Result<Value, AdapterError> {
        let model = request
            .model
            .strip_prefix(&format!("{}/", self.id))
            .ok_or(AdapterError::ModelUnavailable)?;
        let capabilities = self
            .capabilities
            .read()
            .map_err(|_| AdapterError::ProcessFailed)?
            .get(&request.model)
            .cloned()
            .unwrap_or_else(ChatCapabilities::http_text);
        request.parameters.validate(&capabilities)?;
        let mut body = json!({"model":model,"messages":request.chat_messages(),"stream":stream});
        let parameters =
            serde_json::to_value(&request.parameters).map_err(|_| AdapterError::ProcessFailed)?;
        if let Some(values) = parameters.as_object() {
            body.as_object_mut()
                .ok_or(AdapterError::ProcessFailed)?
                .extend(values.clone());
        }
        Ok(body)
    }
    async fn response(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, AdapterError> {
        let response = request
            .bearer_auth(self.key.expose_secret())
            .send()
            .await
            .map_err(|_| AdapterError::ProviderUnavailable)?;
        match response.status().as_u16() {
            200..=299 => Ok(response),
            401 | 403 => Err(AdapterError::AuthRequired),
            400 | 422 => Err(crate::parameter_error(
                "request",
                "upstream_validation",
                "public_interface",
            )),
            413 => Err(AdapterError::ContextLimit),
            404 => Err(AdapterError::ModelUnavailable),
            429 => Err(AdapterError::RateLimited),
            500..=599 => Err(AdapterError::ProviderUnavailable),
            _ => Err(AdapterError::ProcessFailed),
        }
    }
}
#[async_trait]
impl Adapter for HttpProvider {
    fn id(&self) -> &'static str {
        self.id
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Provider
    }
    async fn health(&self) -> HealthStatus {
        HealthStatus::Degraded
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let response = self
            .response(self.client.get(self.endpoint("models")))
            .await?;
        let body = bounded_json(response, MAX_CATALOG_JSON_BYTES).await?;
        let data = body["data"].as_array().ok_or(AdapterError::Unsupported)?;
        // Official MiMo SDK defines GET /provider model.temperature and model.limit.output.
        // Only read this public loopback metadata after an explicitly connected catalog request;
        // never send the scoped inference token to a different API surface.
        let metadata = if self.id == "mimocode" && self.base.starts_with("http://127.0.0.1:") {
            let endpoint = format!("{}/provider", self.base.trim_end_matches("/v1"));
            match self
                .client
                .get(endpoint)
                .timeout(Duration::from_secs(10))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    bounded_json(response, MAX_CATALOG_JSON_BYTES).await.ok()
                }
                _ => None,
            }
        } else {
            None
        };
        let models: Vec<_> = data
            .iter()
            .filter_map(|item| {
                let id = item["id"].as_str()?;
                if id.is_empty() {
                    return None;
                }
                Some(ModelDescriptor {
                    id: format!("{}/{id}", self.id),
                    provider_id: self.id.into(),
                    connection_id: None,
                    cost_mode: CostMode::Unknown,
                    supports_streaming: true,
                    supports_session: false,
                    chat: http_capabilities(id, item, metadata.as_ref()),
                })
            })
            .collect();
        if models.is_empty() {
            Err(AdapterError::Unsupported)
        } else {
            let mut capabilities = self
                .capabilities
                .write()
                .map_err(|_| AdapterError::ProcessFailed)?;
            *capabilities = models
                .iter()
                .map(|model| (model.id.clone(), model.chat.clone()))
                .collect();
            Ok(models)
        }
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        let body = self.request_body(&request, false)?;
        let response = self
            .response(
                self.client
                    .post(self.endpoint("chat/completions"))
                    .json(&body),
            )
            .await?;
        let value = bounded_json(response, MAX_RESPONSE_JSON_BYTES).await?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or(AdapterError::ProcessFailed)?;
        if text.len() > MAX_RESPONSE_TEXT_BYTES {
            return Err(AdapterError::OutputTooLarge);
        }
        Ok(NormalizedResponse {
            text: text.to_owned(),
            provider_session_id: None,
            metadata: http_metadata(&value),
        })
    }
    async fn invoke_events(&self, request: NormalizedRequest) -> Result<EventStream, AdapterError> {
        let body = self.request_body(&request, true)?;
        let response = self
            .response(
                self.client
                    .post(self.endpoint("chat/completions"))
                    .json(&body),
            )
            .await?;
        let mut bytes = response.bytes_stream();
        Ok(Box::pin(async_stream::try_stream! {
            let mut buffer = Vec::new();
            let mut metadata = ResponseMetadata::default();
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|_| AdapterError::ProviderUnavailable)?;
                if chunk.len() > (1024_usize * 1024).saturating_sub(buffer.len()) { Err(AdapterError::OutputTooLarge)?; }
                buffer.extend_from_slice(&chunk);
                while let Some((end, separator_len)) = frame_end(&buffer) {
                    let frame: Vec<u8> = buffer.drain(..end + separator_len).collect();
                    let text = std::str::from_utf8(&frame).map_err(|_| AdapterError::ProcessFailed)?;
                    for line in text.lines() {
                        let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue; };
                        if data == "[DONE]" { yield StreamEvent::Completed(metadata); return; }
                        let value: Value = serde_json::from_str(data).map_err(|_| AdapterError::ProcessFailed)?;
                        if value.get("error").is_some() { Err(AdapterError::ProcessFailed)?; }
                        if let Some(usage) = http_usage(&value["usage"]) { metadata.usage = Some(usage.clone()); yield StreamEvent::Usage(usage); }
                        if let Some(model) = value["model"].as_str() { metadata.actual_model = Some(model.to_owned()); metadata.actual_model_source = Some("provider_reported".into()); }
                        if let Some(reason) = value["choices"][0]["finish_reason"].as_str() { metadata.finish_reason = FinishReason::from_public(Some(reason)); metadata.finish_source = Some("provider_reported".into()); }
                        if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() { yield StreamEvent::Delta(delta.to_owned()); }
                    }
                }
            }
            Err(AdapterError::ProcessFailed)?;
        }))
    }
    async fn invoke_session_events(
        &self,
        request: NormalizedRequest,
        session: Option<&str>,
    ) -> Result<crate::SessionEventStream, AdapterError> {
        if session.is_some() {
            return Err(AdapterError::Unsupported);
        }
        Ok(crate::SessionEventStream {
            stream: self.invoke_events(request).await?,
            provider_session_id: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        Ok(text_stream(self.invoke_events(request).await?))
    }
}

fn http_metadata(value: &Value) -> ResponseMetadata {
    ResponseMetadata {
        finish_reason: FinishReason::from_public(value["choices"][0]["finish_reason"].as_str()),
        usage: http_usage(&value["usage"]),
        actual_model: value["model"].as_str().map(str::to_owned),
        actual_model_source: Some("provider_reported".into()),
        finish_source: Some("provider_reported".into()),
        ..Default::default()
    }
}
fn http_usage(value: &Value) -> Option<TokenUsage> {
    let input = value["prompt_tokens"].as_u64();
    let output = value["completion_tokens"].as_u64();
    if input.is_none() && output.is_none() {
        return None;
    }
    Some(TokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: value["total_tokens"].as_u64(),
        reasoning_tokens: value["completion_tokens_details"]["reasoning_tokens"].as_u64(),
        cached_input_tokens: value["prompt_tokens_details"]["cached_tokens"].as_u64(),
        source: "provider_reported".into(),
        scope: "request".into(),
    })
}
fn http_capabilities(id: &str, item: &Value, registry: Option<&Value>) -> ChatCapabilities {
    let reported = id
        .split_once('/')
        .and_then(|(provider, model)| {
            registry?["all"]
                .as_array()?
                .iter()
                .find(|entry| entry["id"] == provider)?["models"]
                .get(model)
        })
        .unwrap_or(item);
    let mut capabilities = ChatCapabilities::http_text();
    let temperature = reported["temperature"]
        .as_bool()
        .or_else(|| reported["capabilities"]["temperature"].as_bool());
    if let Some(temperature) = temperature {
        capabilities.temperature = if temperature {
            ParameterCapability::range(0.0, 2.0)
        } else {
            ParameterCapability {
                support: Support::Unsupported,
                ..Default::default()
            }
        };
        capabilities.source = "公开模型元数据；真实参数效果尚未实测".into();
    }
    if reported["capabilities"]["top_p"] == true {
        capabilities.top_p = ParameterCapability::range(0.0, 1.0);
    }
    if let Some(max) = reported["limit"]["output"]
        .as_u64()
        .filter(|value| *value > 0 && *value <= u64::from(u32::MAX))
    {
        capabilities.max_tokens = ParameterCapability::range(1.0, max as f64);
        capabilities.source = "公开模型元数据；真实参数效果尚未实测".into();
    }
    capabilities.describe(None, "openai_chat_http");
    capabilities.describe_session_resume(Support::Unsupported);
    capabilities
}
fn frame_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| (position, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 < right.0 { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;
    #[test]
    fn public_model_metadata_is_exactly_scoped_and_missing_fields_stay_unknown() {
        let registry = json!({"all":[{"id":"mimo","models":{"model":{"temperature":true,"limit":{"output":8192}}}}]});
        let capabilities = http_capabilities("mimo/model", &json!({}), Some(&registry));
        assert_eq!(capabilities.temperature.support, Support::Supported);
        assert_eq!(capabilities.max_tokens.max, Some(8192.0));
        assert_eq!(capabilities.top_p.support, Support::Unknown);
        let other = http_capabilities("xiaomi/model", &json!({}), Some(&registry));
        assert_eq!(other.temperature.support, Support::Unknown);
        let disabled = http_capabilities(
            "mimo/model",
            &json!({"temperature":false,"limit":{"output":0}}),
            None,
        );
        assert_eq!(disabled.temperature.support, Support::Unsupported);
        assert_eq!(disabled.max_tokens.support, Support::Unknown);
    }
}

#[cfg(test)]
mod body_limit_tests {
    use super::*;
    use axum::{
        body::{Body, Bytes},
        http::Response,
        routing::{get, post},
        Json, Router,
    };
    use std::future::IntoFuture;
    #[tokio::test]
    async fn bounded_json_rejects_declared_and_chunked_oversize_before_copying() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(
            axum::serve(
                listener,
                Router::new()
                    .route("/small", get(|| async { "{}" }))
                    .route("/declared", get(|| async { "x".repeat(2048) }))
                    .route(
                        "/chunked",
                        get(|| async {
                            Response::builder()
                                .body(Body::from_stream(futures_util::stream::iter([Ok::<
                                    _,
                                    std::io::Error,
                                >(
                                    Bytes::from(vec![b'x'; 2048]),
                                )])))
                                .unwrap()
                        }),
                    ),
            )
            .into_future(),
        );
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        assert_eq!(
            bounded_json(client.get(format!("{base}/small")).send().await.unwrap(), 4)
                .await
                .unwrap(),
            serde_json::json!({})
        );
        for route in ["declared", "chunked"] {
            assert!(matches!(
                bounded_json(
                    client.get(format!("{base}/{route}")).send().await.unwrap(),
                    1024
                )
                .await,
                Err(AdapterError::OutputTooLarge)
            ));
        }
        server.abort();
    }
    #[tokio::test]
    async fn http_nonstream_text_cannot_bypass_the_gateway_output_limit() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(axum::serve(listener, Router::new()
            .route("/models", get(|| async { Json(serde_json::json!({"data":[{"id":"model"}]})) }))
            .route("/chat/completions", post(|| async { Json(serde_json::json!({"choices":[{"message":{"content":"x".repeat(MAX_RESPONSE_TEXT_BYTES+1)}}]})) }))
        ).into_future());
        let provider =
            HttpProvider::new("fixture", base, SecretString::from("synthetic".to_string()))
                .unwrap();
        provider.list_models().await.unwrap();
        let result = provider
            .invoke(NormalizedRequest {
                cli_idle_timeout: crate::cli::CLI_IDLE_TIMEOUT,
                model: "fixture/model".into(),
                prompt: "fixture".into(),
                messages: vec![],
                parameters: Default::default(),
            })
            .await;
        assert!(matches!(result, Err(AdapterError::OutputTooLarge)));
        server.abort();
    }
}
