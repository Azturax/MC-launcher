use crate::auth::SessionDescriptor;
use crate::error::{AppError, AppResult};
use crate::instances::Instance;
use crate::java;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub instance_id: String,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchExit {
    pub instance_id: String,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStarted {
    pub instance_id: String,
    pub pid: u32,
}

#[derive(Clone, Default)]
pub struct LaunchTable {
    inner: Arc<Mutex<HashMap<String, u32>>>,
}

impl LaunchTable {
    pub fn insert(&self, instance_id: &str, pid: u32) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(instance_id.to_string(), pid);
        }
    }

    pub fn remove(&self, instance_id: &str) -> Option<u32> {
        self.inner.lock().ok()?.remove(instance_id)
    }

    pub fn get(&self, instance_id: &str) -> Option<u32> {
        self.inner.lock().ok()?.get(instance_id).copied()
    }

    pub fn running_ids(&self) -> Vec<String> {
        self.inner
            .lock()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }
}

pub fn launch(
    app: &AppHandle,
    instance: &Instance,
    session: &SessionDescriptor,
    cache_dir: &Path,
    java_override: Option<&str>,
    extra_jvm: Option<&str>,
    launches: &LaunchTable,
) -> AppResult<u32> {
    let game_dir = PathBuf::from(&instance.game_dir);
    let version_id = effective_version_id(instance);
    let version_json = load_version_json(&game_dir, instance, &version_id)?;
    let vanilla_json = load_vanilla_json(&game_dir, &instance.game_version)?;

    let java = java::resolve_bin(instance.java_path.as_deref(), java_override)?;
    let natives = game_dir.join("natives");
    std::fs::create_dir_all(game_dir.join("logs"))?;
    std::fs::create_dir_all(game_dir.join("crash-reports"))?;

    let classpath = build_classpath(&version_json, &vanilla_json, cache_dir, &game_dir, instance)?;
    let main_class = version_json
        .get("mainClass")
        .and_then(|v| v.as_str())
        .or_else(|| vanilla_json.get("mainClass").and_then(|v| v.as_str()))
        .ok_or_else(|| AppError::msg("Version JSON is missing mainClass — install first"))?
        .to_string();

    let assets_dir = cache_dir.join("assets");
    let asset_index = vanilla_json
        .pointer("/assetIndex/id")
        .and_then(|v| v.as_str())
        .unwrap_or("legacy")
        .to_string();

    let access_token = session
        .access_token
        .clone()
        .unwrap_or_else(|| "0".to_string());

    let mut replacements = HashMap::new();
    replacements.insert("${auth_player_name}".into(), session.name.clone());
    replacements.insert("${version_name}".into(), version_id.clone());
    replacements.insert(
        "${game_directory}".into(),
        game_dir.to_string_lossy().into_owned(),
    );
    replacements.insert(
        "${assets_root}".into(),
        assets_dir.to_string_lossy().into_owned(),
    );
    replacements.insert("${game_assets}".into(), assets_dir.to_string_lossy().into_owned());
    replacements.insert("${assets_index_name}".into(), asset_index);
    replacements.insert("${auth_uuid}".into(), session.uuid.clone());
    replacements.insert("${auth_access_token}".into(), access_token.clone());
    replacements.insert("${auth_session}".into(), access_token);
    replacements.insert("${user_type}".into(), session.user_type.clone());
    replacements.insert(
        "${version_type}".into(),
        vanilla_json
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("release")
            .to_string(),
    );
    replacements.insert(
        "${natives_directory}".into(),
        natives.to_string_lossy().into_owned(),
    );
    replacements.insert("${launcher_name}".into(), "Aureum".into());
    replacements.insert("${launcher_version}".into(), env!("CARGO_PKG_VERSION").into());
    replacements.insert("${classpath}".into(), classpath);
    replacements.insert("${user_properties}".into(), "{}".into());
    replacements.insert("${clientid}".into(), "aureum".into());
    replacements.insert("${auth_xuid}".into(), "0".into());

    let mut args: Vec<String> = vec![
        format!("-Xms{}M", (instance.memory_mb / 2).max(512)),
        format!("-Xmx{}M", instance.memory_mb),
        format!("-Djava.library.path={}", natives.display()),
    ];

    if let Some(extra) = extra_jvm.filter(|s| !s.is_empty()) {
        args.extend(split_args(extra));
    }
    if let Some(extra) = &instance.jvm_args {
        args.extend(split_args(extra));
    }

    args.extend(collect_args(vanilla_json.get("arguments"), "jvm", &replacements));
    if version_json.get("arguments") != vanilla_json.get("arguments") {
        args.extend(collect_args(version_json.get("arguments"), "jvm", &replacements));
    }

    if !args.iter().any(|a| a == "-cp" || a == "-classpath") {
        args.push("-cp".into());
        args.push(replacements["${classpath}"].clone());
    }
    args.push(main_class);

    let mut game_args = collect_args(vanilla_json.get("arguments"), "game", &replacements);
    if version_json.get("arguments") != vanilla_json.get("arguments") {
        game_args.extend(collect_args(version_json.get("arguments"), "game", &replacements));
    }
    if game_args.is_empty() {
        if let Some(legacy) = vanilla_json
            .get("minecraftArguments")
            .and_then(|v| v.as_str())
        {
            args.extend(interpolate_legacy(legacy, &replacements));
        } else {
            args.extend([
                "--username".into(),
                session.name.clone(),
                "--version".into(),
                version_id,
                "--gameDir".into(),
                game_dir.to_string_lossy().into_owned(),
                "--assetsDir".into(),
                assets_dir.to_string_lossy().into_owned(),
                "--assetIndex".into(),
                replacements["${assets_index_name}"].clone(),
                "--uuid".into(),
                session.uuid.clone(),
                "--accessToken".into(),
                replacements["${auth_access_token}"].clone(),
                "--userType".into(),
                session.user_type.clone(),
            ]);
        }
    } else {
        args.extend(game_args);
    }

    let mut cmd = Command::new(&java);
    cmd.args(&args)
        .current_dir(&game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::msg(format!("Failed to start Java ({java}): {e}")))?;
    let pid = child.id();
    launches.insert(&instance.id, pid);
    let _ = app.emit(
        "launch-started",
        LaunchStarted {
            instance_id: instance.id.clone(),
            pid,
        },
    );

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let log_path = game_dir.join("logs").join("aureum-launch.log");
    let app_out = app.clone();
    let id_out = instance.id.clone();
    let log_out = log_path.clone();
    if let Some(out) = stdout {
        thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                append_log_file(&log_out, &line);
                let _ = app_out.emit(
                    "launch-log",
                    LogLine {
                        instance_id: id_out.clone(),
                        stream: "stdout".into(),
                        line,
                    },
                );
            }
        });
    }
    let app_err = app.clone();
    let id_err = instance.id.clone();
    if let Some(err) = stderr {
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                append_log_file(&log_path, &line);
                let _ = app_err.emit(
                    "launch-log",
                    LogLine {
                        instance_id: id_err.clone(),
                        stream: "stderr".into(),
                        line,
                    },
                );
            }
        });
    }

    let app_wait = app.clone();
    let id_wait = instance.id.clone();
    let keep_open = instance.keep_open;
    let table = launches.clone();
    thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        table.remove(&id_wait);
        let _ = app_wait.emit(
            "launch-exited",
            LaunchExit {
                instance_id: id_wait,
                code,
            },
        );
        if !keep_open {
            if let Some(window) = app_wait.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });

    if !instance.keep_open {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    Ok(pid)
}

