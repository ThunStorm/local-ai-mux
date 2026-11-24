//! Local snapshot export. No provider calls, raw errors, or frontend-supplied paths.
use localmux_storage::{CallExport, CallFilter};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri_plugin_dialog::DialogExt;

static EXPORTING: AtomicBool = AtomicBool::new(false);
struct ExportGuard;
impl Drop for ExportGuard {
    fn drop(&mut self) {
        EXPORTING.store(false, Ordering::Release);
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Md,
    Xlsx,
    Csv,
    Txt,
    Html,
}
impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Md => "md",
            Self::Xlsx => "xlsx",
            Self::Csv => "csv",
            Self::Txt => "txt",
            Self::Html => "html",
        }
    }
}
#[derive(Serialize)]
pub struct ExportResult {
    path: String,
    count: usize,
    recorded_count: usize,
}

#[tauri::command]
pub async fn call_records_export(
    app: tauri::AppHandle,
    control: tauri::State<'_, super::GatewayControl>,
    filter: CallFilter,
    format: ExportFormat,
) -> Result<Option<ExportResult>, String> {
    filter.validate().map_err(|_| "INVALID_TIME_RANGE")?;
    EXPORTING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "EXPORT_BUSY")?;
    let _guard = ExportGuard;
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter(format.extension(), &[format.extension()])
            .set_file_name(format!("localmux-sessions.{}", format.extension()))
            .blocking_save_file()
    })
    .await
    .map_err(|_| "EXPORT_DIALOG_FAILED")?;
    let Some(path) = path else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|_| "EXPORT_LOCAL_PATH_REQUIRED")?;
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case(format.extension()))
    {
        return Err("EXPORT_EXTENSION_MISMATCH".into());
    }
    let export = localmux_storage::export_calls(&control.state.pool, &filter)
        .await
        .map_err(|e| {
            if e.to_string() == "EXPORT_TOO_LARGE" {
                "EXPORT_TOO_LARGE"
            } else {
                "EXPORT_READ_FAILED"
            }
        })?;
    let count = export.calls.len();
    let recorded_count = export.calls.iter().filter(|c| c.content.is_some()).count();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = render(&export, format).map_err(|_| "EXPORT_FORMAT_FAILED")?;
        atomic_save(&path, &bytes).map_err(|_| "EXPORT_WRITE_FAILED")?;
        Ok(Some(ExportResult {
            path: path.to_string_lossy().into_owned(),
            count,
            recorded_count,
        }))
    })
    .await
    .map_err(|_| "EXPORT_WRITE_FAILED")?
}

