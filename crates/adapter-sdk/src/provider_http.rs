use crate::{Adapter, AdapterError, NormalizedRequest, NormalizedResponse, NormalizedStream};
use async_trait::async_trait;
use futures_util::StreamExt;
use localmux_domain::{AdapterKind, CostMode, HealthStatus, ModelDescriptor};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use std::time::Duration;

pub struct HttpProvider {
    id: &'static str,
    base: String,
    key: SecretString,
    client: reqwest::Client,
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AdapterError::ProviderUnavailable)?;
        Ok(Self {
            id,
            base: base.trim_end_matches('/').into(),
            key,
            client,
        })
    }
    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base, path)
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
        HealthStatus::Unknown
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        let response = self
            .response(self.client.get(self.endpoint("models")))
            .await?;
        let body: Value = response
            .json()
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        let data = body["data"].as_array().ok_or(AdapterError::Unsupported)?;
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
                })
            })
            .collect();
        if models.is_empty() {
            Err(AdapterError::Unsupported)
        } else {
            Ok(models)
        }
    }
    async fn invoke(&self, request: NormalizedRequest) -> Result<NormalizedResponse, AdapterError> {
        let model = request
            .model
            .strip_prefix(&format!("{}/", self.id))
            .ok_or(AdapterError::ModelUnavailable)?;
        let body = json!({"model":model,"messages":[{"role":"user","content":request.prompt}],"stream":false});
        let response = self
            .response(
                self.client
                    .post(self.endpoint("chat/completions"))
                    .json(&body),
            )
            .await?;
        let value: Value = response
            .json()
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or(AdapterError::ProcessFailed)?;
        Ok(NormalizedResponse {
            text: text.to_owned(),
            provider_session_id: None,
        })
    }
    async fn invoke_stream(
        &self,
        request: NormalizedRequest,
    ) -> Result<NormalizedStream, AdapterError> {
        let model = request
            .model
            .strip_prefix(&format!("{}/", self.id))
            .ok_or(AdapterError::ModelUnavailable)?;
        let body = json!({"model":model,"messages":[{"role":"user","content":request.prompt}],"stream":true});
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
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.map_err(|_| AdapterError::ProviderUnavailable)?;
                buffer.extend_from_slice(&chunk);
                if buffer.len() > 1024 * 1024 { Err(AdapterError::ProcessFailed)?; }
                while let Some((end, separator_len)) = frame_end(&buffer) {
                    let frame: Vec<u8> = buffer.drain(..end + separator_len).collect();
                    let text = std::str::from_utf8(&frame).map_err(|_| AdapterError::ProcessFailed)?;
                    for line in text.lines() {
                        let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue; };
                        if data == "[DONE]" { return; }
                        let value: Value = serde_json::from_str(data).map_err(|_| AdapterError::ProcessFailed)?;
                        if value.get("error").is_some() { Err(AdapterError::ProcessFailed)?; }
                        if let Some(delta) = value["choices"][0]["delta"]["content"].as_str() { yield delta.to_owned(); }
                    }
                }
            }
            Err(AdapterError::ProcessFailed)?;
        }))
    }
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
