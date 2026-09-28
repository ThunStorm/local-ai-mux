use localmux_domain::AdapterKind;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionProfile {
    InferenceOnly,
    ReadOnlyAgent,
    WorkspaceWrite,
    FullAgent,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyRestrictions {
    #[serde(default)]
    pub allowed_routes: Vec<String>,
    #[serde(default)]
    pub allowed_connections: Vec<String>,
    pub execution_profile: ExecutionProfile,
    pub rate_limit_per_minute: u32,
}
impl Default for KeyRestrictions {
    fn default() -> Self {
        Self {
            allowed_routes: Vec::new(),
            allowed_connections: Vec::new(),
            execution_profile: ExecutionProfile::InferenceOnly,
            rate_limit_per_minute: 60,
        }
    }
}
impl KeyRestrictions {
    pub fn valid(&self) -> bool {
        (1..=600).contains(&self.rate_limit_per_minute)
            && self.allowed_routes.len() <= 64
            && self.allowed_connections.len() <= 64
            && self
                .allowed_routes
                .iter()
                .chain(&self.allowed_connections)
                .all(|id| {
                    !id.is_empty()
                        && id.len() <= 128
                        && id.bytes().all(|byte| byte.is_ascii_graphic())
                })
            && matches!(
                self.execution_profile,
                ExecutionProfile::InferenceOnly | ExecutionProfile::ReadOnlyAgent
            )
    }
    pub fn permits(
        &self,
        requested: &str,
        actual: &str,
        connection: &str,
        kind: AdapterKind,
    ) -> bool {
        let route_allowed = self.allowed_routes.is_empty()
            || self
                .allowed_routes
                .iter()
                .any(|id| id == requested || id == actual);
        let connection_allowed = self.allowed_connections.is_empty()
            || self.allowed_connections.iter().any(|id| id == connection);
        let profile_allowed =
            kind != AdapterKind::Agent || self.execution_profile != ExecutionProfile::InferenceOnly;
        route_allowed && connection_allowed && profile_allowed
    }
}
#[derive(Default)]
pub struct KeyRateLimiter {
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}
impl KeyRateLimiter {
    pub fn allow(&self, key_id: &str, limit: u32, now: Instant) -> bool {
        let mut windows = self.windows.lock().expect("rate limiter lock");
        let entry = windows.entry(key_id.to_owned()).or_insert((now, 0));
        if now.duration_since(entry.0) >= Duration::from_secs(60) {
            *entry = (now, 0);
        }
        if entry.1 >= limit {
            return false;
        }
        entry.1 += 1;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_key_blocks_agents_and_restricts_routes_when_set() {
        let mut policy = KeyRestrictions::default();
        assert!(policy.permits("mock/echo", "mock/echo", "mock", AdapterKind::Mock));
        assert!(!policy.permits("codex/model", "codex/model", "codex", AdapterKind::Agent));
        policy.execution_profile = ExecutionProfile::ReadOnlyAgent;
        policy.allowed_routes = vec!["codex/model".into()];
        assert!(policy.permits("codex/model", "codex/model", "codex", AdapterKind::Agent));
        assert!(!policy.permits("qoder/model", "qoder/model", "qoder", AdapterKind::Agent));
    }
    #[test]
    fn fixed_window_limits_each_key_independently() {
        let limiter = KeyRateLimiter::default();
        let now = Instant::now();
        assert!(limiter.allow("a", 1, now));
        assert!(!limiter.allow("a", 1, now));
        assert!(limiter.allow("b", 1, now));
        assert!(limiter.allow("a", 1, now + Duration::from_secs(61)));
    }
    #[test]
    fn unsupported_profiles_fail_closed() {
        let mut policy = KeyRestrictions {
            execution_profile: ExecutionProfile::WorkspaceWrite,
            ..Default::default()
        };
        assert!(!policy.valid());
        policy.execution_profile = ExecutionProfile::FullAgent;
        assert!(!policy.valid());
    }
}
