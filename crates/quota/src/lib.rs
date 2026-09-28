use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuotaSource {
    ProviderReported,
    Estimated,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuotaBucket {
    pub connection_id: String,
    pub bucket_id: String,
    pub label: String,
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    pub reset_at: Option<i64>,
    pub source: QuotaSource,
    pub observed_at: i64,
}
impl QuotaBucket {
    pub fn as_snapshot(&self) -> QuotaSnapshot {
        QuotaSnapshot {
            connection_id: self.connection_id.clone(),
            remaining: self.remaining,
            limit: self.limit,
            unit: self.unit.clone(),
            reset_at: self.reset_at,
            source: self.source,
            observed_at: self.observed_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuotaSnapshot {
    pub connection_id: String,
    pub remaining: Option<f64>,
    pub limit: Option<f64>,
    pub unit: Option<String>,
    pub reset_at: Option<i64>,
    pub source: QuotaSource,
    pub observed_at: i64,
}
impl QuotaSnapshot {
    pub fn confirmed_available(&self) -> bool {
        self.source == QuotaSource::ProviderReported
            && self.remaining.is_some_and(|value| value > 0.0)
    }
    pub fn fresh_confirmed_available(&self, now: i64, max_age_seconds: i64) -> bool {
        self.confirmed_available()
            && self.observed_at <= now
            && now - self.observed_at <= max_age_seconds
    }
}

/// Per-connection cache and debounce. Missing reports remain unknown.
pub struct QuotaRegistry {
    snapshots: Mutex<HashMap<String, QuotaSnapshot>>,
    last_refresh: Mutex<HashMap<String, Instant>>,
    minimum_interval: Duration,
}
impl QuotaRegistry {
    pub fn new(minimum_interval: Duration) -> Self {
        Self {
            snapshots: Mutex::new(HashMap::new()),
            last_refresh: Mutex::new(HashMap::new()),
            minimum_interval,
        }
    }
    pub fn should_refresh(&self, connection_id: &str, now: Instant) -> bool {
        let mut last = self.last_refresh.lock().expect("quota refresh lock");
        if last
            .get(connection_id)
            .is_some_and(|time| now.duration_since(*time) < self.minimum_interval)
        {
            return false;
        }
        last.insert(connection_id.to_owned(), now);
        true
    }
    pub fn update(&self, snapshot: QuotaSnapshot) {
        self.snapshots
            .lock()
            .expect("quota snapshot lock")
            .insert(snapshot.connection_id.clone(), snapshot);
    }
    pub fn get(&self, connection_id: &str) -> Option<QuotaSnapshot> {
        self.snapshots
            .lock()
            .expect("quota snapshot lock")
            .get(connection_id)
            .cloned()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_or_estimated_quota_never_claims_available() {
        let mut snapshot = QuotaSnapshot {
            connection_id: "a".into(),
            remaining: Some(10.0),
            limit: Some(10.0),
            unit: Some("credits".into()),
            reset_at: None,
            source: QuotaSource::Estimated,
            observed_at: 0,
        };
        assert!(!snapshot.confirmed_available());
        snapshot.source = QuotaSource::ProviderReported;
        assert!(snapshot.confirmed_available());
        assert!(!snapshot.fresh_confirmed_available(1_000, 600));
        snapshot.observed_at = 900;
        assert!(snapshot.fresh_confirmed_available(1_000, 600));
    }
    #[test]
    fn refresh_is_debounced_per_connection() {
        let registry = QuotaRegistry::new(Duration::from_secs(30));
        let now = Instant::now();
        assert!(registry.should_refresh("a", now));
        assert!(!registry.should_refresh("a", now + Duration::from_secs(1)));
        assert!(registry.should_refresh("b", now + Duration::from_secs(1)));
        assert!(registry.should_refresh("a", now + Duration::from_secs(31)));
    }
    #[test]
    fn separate_windows_keep_independent_resets() {
        let first = QuotaBucket {
            connection_id: "codex".into(),
            bucket_id: "primary".into(),
            label: "5h".into(),
            remaining: Some(75.0),
            limit: Some(100.0),
            unit: Some("%".into()),
            reset_at: Some(100),
            source: QuotaSource::ProviderReported,
            observed_at: 1,
        };
        let second = QuotaBucket {
            bucket_id: "secondary".into(),
            label: "weekly".into(),
            remaining: Some(40.0),
            reset_at: Some(200),
            ..first.clone()
        };
        assert_eq!(first.as_snapshot().reset_at, Some(100));
        assert_eq!(second.as_snapshot().reset_at, Some(200));
    }
}
