use localmux_gateway::GatewayState;
use localmux_secrets::SecretStore;
use localmux_security::{ExecutionProfile, KeyRestrictions};
use tokio::sync::Mutex;

const DEFAULT_KEY_SETTING: &str = "default_api_key_id";
fn reference(id: &str) -> String {
    format!("api.key.{id}")
}

/// One default key lifecycle; permissions and credentials are retained across normal restarts.
pub struct DefaultKey {
    state: GatewayState,
    store: Box<dyn SecretStore>,
    lock: Mutex<()>,
}
impl DefaultKey {
    pub fn new(state: GatewayState, store: Box<dyn SecretStore>) -> Self {
        Self {
            state,
            store,
            lock: Mutex::new(()),
        }
    }
    async fn id(&self) -> Result<Option<String>, String> {
        localmux_storage::setting_string(&self.state.pool, DEFAULT_KEY_SETTING)
            .await
            .map_err(|_| "读取本机 Key 记录失败".into())
    }
    async fn create(&self, old_id: Option<&str>, read_only_agent: bool) -> Result<(), String> {
        let mut policy = KeyRestrictions::default();
        let mut scopes = vec!["models:read", "chat:invoke", "responses:invoke"];
        if read_only_agent {
            policy.execution_profile = ExecutionProfile::ReadOnlyAgent;
            scopes.push("agent:invoke");
        }
        let (id, key) = self
            .state
            .create_key_with_policy(&scopes, policy)
            .await
            .map_err(|_| "创建本机 Key 失败")?;
        if self.store.put(&reference(&id), &key).is_err() {
            let _ = localmux_storage::revoke_api_key(&self.state.pool, &id).await;
            return Err("保存本机 Key 失败；原 Key 保持有效".into());
        }
        if localmux_storage::replace_default_api_key(&self.state.pool, &id, old_id)
            .await
            .is_err()
        {
            let _ = self.store.delete(&reference(&id));
            let _ = localmux_storage::revoke_api_key(&self.state.pool, &id).await;
            return Err("切换本机 Key 失败；原 Key 保持有效".into());
        }
        if let Some(old) = old_id {
            if self.store.delete(&reference(old)).is_err() {
                tracing::warn!("revoked local key credential cleanup failed");
            }
        }
        Ok(())
    }
    pub async fn material(&self) -> Result<Option<(String, String)>, String> {
        let Some(id) = self.id().await? else {
            return Ok(None);
        };
        let key = self
            .store
            .get(&reference(&id))
            .map_err(|_| "本机 Key 凭据文件不可用，请检查文件权限或显式重建")?
            .ok_or("本机 Key 凭据缺失，请显式重建；不会自动更换 Key")?;
        if !self
            .state
            .key_matches_id(&id, &key)
            .await
            .map_err(|_| "校验本机 Key 失败")?
        {
            return Err("本机 Key 已失效或凭据损坏，请显式重建；不会自动更换 Key".into());
        }
        Ok(Some((id, key)))
    }
    pub async fn ensure(&self) -> Result<(), String> {
        let _guard = self.lock.lock().await;
        match localmux_storage::setting_string(&self.state.pool, "default_api_key_store_version")
            .await
            .map_err(|_| "读取本机 Key 存储版本失败")?
            .as_deref()
        {
            Some("1") => {
                self.material()
                    .await?
                    .ok_or("本机 Key 记录缺失，请显式重建")?;
                Ok(())
            }
            None => {
                // One-time transition from the old in-memory development key. Later failures never rotate.
                let old = self.id().await?;
                let allow = self.allows_agent().await?;
                self.create(old.as_deref(), allow).await
            }
            Some(_) => Err("本机 Key 存储版本无法识别，请检查数据或显式重建".into()),
        }
    }

