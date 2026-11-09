use localmux_domain::{CostMode, HealthStatus};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct RouteCandidate {
    pub model_id: String,
    pub connection_id: String,
    pub cost_mode: CostMode,
    pub quota_multiplier: Option<f32>,
    pub reserve_available: bool,
    pub health: HealthStatus,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoutePolicy {
    #[serde(default)]
    pub reserve_as_free: bool,
    #[serde(default)]
    pub allow_metered: bool,
    pub best_model: Option<String>,
    #[serde(default)]
    pub priority: Vec<String>,
}
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("model unavailable")]
    ModelUnavailable,
    #[error("free pool exhausted")]
    FreePoolExhausted,
    #[error("no eligible route")]
    NoEligibleRoute,
}
#[derive(Clone, Copy)]
pub enum CooldownReason {
    TransientFailure,
    RateLimited,
    QuotaExhausted,
}
impl CooldownReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::TransientFailure => "transient_failure",
            Self::RateLimited => "rate_limited",
            Self::QuotaExhausted => "quota_exhausted",
        }
    }
}

#[derive(Default)]
pub struct CircuitBreaker {
    open_until: Mutex<HashMap<String, (Instant, CooldownReason)>>,
}
impl CircuitBreaker {
    pub fn open_count(&self, now: Instant) -> usize {
        self.open_until
            .lock()
            .expect("breaker lock")
            .values()
            .filter(|(until, _)| now < *until)
            .count()
    }
    pub fn available(&self, connection_id: &str, now: Instant) -> bool {
        self.cooldown(connection_id, now).is_none()
    }
    pub fn cooldown(
        &self,
        connection_id: &str,
        now: Instant,
    ) -> Option<(Duration, CooldownReason)> {
        self.open_until
            .lock()
            .expect("breaker lock")
            .get(connection_id)
            .filter(|(until, _)| now < *until)
            .map(|(until, reason)| (*until - now, *reason))
    }
    pub fn open(&self, connection_id: &str, now: Instant, duration: Duration) {
        self.open_with_reason(
            connection_id,
            now,
            duration,
            CooldownReason::TransientFailure,
        );
    }
    pub fn open_with_reason(
        &self,
        connection_id: &str,
        now: Instant,
        duration: Duration,
        reason: CooldownReason,
    ) {
        self.open_until
            .lock()
            .expect("breaker lock")
            .insert(connection_id.to_owned(), (now + duration, reason));
    }
    pub fn success(&self, connection_id: &str) {
        self.open_until
            .lock()
            .expect("breaker lock")
            .remove(connection_id);
    }
}

