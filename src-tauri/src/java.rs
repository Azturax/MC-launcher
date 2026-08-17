use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: String,
    pub version: String,
    pub vendor: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub recommended_mb: u64,
}

pub fn system_memory() -> MemoryInfo {
    let mut sys = System::new();
    sys.refresh_memory();
    let total_mb = sys.total_memory() / 1024 / 1024;
    let recommended = if total_mb > 16384 {
        4096
    } else if total_mb > 8192 {
        2048
    } else {
        1024
    };
    MemoryInfo {
        total_mb,
        recommended_mb: recommended.min(total_mb.saturating_sub(1024)).max(512),
    }
}

pub fn resolve_bin(instance_path: Option<&str>, override_path: Option<&str>) -> AppResult<String> {
    if let Some(path) = override_path.filter(|s| !s.is_empty()) {
        return Ok(path.to_string());
    }
    if let Some(path) = instance_path.filter(|s| !s.is_empty()) {
        return Ok(path.to_string());
    }
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let bin = Path::new(&home).join(java_bin());
        if bin.exists() {
            return Ok(bin.to_string_lossy().into_owned());
        }
    }
    which::which("java")
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|_| AppError::msg("No Java on PATH. Discover a JDK in Settings."))
}

pub fn discover() -> AppResult<Vec<JavaInstall>> {
    let mut found: Vec<JavaInstall> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        push_java(&mut found, &mut seen, Path::new(&home).join(java_bin()), "JAVA_HOME");
    }

    if let Ok(path) = which::which("java") {
        push_java(&mut found, &mut seen, path, "PATH");
    }

    for root in candidate_roots() {
        if !root.exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let bin = entry.path().join(java_bin());
                if bin.exists() {
                    push_java(&mut found, &mut seen, bin, root.to_string_lossy().as_ref());
                }
                let nested = entry.path().join("bin").join(if cfg!(windows) {
                    "java.exe"
                } else {
                    "java"
                });
                if nested.exists() {
                    push_java(
                        &mut found,
                        &mut seen,
                        nested,
                        root.to_string_lossy().as_ref(),
                    );
                }
            }
        }
    }

    Ok(found)
}

fn java_bin() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("bin").join("java.exe")
    } else {
        PathBuf::from("bin").join("java")
    }
}

fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        for base in [
            r"C:\Program Files\Java",
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\Amazon Corretto",
            r"C:\Program Files\BellSoft",
            r"C:\Program Files\AdoptOpenJDK",
            r"C:\Program Files\Eclipse Foundation",
            r"C:\Program Files (x86)\Java",
            r"C:\Program Files\Microsoft\jdk-21",
        ] {
            roots.push(PathBuf::from(base));
        }
    } else if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        roots.push(PathBuf::from(
            "/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home",
        ));
    } else {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/usr/lib64/jvm"));
    }
    roots
}

fn push_java(
    found: &mut Vec<JavaInstall>,
    seen: &mut std::collections::HashSet<String>,
    path: PathBuf,
    source: &str,
) {
    let Ok(canon) = path.canonicalize() else {
        return;
    };
    let key = canon.to_string_lossy().to_string();
    if !seen.insert(key.clone()) {
        return;
    }
    if let Some(meta) = probe(&canon) {
        found.push(JavaInstall {
            path: key,
            version: meta.0,
            vendor: meta.1,
            source: source.to_string(),
        });
    }
}

fn probe(java: &Path) -> Option<(String, Option<String>)> {
    let output = Command::new(java).arg("-version").output().ok()?;
    let text = if output.stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    let first = text.lines().next().unwrap_or("unknown").trim().to_string();
    let vendor = if text.to_ascii_lowercase().contains("temurin") {
        Some("Eclipse Temurin".into())
    } else if text.to_ascii_lowercase().contains("zulu") {
        Some("Azul Zulu".into())
    } else if text.to_ascii_lowercase().contains("microsoft") {
        Some("Microsoft".into())
    } else if text.to_ascii_lowercase().contains("graal") {
        Some("GraalVM".into())
    } else if text.to_ascii_lowercase().contains("openjdk") {
        Some("OpenJDK".into())
    } else {
        None
    };
    Some((first, vendor))
}
