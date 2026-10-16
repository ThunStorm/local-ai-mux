use sqlx::SqlitePool;

#[derive(Clone, Debug, Default, serde::Serialize, sqlx::FromRow)]
pub struct CallRecord {
    pub sequence: i64,
    pub id: String,
    pub owner_key_id: Option<String>,
    pub source: String,
    pub endpoint: String,
    pub requested_model: String,
    pub resolved_model: Option<String>,
    pub adapter_id: Option<String>,
    pub response_id: Option<String>,
    pub conversation_id: Option<String>,
    pub streamed: bool,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
    pub finish_reason: Option<String>,
    pub actual_model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub request_id: Option<String>,
    pub diagnostics: Option<sqlx::types::Json<CallDiagnostics>>,
}

/// Fixed keys and optional measurements; no prompt, raw events, arbitrary upstream error or credentials.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CallDiagnostics {
    pub accepted_at: Option<i64>,
    pub auth_route_ms: Option<u64>,
    pub queue_wait_ms: Option<u64>,
    pub interval_wait_ms: Option<u64>,
    pub adapter_startup_ms: Option<u64>,
    pub adapter_ms: Option<u64>,
    pub first_output_ms: Option<u64>,
    pub model_execution_ms: Option<u64>,
    pub response_finalize_ms: Option<u64>,
    pub total_elapsed_ms: Option<u64>,
    pub cancel_requested: bool,
    pub upstream_cancel_verified: Option<String>,
    pub route_kind: Option<String>,
    pub usage_source: Option<String>,
    pub usage_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_cached_input_tokens: Option<u64>,
    pub finish_source: Option<String>,
    pub actual_model_source: Option<String>,
    pub normalization: Option<String>,
    pub outcome: Option<String>,
    pub request_bytes: Option<u64>,
    pub response_bytes: Option<u64>,
    pub response_bytes_scope: Option<String>,
    pub output_text_bytes: Option<u64>,
    pub http_status: Option<u16>,
    pub reason_code: Option<String>,
    pub body_recording_enabled: Option<bool>,
}

pub async fn start_call(pool: &SqlitePool, record: &CallRecord) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO call_records (id,owner_key_id,source,endpoint,requested_model,streamed,started_at,status) VALUES (?,?,?,?,?,?,?,'in_progress')")
        .bind(&record.id).bind(&record.owner_key_id).bind(&record.source).bind(&record.endpoint)
        .bind(&record.requested_model).bind(record.streamed).bind(record.started_at).execute(pool).await?;
    Ok(())
}

