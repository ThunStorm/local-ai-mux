use async_trait::async_trait;
use localmux_adapter_sdk::{Adapter, AdapterError, NormalizedRequest, NormalizedResponse};
use localmux_domain::{AdapterKind, HealthStatus, ModelDescriptor};
use std::path::PathBuf;

/// Kimi CLI print mode currently permits agent tools without a verified
/// inference-only configuration. Keep detection separate from invocation.
pub struct KimiAdapter {
    executable: PathBuf,
}
impl KimiAdapter {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
}
#[async_trait]
impl Adapter for KimiAdapter {
    fn id(&self) -> &'static str {
        "kimi"
    }
    fn kind(&self) -> AdapterKind {
        AdapterKind::Agent
    }
    async fn health(&self) -> HealthStatus {
        if self.executable.is_file() {
            HealthStatus::Unsupported
        } else {
            HealthStatus::Offline
        }
    }
    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, AdapterError> {
        Err(AdapterError::Unsupported)
    }
    async fn invoke(
        &self,
        _request: NormalizedRequest,
    ) -> Result<NormalizedResponse, AdapterError> {
        Err(AdapterError::Unsupported)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn unsupported_runtime_never_exposes_a_model() {
        let adapter = KimiAdapter::new(PathBuf::from("kimi"));
        assert!(matches!(
            adapter.list_models().await,
            Err(AdapterError::Unsupported)
        ));
    }
}
