use crate::{AdapterError, NormalizedStream};
use serde_json::Value;
use std::{path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
    time::timeout,
};

pub async fn json_output(executable: &Path, args: &[&str]) -> Result<Value, AdapterError> {
    if !executable.is_file() {
        return Err(AdapterError::ProviderUnavailable);
    }
    let output = timeout(
        Duration::from_secs(20),
        Command::new(executable)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| AdapterError::ProcessFailed)?
    .map_err(|_| AdapterError::ProviderUnavailable)?;
    if !output.status.success() {
        return Err(AdapterError::Unsupported);
    }
    serde_json::from_slice(&output.stdout).map_err(|_| AdapterError::Unsupported)
}

pub async fn stream_json(
    executable: &Path,
    args: &[String],
    prompt: String,
    parse_delta: fn(&Value) -> Option<String>,
    map_error: fn(&Value) -> AdapterError,
) -> Result<(NormalizedStream, Arc<Mutex<Option<String>>>), AdapterError> {
    if !executable.is_file() {
        return Err(AdapterError::ProviderUnavailable);
    }
    let dir = tempfile::tempdir().map_err(|_| AdapterError::ProcessFailed)?;
    let mut child = Command::new(executable)
        .args(args)
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| AdapterError::ProviderUnavailable)?;
    let mut stdin = child.stdin.take().ok_or(AdapterError::ProcessFailed)?;
    stdin
        .write_all(prompt.as_bytes())
        .await
        .map_err(|_| AdapterError::ProcessFailed)?;
    drop(stdin);
    let stdout = child.stdout.take().ok_or(AdapterError::ProcessFailed)?;
    let session = Arc::new(Mutex::new(None));
    let shared = session.clone();
    let stream = Box::pin(async_stream::try_stream! {
        let _dir = dir;
        let mut reader = BufReader::new(stdout);
        let mut saw_delta = false;
        let mut final_text = String::new();
        loop {
            let mut line = String::new();
            let size = timeout(Duration::from_secs(120), reader.read_line(&mut line)).await
                .map_err(|_| AdapterError::ProcessFailed)?.map_err(|_| AdapterError::ProcessFailed)?;
            if size == 0 { break; }
            if size > 1024 * 1024 { Err(AdapterError::ProcessFailed)?; }
            let value: Value = serde_json::from_str(&line).map_err(|_| AdapterError::ProcessFailed)?;
            if let Some(id) = value["session_id"].as_str().or_else(|| value["sessionId"].as_str()) { *shared.lock().await = Some(id.to_owned()); }
            if let Some(error) = value.get("error") { Err(map_error(error))?; }
            if let Some(delta) = parse_delta(&value) { saw_delta = true; yield delta; }
            if value["type"] == "result" {
                if value["is_error"] == true { Err(AdapterError::ProcessFailed)?; }
                if let Some(text) = value["result"].as_str().or_else(|| value["response"].as_str()) { final_text = text.to_owned(); }
            }
        }
        let status = child.wait().await.map_err(|_| AdapterError::ProcessFailed)?;
        if !status.success() { Err(AdapterError::ProcessFailed)?; }
        if !saw_delta && !final_text.is_empty() { yield final_text; }
    });
    Ok((stream, session))
}