fn atomic_save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if path.symlink_metadata().is_ok_and(|m| !m.is_file()) {
        return Err(std::io::Error::other("destination is not a regular file"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    // tempfile creates owner-only Unix files; persist replaces only after the complete write.
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn utc(value: i64) -> Option<String> {
    time::OffsetDateTime::from_unix_timestamp(value)
        .ok()?
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}
fn public_snapshot(export: &CallExport) -> Result<Value, serde_json::Error> {
    let mut v = serde_json::to_value(export)?;
    v["exported_at_utc"] = json!(utc(export.exported_at));
    for call in v["calls"].as_array_mut().unwrap() {
        for field in ["started_at", "finished_at"] {
            call["record"][format!("{field}_utc")] =
                json!(call["record"][field].as_i64().and_then(utc));
        }
        call["record"]
            .as_object_mut()
            .unwrap()
            .remove("owner_key_id");
    }
    v["time_basis"] = json!("UTC Unix seconds; started_from inclusive, started_before exclusive");
    v["notes"]=json!(["Only previously recorded content; missing content is null, never reconstructed.","Running/partial/stopped content is a snapshot, not a completed conversation.","CSV prefixes risky spreadsheet cells with an apostrophe; JSON preserves exact stored text."]);
    Ok(v)
}
fn cell(v: &Value) -> String {
    if v.is_null() {
        String::new()
    } else if let Some(s) = v.as_str() {
        s.to_owned()
    } else {
        v.to_string()
    }
}
fn escaped(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn fenced(s: &str) -> String {
    let longest = s.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest + 1));
    format!("{fence}text\n{s}\n{fence}\n")
}
fn dialogue(call: &Value) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let c = &call["content"];
    if let Some(messages) = c["messages"].as_array() {
        for m in messages {
            result.push((
                m["role"].as_str().unwrap_or("input").into(),
                cell(&m["content"]),
            ));
        }
    } else if !c.is_null() {
        result.push(("input".into(), cell(&c["messages"])));
    }
    if !c.is_null() {
        result.push(("assistant (captured answer)".into(), cell(&c["answer"])));
    }
    result
}
fn render(
    export: &CallExport,
    format: ExportFormat,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let v = public_snapshot(export)?;
    let bytes = match format {
        ExportFormat::Json => serde_json::to_vec_pretty(&v)?,
        ExportFormat::Xlsx => xlsx(&v)?,
        ExportFormat::Csv => csv(&v).into_bytes(),
        _ => text_document(&v, format).into_bytes(),
    };
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("export output too large".into());
    }
    Ok(bytes)
}
fn text_document(v: &Value, format: ExportFormat) -> String {
    let html = matches!(format, ExportFormat::Html);
    let md = matches!(format, ExportFormat::Md);
    let intro = format!(
        "LocalAiMux 会话导出 / Session export\n导出时间 exported_at_utc: {}\n筛选 Filter: {}\n{}\n{}\n",
        v["exported_at_utc"], v["filter"], v["time_basis"].as_str().unwrap(), v["notes"],
    );
    let mut s = if html {
        format!("<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\"><title>LocalAiMux 会话导出</title><style>body{{font:15px/1.6 system-ui;margin:32px auto;padding:0 20px;max-width:1000px;color:#18202d}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;background:#f4f5f7;padding:16px}}article{{border-top:1px solid #ccc;margin:28px 0}}h2{{overflow-wrap:anywhere}}</style><h1>LocalAiMux 会话导出</h1><pre>{}</pre>",escaped(&intro))
    } else if md {
        format!("# LocalAiMux 会话导出\n\n{}\n", fenced(&intro))
    } else {
        intro
    };
    for call in v["calls"].as_array().unwrap() {
        let r = &call["record"];
        let title = format!(
            "{} · {} · {}",
            cell(&r["id"]),
            cell(&r["requested_model"]),
            cell(&r["status"])
        );
        let state = call["content"]["state"]
            .as_str()
            .unwrap_or("not_recorded / 未录正文");
        let metadata = format!(
            "录制状态 Recording: {state}\n{}",
            serde_json::to_string_pretty(r).unwrap()
        );
        if html {
            s.push_str(&format!(
                "<article><h2>{}</h2><pre>{}</pre>",
                escaped(&title),
                escaped(&metadata)
            ));
        } else if md {
            s.push_str(&format!(
                "\n## 调用记录\n\n{}\n{}\n",
                fenced(&title),
                fenced(&metadata)
            ));
        } else {
            s.push_str(&format!("\n====================\n{title}\n{metadata}\n"));
        }
        for (role, text) in dialogue(call) {
            if html {
                s.push_str(&format!(
                    "<h3>{}</h3><pre>{}</pre>",
                    escaped(&role),
                    escaped(&text)
                ));
            } else if md {
                s.push_str(&format!("\n{}\n{}\n", fenced(&role), fenced(&text)));
            } else {
                s.push_str(&format!("\n[{role}]\n{text}\n"));
            }
        }
        if html {
            s.push_str("</article>");
        }
    }
    if html {
        s.push_str("</html>");
    }
    s
}
fn csv_field(s: &str) -> String {
    let dangerous =
        s.trim_start().starts_with(['=', '+', '-', '@']) || s.starts_with(['\t', '\r', '\n']);
    let safe = if dangerous { format!("'{s}") } else { s.into() };
    format!("\"{}\"", safe.replace('"', "\"\""))
}
fn csv(v: &Value) -> String {
    let headers = [
        "exported_at_utc_unix",
        "filter_json",
        "call_id",
        "started_at_utc_unix",
        "source",
        "status",
        "requested_model",
        "recording_state",
        "record_json",
        "messages_json",
        "answer",
    ];
    let mut s = String::from("\u{feff}");
    s.push_str(&headers.join(","));
    s.push_str("\r\n");
    for c in v["calls"].as_array().unwrap() {
        let r = &c["record"];
        let values = [
            cell(&v["exported_at"]),
            cell(&v["filter"]),
            cell(&r["id"]),
            cell(&r["started_at"]),
            cell(&r["source"]),
            cell(&r["status"]),
            cell(&r["requested_model"]),
            c["content"]["state"]
                .as_str()
                .unwrap_or("not_recorded")
                .into(),
            r.to_string(),
            cell(&c["content"]["messages"]),
            cell(&c["content"]["answer"]),
        ];
        s.push_str(
            &values
                .iter()
                .map(|v| csv_field(v))
                .collect::<Vec<_>>()
                .join(","),
        );
        s.push_str("\r\n");
    }
    s
}
// Excel's 32767 limit is in UTF-16 units; split on scalar boundaries and preserve every character.
fn chunks(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut part = String::new();
    let mut n = 0;
    for c in s.chars() {
        if n + c.len_utf16() > 30_000 {
            result.push(std::mem::take(&mut part));
            n = 0;
        }
        part.push(c);
        n += c.len_utf16();
    }
    result.push(part);
    result
}
fn xlsx(v: &Value) -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
    let mut book = Workbook::new();
    let header = Format::new().set_bold().set_background_color("#E3EEFF");
    let wrap = Format::new().set_text_wrap();
    let summary = book.add_worksheet();
    summary.set_name("Calls 调用")?;
    summary.set_freeze_panes(1, 0)?;
    let cols = [
        "id",
        "started_at_utc",
        "finished_at_utc",
        "source",
        "status",
        "requested_model",
        "resolved_model",
        "adapter_id",
        "conversation_id",
        "response_id",
        "elapsed_ms",
        "input_tokens",
        "output_tokens",
        "total_tokens",
        "error_code",
        "recording_state",
    ];
    for (i, k) in cols.iter().enumerate() {
        summary.write_string_with_format(0, i as u16, *k, &header)?;
        summary.set_column_width(i as u16, 24)?;
    }
    for (row, c) in v["calls"].as_array().unwrap().iter().enumerate() {
        for (col, key) in cols.iter().enumerate() {
            let value = if *key == "recording_state" {
                c["content"]["state"]
                    .as_str()
                    .unwrap_or("not_recorded")
                    .into()
            } else {
                cell(&c["record"][*key])
            };
            if value.encode_utf16().count() > 30_000 {
                summary.write_string(
                    (row + 1) as u32,
                    col as u16,
                    "完整值见 Content / record_json",
                )?;
            } else if let Some(n) = c["record"][*key]
                .as_i64()
                .filter(|n| n.unsigned_abs() <= 9_007_199_254_740_991)
            {
                summary.write_number((row + 1) as u32, col as u16, n as f64)?;
            } else {
                summary.write_string((row + 1) as u32, col as u16, &value)?;
            }
        }
    }
    summary.autofilter(0, 0, v["calls"].as_array().unwrap().len().max(1) as u32, 15)?;
    let body = book.add_worksheet();
    body.set_name("Content 正文")?;
    body.set_freeze_panes(1, 0)?;
    for (i, h) in [
        "call_id",
        "message_index",
        "role_or_section",
        "part",
        "parts",
        "text",
    ]
    .iter()
    .enumerate()
    {
        body.write_string_with_format(0, i as u16, *h, &header)?;
        body.set_column_width(i as u16, if i == 5 { 90.0 } else { 24.0 })?;
    }
    let mut row = 1;
    for c in v["calls"].as_array().unwrap() {
        let mut blocks = vec![(
            "record_json".into(),
            serde_json::to_string_pretty(&c["record"]).unwrap(),
        )];
        blocks.extend(dialogue(c));
        for (message, (role, text)) in blocks.iter().enumerate() {
            let parts = chunks(text);
            for (index, part) in parts.iter().enumerate() {
                body.write_string(row, 0, cell(&c["record"]["id"]))?;
                body.write_number(row, 1, message as f64)?;
                body.write_string(row, 2, role)?;
                body.write_number(row, 3, (index + 1) as f64)?;
                body.write_number(row, 4, parts.len() as f64)?;
                body.write_string_with_format(row, 5, part, &wrap)?;
                body.set_row_height(row, 60)?;
                row += 1;
            }
        }
    }
    let info = book.add_worksheet();
    info.set_name("About 导出说明")?;
    info.set_column_width(0, 30)?;
    info.set_column_width(1, 100)?;
    let info_rows = [
        ("schema_version", cell(&v["schema_version"])),
        ("exported_at_utc", cell(&v["exported_at_utc"])),
        ("filter_json", cell(&v["filter"])),
        ("time_basis", cell(&v["time_basis"])),
        ("notes", cell(&v["notes"])),
        ("content_parts", "Concatenate text in part order for each call_id/message_index. Text cells are literal strings; blanks mean unknown. Body includes record_json with full diagnostics.".into()),
    ];
    for (row, (key, value)) in info_rows.iter().enumerate() {
        info.write_string_with_format(row as u32, 0, *key, &header)?;
        info.write_string_with_format(row as u32, 1, value, &wrap)?;
        info.set_row_height(row as u32, 48)?;
    }
    book.save_to_buffer()
}