    pub async fn reset(&self, read_only_agent: bool) -> Result<(), String> {
        let _guard = self.lock.lock().await;
        self.create(self.id().await?.as_deref(), read_only_agent)
            .await
    }
    pub async fn allows_agent(&self) -> Result<bool, String> {
        let id = self.id().await?;
        Ok(localmux_storage::list_api_keys(&self.state.pool)
            .await
            .map_err(|_| "读取本机 Key 权限失败")?
            .iter()
            .any(|key| {
                Some(key.id.as_str()) == id.as_deref()
                    && key.policy.execution_profile == ExecutionProfile::ReadOnlyAgent
                    && key.scopes.iter().any(|scope| scope == "agent:invoke")
            }))
    }
    pub async fn set_agent(&self, allow: bool) -> Result<(), String> {
        let _guard = self.lock.lock().await;
        let (id, _) = self
            .material()
            .await?
            .ok_or("本机 Key 不可用，请先显式重建")?;
        let mut record = localmux_storage::list_api_keys(&self.state.pool)
            .await
            .map_err(|_| "读取本机 Key 权限失败")?
            .into_iter()
            .find(|key| key.id == id)
            .ok_or("本机 Key 已撤销")?;
        record.policy.execution_profile = if allow {
            ExecutionProfile::ReadOnlyAgent
        } else {
            ExecutionProfile::InferenceOnly
        };
        record.scopes.retain(|scope| scope != "agent:invoke");
        if allow {
            record.scopes.push("agent:invoke".into());
        }
        localmux_storage::update_api_key_access(
            &self.state.pool,
            &id,
            &record.scopes,
            &record.policy,
        )
        .await
        .map_err(|_| "保存本机 Key 权限失败".into())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use localmux_secrets::LocalFileSecretStore;
    use std::path::Path;
    async fn open(root: &Path) -> (DefaultKey, GatewayState) {
        let pool = localmux_storage::open(&root.join("fixture.db"))
            .await
            .unwrap();
        let state = GatewayState::mock(pool);
        (
            DefaultKey::new(
                state.clone(),
                Box::new(LocalFileSecretStore::new(root.join("keys"))),
            ),
            state,
        )
    }
    #[tokio::test]
    async fn restart_preserves_key_authorization_limits_and_external_authentication() {
        let dir = tempfile::tempdir().unwrap();
        let (key, state) = open(dir.path()).await;
        key.ensure().await.unwrap();
        let first = key.material().await.unwrap().unwrap();
        let mut record = localmux_storage::list_api_keys(&state.pool)
            .await
            .unwrap()
            .remove(0);
        record.policy.allowed_routes = vec!["mock/echo".into()];
        record.policy.allowed_connections = vec!["mock".into()];
        record.policy.rate_limit_per_minute = 37;
        localmux_storage::update_api_key_access(
            &state.pool,
            &first.0,
            &record.scopes,
            &record.policy,
        )
        .await
        .unwrap();
        key.set_agent(true).await.unwrap();
        assert_eq!(key.material().await.unwrap().unwrap(), first);
        assert!(key.allows_agent().await.unwrap());
        drop(key);
        state.pool.close().await;
        drop(state);
        let (key, state) = open(dir.path()).await;
        key.ensure().await.unwrap();
        assert_eq!(key.material().await.unwrap().unwrap(), first);
        assert!(key.allows_agent().await.unwrap());
        let record = localmux_storage::list_api_keys(&state.pool)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(record.policy.allowed_routes, ["mock/echo"]);
        assert_eq!(record.policy.allowed_connections, ["mock"]);
        assert_eq!(record.policy.rate_limit_per_minute, 37);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!(
            "http://127.0.0.1:{}/v1/models",
            listener.local_addr().unwrap().port()
        );
        let server = tokio::spawn(localmux_gateway::serve_on(listener, state.clone()));
        assert_eq!(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(base)
                .bearer_auth(&first.1)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::OK
        );
        server.abort();
        key.set_agent(false).await.unwrap();
        assert!(!key.allows_agent().await.unwrap());
        assert_eq!(key.material().await.unwrap().unwrap(), first);
        assert_eq!(
            localmux_storage::list_api_keys(&state.pool)
                .await
                .unwrap()
                .len(),
            1
        );
        state.pool.close().await;
    }
    #[tokio::test]
    async fn only_explicit_reset_rotates_and_missing_corrupt_or_revoked_credentials_never_recover_silently(
    ) {
        let dir = tempfile::tempdir().unwrap();
        let (key, state) = open(dir.path()).await;
        key.ensure().await.unwrap();
        let first = key.material().await.unwrap().unwrap();
        let (_, independent) = state.create_key(&["models:read"]).await.unwrap();
        key.reset(true).await.unwrap();
        let next = key.material().await.unwrap().unwrap();
        assert_ne!(first, next);
        assert!(
            !localmux_storage::api_key_active_by_id(&state.pool, &first.0)
                .await
                .unwrap()
        );
        assert!(key.allows_agent().await.unwrap());
        assert!(!dir.path().join("keys").join(reference(&first.0)).exists());
        let file = dir.path().join("keys").join(reference(&next.0));
        std::fs::remove_file(&file).unwrap();
        assert!(key.ensure().await.is_err());
        assert_eq!(key.id().await.unwrap().as_deref(), Some(next.0.as_str()));
        assert!(localmux_storage::api_key_active_by_id(&state.pool, &next.0)
            .await
            .unwrap());
        let store = LocalFileSecretStore::new(dir.path().join("keys"));
        store.put(&reference(&next.0), "corrupt-synthetic").unwrap();
        assert!(key.ensure().await.is_err());
        store.put(&reference(&next.0), &next.1).unwrap();
        key.ensure().await.unwrap();
        localmux_storage::revoke_api_key(&state.pool, &next.0)
            .await
            .unwrap();
        assert!(key.ensure().await.is_err());
        assert!(key.set_agent(true).await.is_err());
        key.reset(false).await.unwrap();
        assert_eq!(
            localmux_storage::list_api_keys(&state.pool)
                .await
                .unwrap()
                .len(),
            2
        );
        // The separately created consumer key was never revoked by default-key operations.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://127.0.0.1:{}/v1/models",
            listener.local_addr().unwrap().port()
        );
        let server = tokio::spawn(localmux_gateway::serve_on(listener, state.clone()));
        assert_eq!(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(url)
                .bearer_auth(independent)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::OK
        );
        server.abort();
        state.pool.close().await;
    }
    #[tokio::test]
    async fn legacy_memory_key_is_replaced_once_and_file_write_failure_preserves_the_active_key() {
        let dir = tempfile::tempdir().unwrap();
        let (key, state) = open(dir.path()).await;
        let (legacy_id, legacy_value) = state
            .create_key_with_policy(
                &[
                    "models:read",
                    "chat:invoke",
                    "responses:invoke",
                    "agent:invoke",
                ],
                KeyRestrictions {
                    execution_profile: ExecutionProfile::ReadOnlyAgent,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (key,value) VALUES ('default_api_key_id',?)")
            .bind(&legacy_id)
            .execute(&state.pool)
            .await
            .unwrap();
        key.ensure().await.unwrap();
        let first = key.material().await.unwrap().unwrap();
        assert_ne!(first.0, legacy_id);
        assert_ne!(first.1, legacy_value);
        assert!(
            !localmux_storage::api_key_active_by_id(&state.pool, &legacy_id)
                .await
                .unwrap()
        );
        assert!(key.allows_agent().await.unwrap());
        key.ensure().await.unwrap();
        assert_eq!(key.material().await.unwrap().unwrap(), first);
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.path().join("keys"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(key.reset(false).await.is_err());
        assert_eq!(key.id().await.unwrap().as_deref(), Some(first.0.as_str()));
        assert!(
            localmux_storage::api_key_active_by_id(&state.pool, &first.0)
                .await
                .unwrap()
        );
        assert!(key.allows_agent().await.unwrap());
        std::fs::set_permissions(
            dir.path().join("keys"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        assert_eq!(key.material().await.unwrap().unwrap(), first);
        assert_eq!(
            localmux_storage::list_api_keys(&state.pool)
                .await
                .unwrap()
                .len(),
            1
        );
        state.pool.close().await;
    }
}