pub async fn finish_call(pool: &SqlitePool, record: &CallRecord) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE call_records SET resolved_model=?,adapter_id=?,response_id=?,conversation_id=?,finished_at=?,elapsed_ms=?,status=?,error_code=?,finish_reason=?,actual_model=?,input_tokens=?,output_tokens=?,total_tokens=?,request_id=?,diagnostics=? WHERE id=? AND status='in_progress'")
        .bind(&record.resolved_model).bind(&record.adapter_id).bind(&record.response_id).bind(&record.conversation_id)
        .bind(record.finished_at).bind(record.elapsed_ms).bind(&record.status).bind(&record.error_code)
        .bind(&record.finish_reason).bind(&record.actual_model).bind(record.input_tokens).bind(record.output_tokens)
        .bind(record.total_tokens).bind(&record.request_id).bind(&record.diagnostics).bind(&record.id).execute(&mut *tx).await?;
    sqlx::query("UPDATE call_contents SET state=? WHERE call_id=? AND state='recording'")
        .bind(if record.status == "completed" {
            "complete"
        } else {
            "partial"
        })
        .bind(&record.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallFilter {
    pub before: Option<i64>,
    pub source: Option<String>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub conversation_id: Option<String>,
}

pub async fn list_calls(pool: &SqlitePool, filter: &CallFilter) -> anyhow::Result<Vec<CallRecord>> {
    // Stable keyset pagination; fetch one extra row so the UI can show a next-page button.
    Ok(sqlx::query_as("SELECT * FROM call_records WHERE (? IS NULL OR sequence < ?) AND (? IS NULL OR source=?) AND (? IS NULL OR status=?) AND (? IS NULL OR instr(requested_model,?)>0 OR instr(COALESCE(resolved_model,''),?)>0) AND (? IS NULL OR conversation_id=?) ORDER BY sequence DESC LIMIT 51")
        .bind(filter.before).bind(filter.before).bind(&filter.source).bind(&filter.source)
        .bind(&filter.status).bind(&filter.status).bind(&filter.model).bind(&filter.model).bind(&filter.model)
        .bind(&filter.conversation_id).bind(&filter.conversation_id)
        .fetch_all(pool).await?)
}

pub async fn delete_calls(pool: &SqlitePool, id: Option<&str>) -> anyhow::Result<u64> {
    Ok(sqlx::query(
        "DELETE FROM call_records WHERE status <> 'in_progress' AND (? IS NULL OR id=?)",
    )
    .bind(id)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected())
}

pub async fn recover_interrupted_calls(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE call_records SET status='interrupted',error_code='LMX_PROCESS_INTERRUPTED',finished_at=CAST(strftime('%s','now') AS INTEGER) WHERE status='in_progress'")
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE call_contents SET state='partial' WHERE state='recording'")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str) -> CallRecord {
        CallRecord {
            id: id.into(),
            source: "api".into(),
            endpoint: "chat".into(),
            requested_model: "mock/echo".into(),
            started_at: 1,
            status: "completed".into(),
            ..Default::default()
        }
    }
    #[tokio::test]
    async fn stable_pages_filters_and_cleanup_preserve_running_calls_and_resume_mappings() {
        let pool = crate::open_memory().await.unwrap();
        for n in 0..55 {
            let mut call = record(&format!("call-{n}"));
            call.source = if n % 2 == 0 { "playground" } else { "api" }.into();
            start_call(&pool, &call).await.unwrap();
            finish_call(&pool, &call).await.unwrap();
        }
        let page = list_calls(&pool, &CallFilter::default()).await.unwrap();
        assert_eq!(page.len(), 51);
        start_call(&pool, &record("running")).await.unwrap();
        let next = list_calls(
            &pool,
            &CallFilter {
                before: Some(page[49].sequence),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(next.len(), 5);
        assert!(next
            .iter()
            .all(|r| !page[..50].iter().any(|p| p.id == r.id)));
        assert_eq!(
            list_calls(
                &pool,
                &CallFilter {
                    source: Some("playground".into()),
                    status: Some("completed".into()),
                    model: Some("echo".into()),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .len(),
            28
        );
        sqlx::query("INSERT INTO conversations (id,model_id,adapter_id,created_at,last_active_at) VALUES ('existing','mock/echo','mock',1,1)").execute(&pool).await.unwrap();
        assert_eq!(delete_calls(&pool, Some("running")).await.unwrap(), 0);
        assert_eq!(delete_calls(&pool, Some("call-0")).await.unwrap(), 1);
        assert_eq!(delete_calls(&pool, None).await.unwrap(), 54);
        assert_eq!(
            list_calls(&pool, &CallFilter::default()).await.unwrap()[0].status,
            "in_progress"
        );
        assert_eq!(crate::list_conversations(&pool).await.unwrap().len(), 1);
    }
    #[tokio::test]
    async fn version_ten_upgrade_preserves_legacy_sessions_and_reopen_recovers_only_unfinished_calls(
    ) {
        let path = std::env::temp_dir().join(format!("localmux-calls-{}.db", std::process::id()));
        let pool = crate::open(&path).await.unwrap();
        sqlx::raw_sql("DROP TABLE call_contents; DROP TABLE call_recording_settings; DROP TABLE call_records; DELETE FROM _sqlx_migrations WHERE version>=11; INSERT INTO conversations (id,model_id,adapter_id,created_at,last_active_at) VALUES ('legacy','mock/echo','mock',1,1);").execute(&pool).await.unwrap();
        pool.close().await;
        let pool = crate::open(&path).await.unwrap();
        assert_eq!(
            crate::list_conversations(&pool).await.unwrap()[0].id,
            "legacy"
        );
        start_call(&pool, &record("crashed")).await.unwrap();
        start_call(&pool, &record("done")).await.unwrap();
        finish_call(&pool, &record("done")).await.unwrap();
        pool.close().await;
        let pool = crate::open(&path).await.unwrap();
        let records = list_calls(&pool, &CallFilter::default()).await.unwrap();
        assert_eq!(records[0].status, "completed");
        assert_eq!(records[1].status, "interrupted");
        assert_eq!(
            records[1].error_code.as_deref(),
            Some("LMX_PROCESS_INTERRUPTED")
        );
        assert_eq!(records[1].elapsed_ms, None);
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
}

#[derive(Clone, Debug, serde::Serialize, sqlx::FromRow)]
pub struct RecordingSetting {
    pub enabled: bool,
    pub generation: i64,
}
pub async fn recording_setting(pool: &SqlitePool) -> anyhow::Result<RecordingSetting> {
    Ok(
        sqlx::query_as("SELECT enabled,generation FROM call_recording_settings WHERE id=1")
            .fetch_one(pool)
            .await?,
    )
}
pub async fn set_recording(pool: &SqlitePool, enabled: bool) -> anyhow::Result<RecordingSetting> {
    let mut tx = pool.begin().await?;
    // The generation gates active writes atomically with the switch; re-enabling cannot resume an old capture.
    sqlx::query("UPDATE call_recording_settings SET enabled=?,generation=generation+1 WHERE id=1 AND enabled<>?")
        .bind(enabled).bind(enabled).execute(&mut *tx).await?;
    if !enabled {
        sqlx::query("UPDATE call_contents SET state='stopped' WHERE state='recording'")
            .execute(&mut *tx)
            .await?;
    }
    let setting =
        sqlx::query_as("SELECT enabled,generation FROM call_recording_settings WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(setting)
}
pub async fn start_content(
    pool: &SqlitePool,
    id: &str,
    request_json: &str,
) -> anyhow::Result<Option<i64>> {
    Ok(sqlx::query_scalar("INSERT INTO call_contents (call_id,recording_generation,request_json,state) SELECT ?,generation,?,'recording' FROM call_recording_settings WHERE id=1 AND enabled=1 ON CONFLICT(call_id) DO NOTHING RETURNING recording_generation")
        .bind(id).bind(request_json).fetch_optional(pool).await?)
}
pub async fn update_content(
    pool: &SqlitePool,
    id: &str,
    generation: i64,
    response: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query("UPDATE call_contents SET response_text=? WHERE call_id=? AND recording_generation=? AND state='recording' AND EXISTS(SELECT 1 FROM call_recording_settings WHERE id=1 AND enabled=1 AND generation=?)")
        .bind(response).bind(id).bind(generation).bind(generation).execute(pool).await?.rows_affected()==1)
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct CallContent {
    pub messages: serde_json::Value,
    pub answer: String,
    pub state: String,
}
pub async fn call_content(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<CallContent>> {
    use sqlx::Row;
    let row =
        sqlx::query("SELECT request_json,response_text,state FROM call_contents WHERE call_id=?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    row.map(|row| {
        Ok(CallContent {
            messages: serde_json::from_str(&row.try_get::<String, _>("request_json")?)?,
            answer: row.try_get("response_text")?,
            state: row.try_get("state")?,
        })
    })
    .transpose()
}

#[cfg(test)]
mod content_tests {
    use super::*;
    #[tokio::test]
    async fn toggle_and_content_survive_restart_and_cleanup_cascades_without_changing_sessions() {
        let path =
            std::env::temp_dir().join(format!("localmux-dialogue-{}.db", std::process::id()));
        let pool = crate::open(&path).await.unwrap();
        assert!(!recording_setting(&pool).await.unwrap().enabled);
        let record = CallRecord {
            id: "with-body".into(),
            source: "api".into(),
            endpoint: "chat".into(),
            requested_model: "mock/echo".into(),
            status: "completed".into(),
            ..Default::default()
        };
        start_call(&pool, &record).await.unwrap();
        assert!(start_content(&pool, &record.id, "[]")
            .await
            .unwrap()
            .is_none());
        set_recording(&pool, true).await.unwrap();
        let generation = start_content(
            &pool,
            &record.id,
            "[{\"role\":\"user\",\"content\":\"原始输入\"}]",
        )
        .await
        .unwrap()
        .unwrap();
        assert!(update_content(&pool, &record.id, generation, "部分文字")
            .await
            .unwrap());
        pool.close().await;
        let pool = crate::open(&path).await.unwrap();
        assert!(recording_setting(&pool).await.unwrap().enabled);
        assert_eq!(
            call_content(&pool, &record.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            "partial"
        );
        set_recording(&pool, false).await.unwrap();
        set_recording(&pool, true).await.unwrap();
        assert!(!update_content(&pool, &record.id, generation, "不应写入")
            .await
            .unwrap());
        assert_eq!(
            call_content(&pool, &record.id)
                .await
                .unwrap()
                .unwrap()
                .answer,
            "部分文字"
        );
        delete_calls(&pool, Some(&record.id)).await.unwrap();
        assert!(call_content(&pool, &record.id).await.unwrap().is_none());
        set_recording(&pool, false).await.unwrap();
        pool.close().await;
        let pool = crate::open(&path).await.unwrap();
        assert!(!recording_setting(&pool).await.unwrap().enabled);
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    #[tokio::test]
    async fn legacy_diagnostics_remain_unknown_without_backfill() {
        let pool = crate::open_memory().await.unwrap();
        let record = CallRecord {
            id: "legacy-cache".into(),
            source: "api".into(),
            endpoint: "chat".into(),
            requested_model: "fixture/text".into(),
            status: "completed".into(),
            ..Default::default()
        };
        start_call(&pool, &record).await.unwrap();
        finish_call(&pool, &record).await.unwrap();
        let legacy = r#"{"cancel_requested":false,"usage_source":"provider_reported","usage_scope":"request"}"#;
        sqlx::query("UPDATE call_records SET diagnostics=? WHERE id=?")
            .bind(legacy)
            .bind(&record.id)
            .execute(&pool)
            .await
            .unwrap();
        let rows = list_calls(&pool, &Default::default()).await.unwrap();
        let diagnostics = rows[0].diagnostics.as_ref().unwrap();
        assert!(diagnostics.reported_input_tokens.is_none());
        assert!(diagnostics.reported_cached_input_tokens.is_none());
        assert!(diagnostics.usage_status.is_none());
        let stored: String = sqlx::query_scalar("SELECT diagnostics FROM call_records WHERE id=?")
            .bind(&record.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, legacy);
    }

    #[tokio::test]
    async fn version_twelve_upgrade_keeps_old_records_unknown_and_rejects_unbounded_diagnostics() {
        let path =
            std::env::temp_dir().join(format!("localmux-diagnostics-{}.db", std::process::id()));
        let pool = crate::open(&path).await.unwrap();
        let mut old = CallRecord {
            id: "legacy".into(),
            source: "api".into(),
            endpoint: "chat".into(),
            requested_model: "mock/echo".into(),
            status: "completed".into(),
            ..Default::default()
        };
        start_call(&pool, &old).await.unwrap();
        finish_call(&pool, &old).await.unwrap();
        sqlx::raw_sql("DROP INDEX call_records_request_id; ALTER TABLE call_records DROP COLUMN request_id; ALTER TABLE call_records DROP COLUMN diagnostics; DELETE FROM _sqlx_migrations WHERE version=13;").execute(&pool).await.unwrap();
        pool.close().await;
        let pool = crate::open(&path).await.unwrap();
        let legacy = list_calls(&pool, &Default::default())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(legacy.status, "completed");
        assert!(legacy.request_id.is_none() && legacy.diagnostics.is_none());
        assert!(!recording_setting(&pool).await.unwrap().enabled);
        old.id = "bounded".into();
        old.request_id = Some("req_synthetic".into());
        old.diagnostics = Some(sqlx::types::Json(CallDiagnostics {
            usage_source: Some("x".repeat(8192)),
            ..Default::default()
        }));
        start_call(&pool, &old).await.unwrap();
        assert!(finish_call(&pool, &old).await.is_err());
        old.diagnostics = Some(sqlx::types::Json(CallDiagnostics {
            upstream_cancel_verified: Some("UNKNOWN".into()),
            ..Default::default()
        }));
        finish_call(&pool, &old).await.unwrap();
        let row = list_calls(&pool, &Default::default())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(row.request_id, old.request_id);
        assert_eq!(
            row.diagnostics.unwrap().upstream_cancel_verified.as_deref(),
            Some("UNKNOWN")
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "ok"
        );
        pool.close().await;
        std::fs::remove_file(path).unwrap();
    }
}