#[cfg(test)]
mod tests {
    use super::*;
    use localmux_storage::{CallContent, CallRecord, ExportCall};
    fn fixture() -> CallExport {
        CallExport {
            schema_version: 1,
            exported_at: 1792500000,
            filter: CallFilter {
                started_from: Some(100),
                ..Default::default()
            },
            calls: vec![
                ExportCall {
                    record: CallRecord {
                        id: "call-fixture".into(),
                        owner_key_id: Some("OWNER_MUST_NOT_EXPORT".into()),
                        requested_model: "fixture/model".into(),
                        source: "api".into(),
                        status: "timeout".into(),
                        started_at: 100,
                        input_tokens: Some(0),
                        ..Default::default()
                    },
                    content: Some(CallContent {
                        messages: json!([{"role":"system","content":"规则\n```\n<script>alert(1)</script>"},{"role":"user","content":"=SUM(1,2)\n中文 😀"}]),
                        answer: format!("=1+1\n{}", "汉😀\r\n".repeat(9000)),
                        state: "partial".into(),
                    }),
                },
                ExportCall {
                    record: CallRecord {
                        id: "no-content".into(),
                        requested_model: "fixture/model".into(),
                        status: "completed".into(),
                        ..Default::default()
                    },
                    content: None,
                },
            ],
        }
    }
    #[test]
    fn six_formats_preserve_content_and_safely_encode_untrusted_text() {
        let export = fixture();
        let json_bytes = render(&export, ExportFormat::Json).unwrap();
        let v: Value = serde_json::from_slice(&json_bytes).unwrap();
        assert!(v["calls"][0]["record"].get("owner_key_id").is_none());
        assert!(v["calls"][1]["content"].is_null());
        assert_eq!(
            v["calls"][0]["content"]["answer"],
            export.calls[0].content.as_ref().unwrap().answer
        );
        for format in [
            ExportFormat::Json,
            ExportFormat::Md,
            ExportFormat::Xlsx,
            ExportFormat::Csv,
            ExportFormat::Txt,
            ExportFormat::Html,
        ] {
            let bytes = render(&export, format).unwrap();
            assert!(!bytes.is_empty());
            if let Ok(dir) = std::env::var("LOCALMUX_EXPORT_FIXTURE_DIR") {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(
                    Path::new(&dir).join(format!("fixture.{}", format.extension())),
                    &bytes,
                )
                .unwrap();
            }
            if matches!(format, ExportFormat::Xlsx) {
                let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
                for name in [
                    "xl/worksheets/sheet1.xml",
                    "xl/worksheets/sheet2.xml",
                    "xl/worksheets/sheet3.xml",
                ] {
                    let mut xml = String::new();
                    std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut xml)
                        .unwrap();
                    assert!(!xml.contains("<f>"));
                }
            } else {
                let text = String::from_utf8(bytes).unwrap();
                assert!(!text.contains("OWNER_MUST_NOT_EXPORT"));
                assert!(text.contains("partial"));
                if matches!(format, ExportFormat::Html) {
                    assert!(!text.contains("<script>"));
                    assert!(text.contains("&lt;script&gt;"));
                }
                if matches!(format, ExportFormat::Csv) {
                    assert!(text.contains("\"'=1+1"));
                }
                if matches!(format, ExportFormat::Md | ExportFormat::Txt) {
                    assert!(text.contains(&export.calls[0].content.as_ref().unwrap().answer));
                }
            }
        }
    }
    #[test]
    fn excel_chunks_roundtrip_unicode_and_csv_never_becomes_a_formula() {
        for text in [
            "😀".repeat(40_000),
            "中文\n\r\t".repeat(10_000),
            String::new(),
        ] {
            let parts = chunks(&text);
            assert_eq!(parts.concat(), text);
            assert!(parts.iter().all(|p| p.encode_utf16().count() <= 30_000));
        }
        for text in ["=1", " +1", "-1", "@SUM(A1)", "\ttext", "\ntext"] {
            assert!(csv_field(text).starts_with("\"'"));
        }
        assert_eq!(csv_field("a,\"b\"\n中文"), "\"a,\"\"b\"\"\n中文\"");
        assert!(fenced("```\nvalue").starts_with("````text"));
    }
    #[test]
    fn atomic_save_replaces_only_after_success_and_keeps_private_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        std::fs::write(&path, b"original").unwrap();
        atomic_save(&path, b"complete").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"complete");
        assert!(atomic_save(&dir.path().join("missing/file.json"), b"failed").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"complete");
        assert!(atomic_save(dir.path(), b"failed").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let link = dir.path().join("link.json");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(atomic_save(&link, b"bad").is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"complete");
        }
    }
}
