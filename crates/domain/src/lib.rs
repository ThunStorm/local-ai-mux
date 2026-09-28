use serde::{Deserialize, Serialize};

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
    fn unknown_is_not_free() {
        assert_ne!(super::CostMode::Unknown, super::CostMode::Free);
    }
}
