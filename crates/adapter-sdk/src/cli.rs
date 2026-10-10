use crate::{AdapterError, EventStream, FinishReason, ResponseMetadata, StreamEvent};
use serde_json::Value;
use std::{path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
    time::timeout,
};

pub const CLI_IDLE_TIMEOUT: Duration = Duration::from_secs(180);

const MAX_CLI_BYTES: usize = 1024 * 1024;

pub async fn read_json_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<Value>, AdapterError> {
    let mut bytes = Vec::new();
    let size = reader
        .take((MAX_CLI_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|_| AdapterError::ProcessFailed)?;
    if size == 0 {
        return Ok(None);
    }
    if size > MAX_CLI_BYTES {
        return Err(AdapterError::ProcessFailed);
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| AdapterError::ProcessFailed)
}
async fn bounded_output<R: AsyncRead + Unpin>(reader: R) -> Result<Vec<u8>, AdapterError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_CLI_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| AdapterError::ProcessFailed)?;
    if bytes.len() > MAX_CLI_BYTES {
        return Err(AdapterError::ProcessFailed);
    }
    Ok(bytes)
}
pub async fn command_output(
    mut command: Command,
    args: &[&str],
    limit: Duration,
) -> Result<std::process::Output, AdapterError> {
    let dir = tempfile::tempdir().map_err(|_| AdapterError::ProcessFailed)?;
    command
        .args(args)
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = crate::process::OwnedChild::spawn(command)
        .map_err(|_| AdapterError::ProviderUnavailable)?;
    let stdout = child.stdout.take().ok_or(AdapterError::ProcessFailed)?;
    let stderr = child.stderr.take().ok_or(AdapterError::ProcessFailed)?;
    timeout(limit, async {
        let (stdout, stderr) = tokio::try_join!(bounded_output(stdout), bounded_output(stderr))?;
        let status = child
            .wait()
            .await
            .map_err(|_| AdapterError::ProcessFailed)?;
        Ok(std::process::Output {
            status,
            stdout,
            stderr,
        })
    })
    .await
    .map_err(|_| AdapterError::Timeout)?
}
pub async fn raw_output(executable: &Path, args: &[&str]) -> Result<Vec<u8>, AdapterError> {
    if !executable.is_file() {
        return Err(AdapterError::ProviderUnavailable);
    }
    let output = command_output(Command::new(executable), args, Duration::from_secs(20)).await?;
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
    command
        .args(args)
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = crate::process::OwnedChild::spawn(command)
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
            let Some(value) = timeout(CLI_IDLE_TIMEOUT, read_json_line(&mut reader)).await.map_err(|_| AdapterError::Timeout)?? else { break; };
            if let Some(id) = value["session_id"].as_str().or_else(|| value["sessionId"].as_str()) { *shared.lock().await = Some(id.to_owned()); }
            if let Some(error) = value.get("error") { Err(map_error(error))?; }
            if let Some(delta) = parse_delta(&value) { saw_delta = true; yield StreamEvent::Delta(delta); }
            if value["type"] == "result" {
                if value["is_error"] == true || value["subtype"].as_str().is_some_and(|s| s.starts_with("error")) {
                    // Classify in memory only; never persist the result text or credentials.
                    Err(classify_cli_failure(value.to_string().as_bytes(), b""))?;
                }
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

    #[cfg(unix)]
    #[tokio::test]
    async fn cli_result_errors_preserve_quota_and_auth_categories() {
        for (message, quota) in [("quota exhausted", true), ("not logged in", false)] {
            let script = format!("cat >/dev/null; printf '%s\\n' '{{\"type\":\"result\",\"is_error\":true,\"result\":\"{message}\"}}'");
            let (stream, _) = stream_json_command(
                Command::new("/bin/sh"),
                &["-c".into(), script],
                "fixture".into(),
                |_| None,
                |_| AdapterError::ProcessFailed,
            )
            .await
            .unwrap();
            let error = crate::collect_events(stream).await.unwrap_err();
            assert!(if quota {
                matches!(error, AdapterError::QuotaExhausted)
            } else {
                matches!(error, AdapterError::AuthRequired)
            });
        }
    }
}

#[cfg(test)]
mod reliability_tests {
    use super::*;
    use futures_util::StreamExt;
    #[tokio::test]
    async fn json_lines_and_catalog_output_stop_at_the_byte_limit() {
        let oversized = vec![b'x'; MAX_CLI_BYTES + 10];
        assert!(matches!(
            read_json_line(&mut BufReader::new(&oversized[..])).await,
            Err(AdapterError::ProcessFailed)
        ));
        let mut valid = BufReader::new(&b"{\"type\":\"result\"}\n"[..]);
        assert_eq!(
            read_json_line(&mut valid).await.unwrap().unwrap()["type"],
            "result"
        );
        assert!(read_json_line(&mut valid).await.unwrap().is_none());
        assert!(bounded_output(&oversized[..]).await.is_err());
    }
    #[cfg(unix)]
    async fn stopped(pid: &str) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let result = Command::new("/bin/ps")
                    .args(["-o", "stat=", "-p", pid])
                    .output()
                    .await
                    .unwrap();
                let state = String::from_utf8_lossy(&result.stdout);
                if !result.status.success()
                    || state.trim().is_empty()
                    || state.trim().starts_with('Z')
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("owned fixture process must stop promptly");
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn dropping_polled_or_unpolled_cli_cleans_its_group_and_preserves_other_tasks() {
        for poll in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let parent_file = dir.path().join("parent");
            let child_file = dir.path().join("child");
            let mut unrelated = Command::new("/bin/sleep")
                .arg("30")
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let mut command = Command::new("/bin/sh");
            command
                .env("LOCALMUX_FIXTURE_PARENT", &parent_file)
                .env("LOCALMUX_FIXTURE_CHILD", &child_file);
            let script = "cat >/dev/null; echo $$ > \"$LOCALMUX_FIXTURE_PARENT\"; sleep 30 & echo $! > \"$LOCALMUX_FIXTURE_CHILD\"; printf '%s\\n' '{\"delta\":\"ready\"}'; wait";
            let (mut stream, _) = stream_json_command(
                command,
                &["-c".into(), script.into()],
                "synthetic".into(),
                |v| v["delta"].as_str().map(str::to_owned),
                |_| AdapterError::ProcessFailed,
            )
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                while !child_file.exists()
                    || std::fs::read_to_string(&child_file)
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            let parent = std::fs::read_to_string(parent_file).unwrap();
            let child = std::fs::read_to_string(child_file).unwrap();
            if poll {
                assert!(matches!(
                    stream.next().await,
                    Some(Ok(StreamEvent::Delta(_)))
                ));
            }
            drop(stream);
            stopped(parent.trim()).await;
            stopped(child.trim()).await;
            assert!(unrelated.try_wait().unwrap().is_none());
            unrelated.kill().await.unwrap();
            unrelated.wait().await.unwrap();
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_spawn_and_malformed_failures_release_owned_processes_without_retries() {
        let output = command_output(
            Command::new("/bin/sh"),
            &["-c", "sleep 30"],
            Duration::from_millis(30),
        )
        .await;
        assert!(matches!(output, Err(AdapterError::Timeout)));
        assert!(matches!(
            command_output(
                Command::new("/synthetic/nonexistent"),
                &[],
                Duration::from_secs(1)
            )
            .await,
            Err(AdapterError::ProviderUnavailable)
        ));
        for script in [
            "cat >/dev/null; echo malformed",
            "cat >/dev/null; printf '%s\\n' '{\"delta\":\"partial\"}'",
            "cat >/dev/null; printf '%s\\n' '{\"type\":\"result\",\"result\":\"done\"}'; exit 1",
        ] {
            let (stream, _) = stream_json_command(
                Command::new("/bin/sh"),
                &["-c".into(), script.into()],
                "synthetic".into(),
                |v| v["delta"].as_str().map(str::to_owned),
                |_| AdapterError::ProcessFailed,
            )
            .await
            .unwrap();
            assert!(matches!(
                crate::collect_events(stream).await,
                Err(AdapterError::ProcessFailed)
            ));
        }
    }
}