pub fn stop(launches: &LaunchTable, instance_id: &str) -> AppResult<()> {
    let Some(pid) = launches.get(instance_id) else {
        return Err(AppError::msg("That instance is not running"));
    };
    // Kill the Java process tree for this instance only (Windows: taskkill /T).
    kill_pid(pid)?;
    let _ = launches.remove(instance_id);
    Ok(())
}

pub fn list_running(launches: &LaunchTable) -> Vec<String> {
    launches.running_ids()
}

pub fn tail_log(instance: &Instance, lines: usize) -> AppResult<Vec<String>> {
    let logs = PathBuf::from(&instance.game_dir).join("logs");
    let mut collected = Vec::new();
    for name in ["latest.log", "aureum-launch.log"] {
        let path = logs.join(name);
        if path.is_file() {
            if let Ok(text) = std::fs::read_to_string(path) {
                collected.extend(text.lines().map(|s| s.to_string()));
            }
        }
    }
    let start = collected.len().saturating_sub(lines);
    Ok(collected[start..].to_vec())
}

fn append_log_file(path: &Path, line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

pub fn crash_reports_dir(instance: &Instance) -> PathBuf {
    PathBuf::from(&instance.game_dir).join("crash-reports")
}

fn effective_version_id(instance: &Instance) -> String {
    match instance.loader.as_str() {
        "vanilla" => instance.game_version.clone(),
        other => format!("{}-{}", other, instance.game_version),
    }
}

fn load_version_json(
    game_dir: &Path,
    instance: &Instance,
    version_id: &str,
) -> AppResult<serde_json::Value> {
    let path = game_dir
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"));
    if path.is_file() {
        return Ok(serde_json::from_slice(&std::fs::read(path)?)?);
    }
    load_vanilla_json(game_dir, &instance.game_version)
}

fn load_vanilla_json(game_dir: &Path, game_version: &str) -> AppResult<serde_json::Value> {
    let path = game_dir
        .join("versions")
        .join(game_version)
        .join(format!("{game_version}.json"));
    if !path.is_file() {
        return Err(AppError::msg(
            "Instance is not installed yet. Use Install first.",
        ));
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn build_classpath(
    version_json: &serde_json::Value,
    vanilla_json: &serde_json::Value,
    cache_dir: &Path,
    game_dir: &Path,
    instance: &Instance,
) -> AppResult<String> {
    let mut entries = Vec::new();
    collect_libs(version_json, cache_dir, game_dir, &mut entries);
    collect_libs(vanilla_json, cache_dir, game_dir, &mut entries);
    let client = game_dir
        .join("versions")
        .join(&instance.game_version)
        .join(format!("{}.jar", instance.game_version));
    if client.is_file() {
        entries.push(client);
    }
    entries.sort();
    entries.dedup();
    let sep = if cfg!(windows) { ";" } else { ":" };
    Ok(entries
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(sep))
}

fn collect_libs(json: &serde_json::Value, cache_dir: &Path, game_dir: &Path, out: &mut Vec<PathBuf>) {
    let Some(libs) = json.get("libraries").and_then(|v| v.as_array()) else {
        return;
    };
    for lib in libs {
        let rel = if let Some(path) = lib.pointer("/downloads/artifact/path").and_then(|v| v.as_str())
        {
            PathBuf::from(path)
        } else if let Some(name) = lib.get("name").and_then(|v| v.as_str()) {
            maven_path(name)
        } else {
            continue;
        };
        let cached = cache_dir.join("libraries").join(&rel);
        if cached.is_file() {
            out.push(cached);
        } else {
            out.push(game_dir.join("libraries").join(rel));
        }
    }
}

fn maven_path(name: &str) -> PathBuf {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return PathBuf::from(name);
    }
    let file = if parts.len() >= 4 {
        format!("{}-{}-{}.jar", parts[1], parts[2], parts[3])
    } else {
        format!("{}-{}.jar", parts[1], parts[2])
    };
    PathBuf::from(parts[0].replace('.', "/"))
        .join(parts[1])
        .join(parts[2])
        .join(file)
}

fn collect_args(
    arguments: Option<&serde_json::Value>,
    side: &str,
    replacements: &HashMap<String, String>,
) -> Vec<String> {
    let Some(list) = arguments.and_then(|a| a.get(side)).and_then(|v| v.as_array()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for item in list {
        if let Some(s) = item.as_str() {
            out.push(replace_tokens(s, replacements));
            continue;
        }
        if !rules_allow(item.get("rules")) {
            continue;
        }
        match item.get("value") {
            Some(serde_json::Value::String(s)) => out.push(replace_tokens(s, replacements)),
            Some(serde_json::Value::Array(arr)) => {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        out.push(replace_tokens(s, replacements));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn rules_allow(rules: Option<&serde_json::Value>) -> bool {
    let Some(rules) = rules.and_then(|v| v.as_array()) else {
        return true;
    };
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };
    let mut allow = false;
    for rule in rules {
        let action = rule["action"].as_str().unwrap_or("allow");
        if let Some(features) = rule.get("features").and_then(|v| v.as_object()) {
            // Demo, quick-play, and custom resolution stay off unless we set them.
            if features.values().any(|v| v.as_bool() == Some(true)) {
                continue;
            }
        }
        let os_ok = match rule.get("os") {
            None => true,
            Some(spec) => spec
                .get("name")
                .and_then(|v| v.as_str())
                .is_none_or(|n| n == os),
        };
        if os_ok {
            allow = action == "allow";
        }
    }
    allow
}

fn interpolate_legacy(raw: &str, replacements: &HashMap<String, String>) -> Vec<String> {
    raw.split_whitespace()
        .map(|s| replace_tokens(s, replacements))
        .collect()
}

fn replace_tokens(input: &str, replacements: &HashMap<String, String>) -> String {
    let mut out = input.to_string();
    for (k, v) in replacements {
        out = out.replace(k, v);
    }
    out
}

fn split_args(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut quote = None;
    for c in raw.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                buf.push(c);
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c.is_whitespace() {
            if !buf.is_empty() {
                out.push(std::mem::take(&mut buf));
            }
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

fn kill_pid(pid: u32) -> AppResult<()> {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|e| AppError::msg(format!("taskkill failed to start: {e}")))?;
        // 128 = process not found — treat as already stopped.
        if !status.success() && status.code() != Some(128) {
            return Err(AppError::msg(format!(
                "taskkill failed (exit {:?})",
                status.code()
            )));
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        // Terminate the process group when possible so child JVMs die with the parent.
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .status();
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|e| AppError::msg(format!("kill failed to start: {e}")))?;
        if !status.success() {
            // Fallback hard kill if still alive.
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
        Ok(())
    }
}