pub fn select<'a>(
    requested: &str,
    candidates: &'a [RouteCandidate],
    policy: &RoutePolicy,
    breaker: &CircuitBreaker,
    now: Instant,
) -> Result<&'a RouteCandidate, RouteError> {
    let eligible = |candidate: &&RouteCandidate| {
        matches!(
            candidate.health,
            HealthStatus::Healthy | HealthStatus::Degraded
        ) && breaker.available(&candidate.connection_id, now)
    };
    if !matches!(requested, "free" | "cheap" | "balanced" | "best") {
        return candidates
            .iter()
            .find(|candidate| {
                candidate.model_id == requested
                    && breaker.available(&candidate.connection_id, now)
                    && !matches!(
                        candidate.health,
                        HealthStatus::AuthRequired
                            | HealthStatus::Offline
                            | HealthStatus::Unsupported
                            | HealthStatus::QuotaExhausted
                    )
            })
            .ok_or(RouteError::ModelUnavailable);
    }
    if requested == "best" {
        let target = policy
            .best_model
            .as_deref()
            .ok_or(RouteError::NoEligibleRoute)?;
        return candidates
            .iter()
            .filter(eligible)
            .find(|candidate| candidate.model_id == target)
            .ok_or(RouteError::NoEligibleRoute);
    }
    if requested == "balanced" && policy.priority.is_empty() {
        return Err(RouteError::NoEligibleRoute);
    }
    let mut allowed: Vec<&RouteCandidate> = candidates
        .iter()
        .filter(eligible)
        .filter(|candidate| match requested {
            "free" => {
                candidate.cost_mode == CostMode::Free
                    || (policy.reserve_as_free
                        && candidate.cost_mode == CostMode::Reserve
                        && candidate.reserve_available)
            }
            "cheap" => {
                matches!(candidate.cost_mode, CostMode::Free | CostMode::Subscription)
                    || (candidate.cost_mode == CostMode::Reserve && candidate.reserve_available)
            }
            "balanced" => {
                matches!(candidate.cost_mode, CostMode::Free | CostMode::Subscription)
                    || (candidate.cost_mode == CostMode::Reserve && candidate.reserve_available)
                    || (policy.allow_metered && candidate.cost_mode == CostMode::Metered)
            }
            _ => false,
        })
        .collect();
    if allowed.is_empty() {
        return Err(if requested == "free" {
            RouteError::FreePoolExhausted
        } else {
            RouteError::NoEligibleRoute
        });
    }
    let priority = |candidate: &RouteCandidate| {
        policy
            .priority
            .iter()
            .position(|item| item == &candidate.model_id)
            .unwrap_or(usize::MAX)
    };
    if requested == "cheap" {
        allowed.sort_by(|left, right| {
            let rank = |candidate: &RouteCandidate| match candidate.cost_mode {
                CostMode::Free => 0,
                CostMode::Reserve => 1,
                CostMode::Subscription => 2,
                _ => 3,
            };
            rank(left)
                .cmp(&rank(right))
                .then_with(|| {
                    left.quota_multiplier
                        .unwrap_or(f32::MAX)
                        .total_cmp(&right.quota_multiplier.unwrap_or(f32::MAX))
                })
                .then_with(|| priority(left).cmp(&priority(right)))
                .then_with(|| left.model_id.cmp(&right.model_id))
        });
    } else {
        allowed.sort_by(|left, right| {
            priority(left)
                .cmp(&priority(right))
                .then_with(|| left.model_id.cmp(&right.model_id))
        });
    }
    Ok(allowed[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(id: &str, mode: CostMode) -> RouteCandidate {
        RouteCandidate {
            model_id: id.into(),
            connection_id: id.into(),
            cost_mode: mode,
            quota_multiplier: None,
            reserve_available: false,
            health: HealthStatus::Healthy,
        }
    }
    #[test]
    fn free_never_falls_back_to_unknown_or_metered() {
        let items = [
            candidate("unknown", CostMode::Unknown),
            candidate("metered", CostMode::Metered),
        ];
        assert_eq!(
            select(
                "free",
                &items,
                &RoutePolicy::default(),
                &CircuitBreaker::default(),
                Instant::now()
            )
            .unwrap_err(),
            RouteError::FreePoolExhausted
        );
        let items = [
            candidate("unknown", CostMode::Unknown),
            candidate("free-model", CostMode::Free),
        ];
        assert_eq!(
            select(
                "free",
                &items,
                &RoutePolicy::default(),
                &CircuitBreaker::default(),
                Instant::now()
            )
            .unwrap()
            .model_id,
            "free-model"
        );
    }
    #[test]
    fn reserve_requires_provider_reported_availability_and_opt_in() {
        let mut reserve = candidate("reserve", CostMode::Reserve);
        let mut policy = RoutePolicy {
            reserve_as_free: true,
            ..Default::default()
        };
        let breaker = CircuitBreaker::default();
        let now = Instant::now();
        assert_eq!(
            select("free", &[reserve.clone()], &policy, &breaker, now).unwrap_err(),
            RouteError::FreePoolExhausted
        );
        reserve.reserve_available = true;
        assert!(select("free", &[reserve.clone()], &policy, &breaker, now).is_ok());
        policy.reserve_as_free = false;
        assert_eq!(
            select("free", &[reserve], &policy, &breaker, now).unwrap_err(),
            RouteError::FreePoolExhausted
        );
    }
    #[test]
    fn cheap_prefers_free_and_best_requires_config() {
        let items = [
            candidate("subscription", CostMode::Subscription),
            candidate("free-model", CostMode::Free),
        ];
        let now = Instant::now();
        assert_eq!(
            select(
                "cheap",
                &items,
                &RoutePolicy::default(),
                &CircuitBreaker::default(),
                now
            )
            .unwrap()
            .model_id,
            "free-model"
        );
        assert_eq!(
            select(
                "best",
                &items,
                &RoutePolicy::default(),
                &CircuitBreaker::default(),
                now
            )
            .unwrap_err(),
            RouteError::NoEligibleRoute
        );
        assert_eq!(
            select(
                "balanced",
                &items,
                &RoutePolicy::default(),
                &CircuitBreaker::default(),
                now
            )
            .unwrap_err(),
            RouteError::NoEligibleRoute
        );
        let policy = RoutePolicy {
            priority: vec!["subscription".into(), "free-model".into()],
            ..Default::default()
        };
        assert_eq!(
            select("balanced", &items, &policy, &CircuitBreaker::default(), now)
                .unwrap()
                .model_id,
            "subscription"
        );
    }
    #[test]
    fn breaker_allows_half_open_after_reset() {
        let items = [candidate("free-model", CostMode::Free)];
        let breaker = CircuitBreaker::default();
        let now = Instant::now();
        breaker.open("free-model", now, Duration::from_secs(60));
        assert_eq!(
            select("free", &items, &RoutePolicy::default(), &breaker, now).unwrap_err(),
            RouteError::FreePoolExhausted
        );
        assert!(select(
            "free",
            &items,
            &RoutePolicy::default(),
            &breaker,
            now + Duration::from_secs(61)
        )
        .is_ok());
    }
}
