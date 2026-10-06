use crate::{AdapterError, EventStream, FinishReason, ResponseMetadata, StreamEvent};
use serde_json::Value;
use std::{path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
    time::timeout,
};

pub async fn raw_output(executable: &Path, args: &[&str]) -> Result<Vec<u8>, AdapterError> {
    if !executable.is_file() {
        return Err(AdapterError::ProviderUnavailable);
    }
    let dir = tempfile::tempdir().map_err(|_| AdapterError::ProcessFailed)?;
    let output = timeout(
        Duration::from_secs(20),
        Command::new(executable)
            .args(args)
            .current_dir(dir.path())
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| AdapterError::ProcessFailed)?
    .map_err(|_| AdapterError::ProviderUnavailable)?;
    if !output.status.success() {
        return Err(classify_cli_failure(&output.stdout, &output.stderr));
    }
    Ok(output.stdout)
}

fn classify_cli_failure(stdout: &[u8], stderr: &[u8]) -> AdapterError {
    let message = format!(
        "{} {}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
    .to_ascii_lowercase();
    if message.contains("not logged in")
        || message.contains("login")
        || message.contains("sign in")
        || message.contains("authentication")
    {
        AdapterError::AuthRequired
    } else if message.contains("rate limit") {
        AdapterError::RateLimited
    } else if message.contains("quota") || message.contains("credit") {
        AdapterError::QuotaExhausted
    } else {
        AdapterError::ProcessFailed
    }
}

pub async fn stream_json(
    executable: &Path,
    args: &[String],
    prompt: String,
    parse_delta: fn(&Value) -> Option<String>,
    map_error: fn(&Value) -> AdapterError,
) -> Result<(EventStream, Arc<Mutex<Option<String>>>), AdapterError> {
    if !executable.is_file() {
        return Err(AdapterError::ProviderUnavailable);
    }
    let command = Command::new(executable);
    stream_json_command(command, args, prompt, parse_delta, map_error).await
}

pub async fn stream_json_command(
    mut command: Command,
    args: &[String],
    prompt: String,
    parse_delta: fn(&Value) -> Option<String>,
    map_error: fn(&Value) -> AdapterError,
) -> Result<(EventStream, Arc<Mutex<Option<String>>>), AdapterError> {
    let dir = tempfile::tempdir().map_err(|_| AdapterError::ProcessFailed)?;
    let mut child = command
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
        let mut metadata = None;
        loop {
            let mut line = String::new();
            let size = timeout(Duration::from_secs(120), reader.read_line(&mut line)).await
                .map_err(|_| AdapterError::ProcessFailed)?.map_err(|_| AdapterError::ProcessFailed)?;
            if size == 0 { break; }
            if size > 1024 * 1024 { Err(AdapterError::ProcessFailed)?; }
            let value: Value = serde_json::from_str(&line).map_err(|_| AdapterError::ProcessFailed)?;
            if let Some(id) = value["session_id"].as_str().or_else(|| value["sessionId"].as_str()) { *shared.lock().await = Some(id.to_owned()); }
            if let Some(error) = value.get("error") { Err(map_error(error))?; }
            if let Some(delta) = parse_delta(&value) { saw_delta = true; yield StreamEvent::Delta(delta); }
            if value["type"] == "result" {
                if value["is_error"] == true || value["subtype"].as_str().is_some_and(|s| s.starts_with("error")) { Err(AdapterError::ProcessFailed)?; }
                if metadata.is_some() { Err(AdapterError::ProcessFailed)?; }
                // CLI result alone does not certify model stop, per-request usage or actual route.
                metadata = Some(ResponseMetadata { finish_reason: FinishReason::Unknown, ..Default::default() });
                if let Some(text) = value["result"].as_str().or_else(|| value["response"].as_str()) { final_text = text.to_owned(); }
            }
        }
        let status = timeout(Duration::from_secs(5), child.wait()).await.map_err(|_| AdapterError::ProcessFailed)?.map_err(|_| AdapterError::ProcessFailed)?;
        if !status.success() { Err(AdapterError::ProcessFailed)?; }
        if !saw_delta && !final_text.is_empty() { yield StreamEvent::Delta(final_text); }
    let metadata = metadata.ok_or(AdapterError::ProcessFailed)?;
        yield StreamEvent::Completed(metadata);
    });
    Ok((stream, session))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_auth_failure_is_not_reported_as_unsupported() {
        assert!(matches!(
            classify_cli_failure(b"", b"Not logged in. Run qodercli login"),
            AdapterError::AuthRequired
        ));
        assert!(matches!(
            classify_cli_failure(b"", b"Node.js is unavailable"),
            AdapterError::ProcessFailed
        ));
    }
}
