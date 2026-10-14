use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Agent,
    Provider,
    Mock,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CostMode {
    Free,
    Subscription,
    Metered,
    Reserve,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    AuthRequired,
    RateLimited,
    QuotaExhausted,
    Offline,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub executable_path: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub provider_id: String,
    pub alias: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub provider_id: String,
    pub account_id: Option<String>,
    pub adapter_id: String,
    pub health: HealthStatus,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub provider_id: String,
    pub connection_id: Option<String>,
    pub cost_mode: CostMode,
    pub supports_streaming: bool,
    pub supports_session: bool,
    #[serde(default)]
    pub chat: ChatCapabilities,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Support {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParameterCapability {
    pub support: Support,
    pub min: f64,
    pub max: Option<f64>,
}
impl ParameterCapability {
    pub fn range(min: f64, max: f64) -> Self {
        Self {
            support: Support::Supported,
            min,
            max: Some(max),
        }
    }
    pub fn accepts(&self, value: f64) -> bool {
        self.support == Support::Supported
            && value.is_finite()
            && value >= self.min
            && self.max.is_some_and(|max| value <= max)
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChatCapabilities {
    pub history: Support,
    pub system: Support,
    pub temperature: ParameterCapability,
    pub top_p: ParameterCapability,
    pub max_tokens: ParameterCapability,
    pub source: String,
    #[serde(default)]
    pub developer: Support,
    #[serde(default)]
    pub max_completion_tokens: ParameterCapability,
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    #[serde(default)]
    pub default_reasoning_effort: Option<String>,
    #[serde(default)]
    pub native_input_modalities: Vec<String>,
    #[serde(default)]
    pub details: BTreeMap<String, CapabilityDetail>,
}
impl ChatCapabilities {
    pub fn single_turn() -> Self {
        Self {
            history: Support::Unsupported,
            system: Support::Unsupported,
            source: "官方 CLI/App Server 单轮文本桥".into(),
            temperature: ParameterCapability {
                support: Support::Unsupported,
                ..Default::default()
            },
            top_p: ParameterCapability {
                support: Support::Unsupported,
                ..Default::default()
            },
            max_tokens: ParameterCapability {
                support: Support::Unsupported,
                ..Default::default()
            },
            ..Self::default()
        }
    }
    pub fn http_text() -> Self {
        Self {
            history: Support::Supported,
            system: Support::Supported,
            source: "HTTP 文字桥；参数能力尚未报告".into(),
            ..Self::default()
        }
    }
}
/// Layered provenance is separate from the legacy effective capability fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapabilityDetail {
    pub effective_support: Support,
    pub native_support: Support,
    pub public_interface_support: Support,
    pub adapter_state: String,
    pub limiting_layer: Option<String>,
    pub reason_code: Option<String>,
    pub evidence: Vec<CapabilityEvidence>,
    pub constraints: CapabilityConstraints,
    #[serde(default)]
    pub freshness: Option<String>,
    #[serde(default)]
    pub unknown_reason: Option<String>,
    /// Effective support per Gateway endpoint; no implicit Chat session semantics.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub api_support: BTreeMap<String, Support>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapabilityEvidence {
    pub source_type: String,
    pub protocol: String,
    pub version: Option<String>,
    pub observed_at: Option<i64>,
    pub live_verified: bool,
    #[serde(default)]
    pub expires_at: Option<i64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CapabilityConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub values: Vec<String>,
    pub counting_basis: Option<String>,
    pub guarantee: Option<String>,
}
impl ChatCapabilities {
    pub fn describe(&mut self, version: Option<String>, protocol: &str) {
        let observed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|value| value.as_secs() as i64);
        for (name, effective) in [
            ("text_input", Support::Supported),
            ("image_input", Support::Unsupported),
            ("non_stream", Support::Supported),
            ("sse", Support::Supported),
            ("system", self.system),
            ("developer", self.developer),
            ("history", self.history),
            ("temperature", self.temperature.support),
            ("top_p", self.top_p.support),
            ("max_tokens", self.max_tokens.support),
            ("max_completion_tokens", self.max_completion_tokens.support),
            (
                "reasoning_effort",
                if self.reasoning_efforts.is_empty() {
                    Support::Unknown
                } else {
                    Support::Supported
                },
            ),
            ("context_window", Support::Unknown),
            ("json_object", Support::Unsupported),
            ("json_schema", Support::Unsupported),
            ("usage", Support::Supported),
            ("finish_reason", Support::Supported),
            ("cancellation", Support::Unknown),
            ("session_resume", Support::Unknown),
            ("prompt_cache_control", Support::Unsupported),
        ] {
            let public = if effective == Support::Supported {
                Support::Supported
            } else {
                Support::Unknown
            };
            self.details.insert(
                name.into(),
                CapabilityDetail {
                    effective_support: effective,
                    public_interface_support: public,
                    adapter_state: if effective == Support::Supported {
                        "IMPLEMENTED"
                    } else {
                        "NOT_IMPLEMENTED"
                    }
                    .into(),
                    limiting_layer: (effective != Support::Supported).then(|| "adapter".into()),
                    reason_code: (effective != Support::Supported)
                        .then(|| "not_implemented".into()),
                    evidence: vec![CapabilityEvidence {
                        source_type: "public_contract_and_fixture".into(),
                        protocol: protocol.into(),
                        version: version.clone(),
                        observed_at,
                        live_verified: false,
                        expires_at: observed_at.map(|at| at + 300),
                    }],
                    freshness: Some("CURRENT".into()),
                    unknown_reason: (effective == Support::Unknown)
                        .then(|| "exact_model_constraints_unverified".into()),
                    ..Default::default()
                },
            );
        }
        for (name, parameter) in [
            ("temperature", &self.temperature),
            ("top_p", &self.top_p),
            ("max_tokens", &self.max_tokens),
            ("max_completion_tokens", &self.max_completion_tokens),
        ] {
            self.details.get_mut(name).unwrap().constraints = CapabilityConstraints {
                min: (parameter.support == Support::Supported).then_some(parameter.min),
                max: parameter.max,
                ..Default::default()
            };
        }
        self.details
            .get_mut("reasoning_effort")
            .unwrap()
            .constraints
            .values = self.reasoning_efforts.clone();
        self.details
            .get_mut("cancellation")
            .unwrap()
            .constraints
            .guarantee = Some("local_receive_only; upstream_generation_and_billing_unknown".into());
        self.describe_session_resume(Support::Unknown);
        let cache = self.details.get_mut("prompt_cache_control").unwrap();
        cache.api_support = BTreeMap::from([
            ("/v1/chat/completions".into(), Support::Unsupported),
            ("/v1/responses".into(), Support::Unsupported),
        ]);
        cache.limiting_layer = Some("gateway".into());
        cache.reason_code = Some("strict_schema_no_cache_control".into());
        for evidence in &mut cache.evidence {
            evidence.source_type = "gateway_contract_and_fixture".into();
            evidence.protocol = "openai_compat_v1".into();
            evidence.version = None;
        }
    }
    pub fn describe_session_resume(&mut self, support: Support) {
        let Some(detail) = self.details.get_mut("session_resume") else {
            return;
        };
        detail.effective_support = support;
        detail.public_interface_support = if support == Support::Supported {
            Support::Supported
        } else {
            Support::Unknown
        };
        detail.adapter_state = if support == Support::Supported {
            "IMPLEMENTED"
        } else {
            "NOT_IMPLEMENTED"
        }
        .into();
        detail.limiting_layer = (support != Support::Supported).then(|| "adapter".into());
        detail.reason_code =
            (support != Support::Supported).then(|| "native_resume_unverified_or_unmapped".into());
        detail.unknown_reason =
            (support == Support::Unknown).then(|| "native_resume_unverified_or_unmapped".into());
        detail.api_support = BTreeMap::from([
            ("/v1/chat/completions".into(), Support::Unsupported),
            ("/v1/responses".into(), support),
        ]);
        detail.constraints.guarantee = Some(
            "same_owner_latest_completed_response_same_model_and_adapter_native_session_required"
                .into(),
        );
    }
    /// Expiry changes effective claims, never the native model's inferred capabilities.
    pub fn expire_evidence(&mut self, at: i64) {
        for (name, detail) in &mut self.details {
            let current = detail.evidence.iter().any(|e| {
                e.observed_at.is_some_and(|t| t <= at)
                    && e.expires_at
                        .or(e.observed_at.map(|t| t + 300))
                        .is_some_and(|t| at < t)
            });
            detail.freshness = Some(
                if current {
                    "CURRENT"
                } else if detail.evidence.is_empty() {
                    "UNKNOWN"
                } else {
                    "STALE"
                }
                .into(),
            );
            if !current && detail.effective_support == Support::Supported {
                detail.effective_support = Support::Unknown;
                detail.unknown_reason = Some("evidence_expired_or_undated".into());
                detail.reason_code = Some("evidence_expired_or_undated".into());
                for support in detail.api_support.values_mut() {
                    if *support == Support::Supported {
                        *support = Support::Unknown;
                    }
                }
                match name.as_str() {
                    "system" => self.system = Support::Unknown,
                    "developer" => self.developer = Support::Unknown,
                    "history" => self.history = Support::Unknown,
                    "temperature" => self.temperature.support = Support::Unknown,
                    "top_p" => self.top_p.support = Support::Unknown,
                    "max_tokens" => self.max_tokens.support = Support::Unknown,
                    "max_completion_tokens" => {
                        self.max_completion_tokens.support = Support::Unknown
                    }
                    "reasoning_effort" => {
                        self.reasoning_efforts.clear();
                        self.default_reasoning_effort = None;
                    }
                    _ => {}
                }
            }
            if detail.effective_support == Support::Unknown && detail.unknown_reason.is_none() {
                detail.unknown_reason = Some(
                    detail
                        .reason_code
                        .clone()
                        .unwrap_or_else(|| "exact_model_constraints_unverified".into()),
                );
            }
        }
    }
    pub fn public_candidate(&mut self, name: &str, reason: &str) {
        if let Some(detail) = self.details.get_mut(name) {
            detail.public_interface_support = Support::Supported;
            detail.limiting_layer = Some("adapter".into());
            detail.reason_code = Some(reason.into());
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub model_id: String,
    pub connection_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub connection_id: String,
    pub provider_session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn resume_evidence_expires_without_opening_chat_or_cache_controls() {
        use super::*;
        let mut chat = ChatCapabilities::single_turn();
        chat.describe(Some("fixture".into()), "offline");
        chat.describe_session_resume(Support::Supported);
        chat.expire_evidence(i64::MAX);
        let resume = &chat.details["session_resume"];
        assert_eq!(resume.effective_support, Support::Unknown);
        assert_eq!(resume.api_support["/v1/responses"], Support::Unknown);
        assert_eq!(
            resume.api_support["/v1/chat/completions"],
            Support::Unsupported
        );
        assert_eq!(
            chat.details["prompt_cache_control"].effective_support,
            Support::Unsupported
        );
    }
    #[test]
    fn unknown_is_not_free() {
        assert_ne!(super::CostMode::Unknown, super::CostMode::Free);
    }
}
