use serde::Serialize;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub struct Manifest {
    pub id: &'static str,
    pub name: &'static str,
    pub executables: &'static [&'static str],
    pub bundles: &'static [&'static str],
}
pub const MANIFESTS: &[Manifest] = &[
    Manifest {
        id: "codex",
        name: "Codex",
        executables: &["codex"],
        bundles: &["Codex"],
    },
    Manifest {
        id: "qoder-cn",
        name: "Qoder CN",
        executables: &["qodercn"],
        bundles: &["Qoder CN"],
    },
    Manifest {
        id: "qoder",
        name: "Qoder",
        executables: &["qoder"],
        bundles: &["Qoder"],
    },
    Manifest {
        id: "codebuddy-cn",
        name: "CodeBuddy CN",
        executables: &[],
        bundles: &["CodeBuddy CN"],
    },
    Manifest {
        id: "codebuddy",
        name: "CodeBuddy Code",
        executables: &["codebuddy", "cbc"],
        bundles: &["CodeBuddy"],
    },
    Manifest {
        id: "workbuddy",
        name: "WorkBuddy",
        executables: &[],
        bundles: &["WorkBuddy"],
    },
    Manifest {
        id: "kimi",
        name: "Kimi Code",
        executables: &["kimi"],
        bundles: &["Kimi", "Kimi Code"],
    },
    Manifest {
        id: "claude",
        name: "Claude Code",
        executables: &["claude"],
        bundles: &["Claude"],
    },
    Manifest {
        id: "gemini",
        name: "Gemini CLI",
        executables: &["gemini"],
        bundles: &["Gemini"],
    },
    Manifest {
        id: "opencode",
        name: "OpenCode",
        executables: &["opencode"],
        bundles: &["OpenCode"],
    },
    Manifest {
        id: "qwen",
        name: "Qwen Code",
        executables: &["qwen"],
        bundles: &["Qwen"],
    },
    Manifest {
        id: "zcode",
        name: "ZCode",
        executables: &["zcode"],
        bundles: &["ZCode"],
    },
    Manifest {
        id: "trae",
        name: "TRAE",
        executables: &["trae"],
        bundles: &["TRAE"],
    },
    Manifest {
        id: "mimo",
        name: "Xiaomi MiMo",
        executables: &["mimo"],
        bundles: &["Xiaomi MiMo"],
    },
    Manifest {
        id: "ollama",
        name: "Ollama",
        executables: &[],
        bundles: &["Ollama"],
    },
];
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Connectability {
    DetectedOnly,
    SupportedVersionUnknown,
    BrokenInstall,
}
#[derive(Clone, Debug, Serialize)]
pub struct DetectedApplication {
    pub app_id: String,
    pub name: String,
    pub version: Option<String>,
    pub executable: Option<String>,
    pub install_path: String,
    pub detection_source: String,
    pub connectability: Connectability,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ScanReport {
    pub applications: Vec<DetectedApplication>,
    pub scanned_roots: Vec<String>,
}

pub fn scan() -> ScanReport {
    scan_with_roots(&default_roots(), MANIFESTS)
}
pub fn scan_with_roots(roots: &[PathBuf], manifests: &[Manifest]) -> ScanReport {
    let mut applications = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for manifest in manifests {
            for executable in manifest.executables {
                for path in candidates(root, executable) {
                    if !path.is_file() {
                        continue;
                    }
                    let path = path.canonicalize().unwrap_or(path);
                    if !seen.insert((manifest.id, path.clone())) {
                        continue;
                    }
                    applications.push(DetectedApplication {
                        app_id: manifest.id.into(),
                        name: manifest.name.into(),
                        version: None,
                        executable: Some(path.to_string_lossy().into_owned()),
                        install_path: root.to_string_lossy().into_owned(),
                        detection_source: "executable".into(),
                        connectability: Connectability::DetectedOnly,
                        diagnostics: vec!["发现可执行文件；尚未验证连接和登录".into()],
                    });
                }
            }
        }
        scan_bundles(root, manifests, &mut seen, &mut applications);
    }
    #[cfg(target_os = "windows")]
    scan_registry(manifests, &mut seen, &mut applications);
    ScanReport {
        applications,
        scanned_roots: roots
            .iter()
            .filter(|root| root.is_dir())
            .map(|root| root.to_string_lossy().into_owned())
            .collect(),
    }
}
fn candidates(root: &Path, executable: &str) -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        ["exe", "cmd", "bat"]
            .iter()
            .map(|ext| root.join(format!("{executable}.{ext}")))
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![root.join(executable)]
    }
}
fn default_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default();
    #[cfg(target_os = "macos")]
    {
        roots.extend(
            [
                "/Applications",
                "/usr/local/bin",
                "/opt/homebrew/bin",
                "/Applications/ChatGPT.app/Contents/Resources",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
        if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
            for path in [
                "Applications",
                "Applications/ChatGPT.app/Contents/Resources",
                ".local/bin",
                ".qoder/entry",
                ".qoder-cn/entry",
                ".mimocode/bin",
                ".cargo/bin",
                "Library/pnpm",
                ".volta/bin",
                ".asdf/shims",
                ".local/share/mise/shims",
            ] {
                roots.push(home.join(path));
            }
            for path in [".nvm/versions/node", ".local/share/mise/installs/node"] {
                if let Ok(entries) = fs::read_dir(home.join(path)) {
                    for entry in entries.flatten() {
                        roots.push(entry.path().join("bin"));
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        for key in [
            "PROGRAMFILES",
            "PROGRAMFILES(X86)",
            "LOCALAPPDATA",
            "APPDATA",
        ] {
            if let Some(root) = env::var_os(key).map(PathBuf::from) {
                roots.push(root.clone());
                if key == "LOCALAPPDATA" {
                    roots.push(root.join("Programs"));
                }
                if key == "APPDATA" {
                    roots.push(root.join("npm"));
                }
                if key.starts_with("PROGRAMFILES") || key == "LOCALAPPDATA" {
                    if let Ok(entries) = fs::read_dir(&root) {
                        for entry in entries.flatten() {
                            if entry.path().is_dir() {
                                roots.push(entry.path());
                            }
                        }
                    }
                }
            }
        }
        if let Some(home) = env::var_os("USERPROFILE").map(PathBuf::from) {
            roots.push(home.join(".cargo/bin"));
            let scoop = home.join("scoop/apps");
            if let Ok(entries) = fs::read_dir(scoop) {
                for entry in entries.flatten() {
                    roots.push(entry.path().join("current"));
                }
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

#[cfg(target_os = "macos")]
fn bundle_matches(manifest: &Manifest, stem: &str, bundle_id: &str) -> bool {
    manifest
        .bundles
        .iter()
        .any(|name| name.eq_ignore_ascii_case(stem))
        || bundle_id.split('.').any(|segment| {
            segment.eq_ignore_ascii_case(manifest.id)
                || (manifest.id == "qoder-cn" && segment.eq_ignore_ascii_case("qodercn"))
                || (manifest.id == "codebuddy-cn" && segment.eq_ignore_ascii_case("codebuddycn"))
        })
}

#[cfg(target_os = "macos")]
fn scan_bundles(
    root: &Path,
    manifests: &[Manifest],
    seen: &mut HashSet<(&'static str, PathBuf)>,
    output: &mut Vec<DetectedApplication>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path().canonicalize().unwrap_or_else(|_| entry.path());
        if path.extension().and_then(|ext| ext.to_str()) != Some("app") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        let plist = plist::Value::from_file(path.join("Contents/Info.plist")).ok();
        let dict = plist.as_ref().and_then(plist::Value::as_dictionary);
        let bundle_id = dict
            .and_then(|dict| dict.get("CFBundleIdentifier"))
            .and_then(plist::Value::as_string)
            .unwrap_or("");
        let version = dict
            .and_then(|dict| dict.get("CFBundleShortVersionString"))
            .and_then(plist::Value::as_string)
            .map(str::to_owned);
        let executable_name = dict
            .and_then(|dict| dict.get("CFBundleExecutable"))
            .and_then(plist::Value::as_string);
        for manifest in manifests {
            if !bundle_matches(manifest, stem, bundle_id) {
                continue;
            }
            if !seen.insert((manifest.id, path.clone())) {
                continue;
            }
            let executable = executable_name
                .map(|name| path.join("Contents/MacOS").join(name))
                .filter(|path| path.is_file());
            output.push(DetectedApplication {
                app_id: manifest.id.into(),
                name: manifest.name.into(),
                version: version.clone(),
                executable: executable
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                install_path: path.to_string_lossy().into_owned(),
                detection_source: "application_bundle".into(),
                connectability: if executable.is_some() {
                    Connectability::DetectedOnly
                } else {
                    Connectability::BrokenInstall
                },
                diagnostics: if executable.is_some() {
                    vec!["发现应用包；尚未验证连接和登录".into()]
                } else {
                    vec!["应用包缺少可执行文件".into()]
                },
            });
            if manifest.id == "workbuddy" {
                output
                    .last_mut()
                    .unwrap()
                    .diagnostics
                    .push("WorkBuddy 桌面版未提供可验证的官方模型 CLI".into());
            }
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn scan_bundles(
    _root: &Path,
    _manifests: &[Manifest],
    _seen: &mut HashSet<(&'static str, PathBuf)>,
    _output: &mut Vec<DetectedApplication>,
) {
}

#[cfg(target_os = "windows")]
fn scan_registry(
    manifests: &[Manifest],
    seen: &mut HashSet<(&'static str, PathBuf)>,
    output: &mut Vec<DetectedApplication>,
) {
    use winreg::{enums::*, RegKey};
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        for subkey in [
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ] {
            let Ok(uninstall) = root.open_subkey(subkey) else {
                continue;
            };
            for key in uninstall.enum_keys().flatten() {
                let Ok(item) = uninstall.open_subkey(key) else {
                    continue;
                };
                let Ok(name): Result<String, _> = item.get_value("DisplayName") else {
                    continue;
                };
                let version: Option<String> = item.get_value("DisplayVersion").ok();
                let location: Option<String> = item.get_value("InstallLocation").ok();
                for manifest in manifests {
                    if !manifest.bundles.iter().any(|known| {
                        name.to_ascii_lowercase()
                            .contains(&known.to_ascii_lowercase())
                    }) {
                        continue;
                    }
                    let path = location.as_deref().map(PathBuf::from).unwrap_or_default();
                    if !seen.insert((manifest.id, path.clone())) {
                        break;
                    }
                    output.push(DetectedApplication {
                        app_id: manifest.id.into(),
                        name: manifest.name.into(),
                        version: version.clone(),
                        executable: None,
                        install_path: path.to_string_lossy().into_owned(),
                        detection_source: "windows_registry".into(),
                        connectability: Connectability::DetectedOnly,
                        diagnostics: vec!["发现安装记录；尚未验证可执行文件和登录".into()],
                    });
                    break;
                }
            }
        }
        if let Ok(app_paths) =
            root.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\App Paths")
        {
            for manifest in manifests {
                for executable in manifest.executables {
                    let Ok(item) = app_paths.open_subkey(format!("{executable}.exe")) else {
                        continue;
                    };
                    let Ok(value): Result<String, _> = item.get_value("") else {
                        continue;
                    };
                    let path = PathBuf::from(value);
                    if !path.is_file() || !seen.insert((manifest.id, path.clone())) {
                        continue;
                    }
                    output.push(DetectedApplication {
                        app_id: manifest.id.into(),
                        name: manifest.name.into(),
                        version: None,
                        executable: Some(path.to_string_lossy().into_owned()),
                        install_path: path
                            .parent()
                            .unwrap_or(Path::new(""))
                            .to_string_lossy()
                            .into_owned(),
                        detection_source: "windows_app_paths".into(),
                        connectability: Connectability::DetectedOnly,
                        diagnostics: vec!["发现 App Paths 注册；尚未验证连接和登录".into()],
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_executable_without_reading_content() {
        let root = env::temp_dir().join(format!("localmux-discovery-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = candidates(&root, "codex").remove(0);
        fs::write(&path, b"not a real CLI").unwrap();
        let report = scan_with_roots(&[root.clone(), root.clone()], &MANIFESTS[..1]);
        assert_eq!(report.applications.len(), 1);
        assert_eq!(report.applications[0].app_id, "codex");
        assert!(report.applications[0].version.is_none());
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundle_id_requires_exact_segment() {
        let codex = &MANIFESTS[0];
        assert!(bundle_matches(codex, "ChatGPT", "com.openai.codex"));
        assert!(!bundle_matches(
            codex,
            "CodexMenuMeter",
            "com.codexkit.CodexMenuMeter"
        ));
        let mimo = MANIFESTS
            .iter()
            .find(|manifest| manifest.id == "mimo")
            .unwrap();
        assert!(bundle_matches(
            mimo,
            "Xiaomi MiMo",
            "com.xiaomi.mimo.desktop"
        ));
        assert!(!bundle_matches(mimo, "MImoMeter", "com.example.mimometer"));
        let qoder = MANIFESTS
            .iter()
            .find(|manifest| manifest.id == "qoder")
            .unwrap();
        let qoder_cn = MANIFESTS
            .iter()
            .find(|manifest| manifest.id == "qoder-cn")
            .unwrap();
        assert!(bundle_matches(qoder, "Qoder", "com.qoder.app"));
        assert!(!bundle_matches(qoder, "Qoder CN", "com.qodercn.app"));
        assert!(bundle_matches(qoder_cn, "Qoder CN", "com.qodercn.app"));
        let codebuddy = MANIFESTS
            .iter()
            .find(|manifest| manifest.id == "codebuddy")
            .unwrap();
        let codebuddy_cn = MANIFESTS
            .iter()
            .find(|manifest| manifest.id == "codebuddy-cn")
            .unwrap();
        assert!(!bundle_matches(
            codebuddy,
            "CodeBuddy CN",
            "com.tencent.codebuddycn"
        ));
        assert!(bundle_matches(
            codebuddy_cn,
            "CodeBuddy CN",
            "com.tencent.codebuddycn"
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn workbuddy_bundle_does_not_claim_a_model_cli() {
        let root = tempfile::tempdir().unwrap();
        let bundle = root.path().join("WorkBuddy.app/Contents");
        let macos = bundle.join("MacOS");
        let cli = bundle.join("Resources/app.asar.unpacked/cli/bin/codebuddy");
        fs::create_dir_all(&macos).unwrap();
        fs::create_dir_all(cli.parent().unwrap()).unwrap();
        fs::write(macos.join("Electron"), b"").unwrap();
        fs::write(&cli, b"").unwrap();
        let mut info = plist::Dictionary::new();
        info.insert("CFBundleExecutable".into(), "Electron".into());
        plist::Value::Dictionary(info)
            .to_file_xml(bundle.join("Info.plist"))
            .unwrap();
        let report = scan_with_roots(&[root.path().to_path_buf()], MANIFESTS);
        assert!(report
            .applications
            .iter()
            .any(|app| app.app_id == "workbuddy" && app.detection_source == "application_bundle"));
        assert!(!report
            .applications
            .iter()
            .any(|app| app.app_id == "codebuddy"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn scans_official_qoder_cli_entry_directory() {
        let home = PathBuf::from(env::var_os("HOME").unwrap());
        assert!(default_roots().contains(&home.join(".qoder/entry")));
        assert!(default_roots().contains(&home.join(".qoder-cn/entry")));
        assert!(default_roots().contains(&home.join(".mimocode/bin")));
    }
}
