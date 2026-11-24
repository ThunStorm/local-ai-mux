use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
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
    pub response_format: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StreamOptions {
    #[serde(default)]
    pub include_usage: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct ModelList {
    pub object: &'static str,
    pub data: Vec<ModelObject>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ModelObject {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub owned_by: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
    pub localmux: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: AssistantMessage,
    pub finish_reason: Option<&'static str>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AssistantMessage {
    pub role: &'static str,
    pub content: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}
#[derive(Clone, Debug, Serialize)]
pub struct PromptTokensDetails {
    pub cached_tokens: u64,
}

pub fn text_content(value: &Value) -> Option<&str> {
    value.as_str()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Value,
    #[serde(default)]
    pub stream: bool,
    pub previous_response_id: Option<String>,
    pub store: Option<bool>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}
pub fn response_input_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_owned());
    }
    let items = value.as_array()?;
    if items.len() != 1 || items[0]["role"] != "user" {
        return None;
    }
    let user = &items[0];
    if let Some(text) = user["content"].as_str() {
        return Some(text.to_owned());
    }
    let parts = user["content"].as_array()?;
    let text: Vec<&str> = parts
        .iter()
        .map(|part| {
            if part["type"] == "input_text" {
                part["text"].as_str()
            } else {
                None
            }
        })
        .collect::<Option<_>>()?;
    Some(text.join("\n"))
}
#[cfg(test)]
mod responses_tests {
    #[test]
    fn accepts_only_text_input() {
        assert_eq!(
            super::response_input_text(&serde_json::json!("hello")),
            Some("hello".into())
        );
        assert_eq!(
            super::response_input_text(
                &serde_json::json!([{"role":"user","content":[{"type":"input_text","text":"hello"}]}])
            ),
            Some("hello".into())
        );
        assert!(super::response_input_text(
            &serde_json::json!([{"role":"user","content":[{"type":"input_image","image_url":"x"}]}])
        )
        .is_none());
        assert!(super::response_input_text(
            &serde_json::json!([{"role":"user","content":"first"},{"role":"assistant","content":"answer"},{"role":"user","content":"next"}])
        )
        .is_none());
    }
}
