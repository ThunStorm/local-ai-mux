use localmux_adapter_sdk::{provider_http::HttpProvider, AdapterError};
use secrecy::SecretString;

/// Coding Plan endpoint; the general metered API has different billing.
pub const CODING_BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/coding/v3";

pub fn connect(key: SecretString) -> Result<HttpProvider, AdapterError> {
    HttpProvider::new("ark", CODING_BASE_URL, key)
}

#[cfg(test)]
mod tests {
    #[test]
    fn stays_on_coding_plan_endpoint() {
        assert!(super::CODING_BASE_URL.contains("/coding/"));
    }
}
