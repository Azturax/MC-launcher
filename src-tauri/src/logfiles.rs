//! Path-safe browser for instance `logs/` and `crash-reports/`.
//! Basename-only; no traversal; text preview with size / line caps.

use crate::error::{AppError, AppResult};
use crate::instances::Instance;
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

const MAX_PREVIEW_BYTES: u64 = 512 * 1024; // 512 KiB
const MAX_TAIL_LINES: usize = 400;
const TEXT_EXTS: &[&str] = &["log", "txt", "json", "md"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogFolder {
    Logs,
    #[serde(rename = "crashReports")]
    CrashReports,
}

impl LogFolder {
    fn dir_name(&self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::CrashReports => "crash-reports",
        }
    }

    fn from_str(s: &str) -> AppResult<Self> {
        match s {
            "logs" => Ok(Self::Logs),
            "crash-reports" | "crashReports" => Ok(Self::CrashReports),
            _ => Err(AppError::msg(
                "Folder must be logs or crash-reports",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFileEntry {
    pub name: String,
    pub folder: LogFolder,
    pub size: u64,
    pub modified_at: Option<String>,
    /// Whether the file can be previewed as text in-app.
    pub previewable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogFilePreview {
    pub name: String,
    pub folder: LogFolder,
    pub text: String,
    pub truncated: bool,
    pub size: u64,
}

fn ensure_dir(instance: &Instance, folder: &LogFolder) -> AppResult<PathBuf> {
    let dir = PathBuf::from(&instance.game_dir).join(folder.dir_name());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn is_previewable(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| TEXT_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn safe_filename(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::msg("Empty file name"));
    }
    if trimmed.len() > 255 {
        return Err(AppError::msg("File name too long"));
    }
    if trimmed.contains('\0') {
        return Err(AppError::msg("Invalid file name"));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(AppError::msg("Absolute paths are not allowed"));
    }
    let mut components = path.components();
    let Some(Component::Normal(os)) = components.next() else {
        return Err(AppError::msg("Unsafe file name"));
    };
    if components.next().is_some() {
        return Err(AppError::msg("Nested paths are not allowed"));
    }
    let file = os
        .to_str()
        .ok_or_else(|| AppError::msg("Invalid UTF-8 file name"))?;
    if file == "." || file == ".." || file.contains('/') || file.contains('\\') {
        return Err(AppError::msg("Unsafe file name"));
    }
    Ok(file.to_string())
}

fn resolve_inside(instance: &Instance, folder: &LogFolder, name: &str) -> AppResult<PathBuf> {
    let safe = safe_filename(name)?;
    let root = ensure_dir(instance, folder)?;
    let candidate = root.join(&safe);
    let canon_root = root.canonicalize().unwrap_or(root.clone());
    if !candidate.exists() {
        return Err(AppError::msg("File not found"));
    }
    let canon = candidate
        .canonicalize()
        .map_err(|e| AppError::msg(format!("Could not resolve path: {e}")))?;
    if !canon.starts_with(&canon_root) {
        return Err(AppError::msg(format!(
            "Refusing path outside {}/",
            folder.dir_name()
        )));
    }
    if !canon.is_file() {
        return Err(AppError::msg("Not a file"));
    }
    Ok(canon)
}

fn modified_rfc3339(meta: &std::fs::Metadata) -> Option<String> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|d| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.to_rfc3339())
        })
}

pub fn list_files(instance: &Instance, folder_raw: &str) -> AppResult<Vec<LogFileEntry>> {
    let folder = LogFolder::from_str(folder_raw)?;
    let dir = ensure_dir(instance, &folder)?;
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if safe_filename(name).is_err() {
            continue;
        }
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        out.push(LogFileEntry {
            name: name.to_string(),
            folder: folder.clone(),
            size,
            modified_at: meta.as_ref().and_then(modified_rfc3339),
            previewable: is_previewable(name),
        });
    }
    out.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    Ok(out)
}

/// Read text preview: prefer last ~MAX_TAIL_LINES within MAX_PREVIEW_BYTES.
pub fn read_preview(
    instance: &Instance,
    folder_raw: &str,
    name: &str,
) -> AppResult<LogFilePreview> {
    let folder = LogFolder::from_str(folder_raw)?;
    let path = resolve_inside(instance, &folder, name)?;
    if !is_previewable(name) {
        return Err(AppError::msg(
            "Only .log, .txt, .json, and .md can be previewed here. Reveal the folder for .gz archives.",
        ));
    }
    let meta = std::fs::metadata(&path)?;
    let size = meta.len();
    let (text, truncated) = read_text_capped(&path, size)?;
    Ok(LogFilePreview {
        name: safe_filename(name)?,
        folder,
        text,
        truncated,
        size,
    })
}

fn read_text_capped(path: &Path, size: u64) -> AppResult<(String, bool)> {
    if size == 0 {
        return Ok((String::new(), false));
    }
    let size_trunc = size > MAX_PREVIEW_BYTES;
    let mut file = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    if size_trunc {
        file.seek(SeekFrom::End(-(MAX_PREVIEW_BYTES as i64)))?;
        file.read_to_end(&mut buf)?;
    } else {
        file.read_to_end(&mut buf)?;
    }
    let raw = String::from_utf8_lossy(&buf);
    let body = if size_trunc {
        raw.split_once('\n').map(|(_, rest)| rest).unwrap_or(&raw)
    } else {
        &raw
    };
    let lines: Vec<&str> = body.lines().collect();
    let line_trunc = lines.len() > MAX_TAIL_LINES;
    let start = lines.len().saturating_sub(MAX_TAIL_LINES);
    let truncated = size_trunc || line_trunc;
    let text = if truncated {
        let mut out = String::from("… [truncated — showing end of file]\n");
        out.push_str(&lines[start..].join("\n"));
        out
    } else {
        lines.join("\n")
    };
    Ok((text, truncated))
}

pub fn delete_file(instance: &Instance, folder_raw: &str, name: &str) -> AppResult<()> {
    let folder = LogFolder::from_str(folder_raw)?;
    let path = resolve_inside(instance, &folder, name)?;
    std::fs::remove_file(&path)?;
    Ok(())
}

pub fn open_folder(instance: &Instance, folder_raw: &str) -> AppResult<()> {
    let folder = LogFolder::from_str(folder_raw)?;
    let dir = ensure_dir(instance, &folder)?;
    open::that(&dir).map_err(|e| AppError::msg(format!("Could not open folder: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::Instance;

    fn fake_instance(root: &Path) -> Instance {
        Instance {
            id: "i1".into(),
            name: "Test".into(),
            loader: "fabric".into(),
            game_version: "1.21".into(),
            loader_version: Some("0.1".into()),
            game_dir: root.to_string_lossy().into_owned(),
            java_path: None,
            memory_mb: 2048,
            jvm_args: None,
            keep_open: true,
            last_played: None,
            icon: None,
            created_at: "now".into(),
            updated_at: "now".into(),
        }
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aureum-logs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_traversal() {
        assert!(safe_filename("../x.log").is_err());
        assert!(safe_filename("a/b.log").is_err());
        assert!(safe_filename("ok.log").is_ok());
    }

    #[test]
    fn list_preview_delete_roundtrip() {
        let tmp = tempfile_dir();
        let inst = fake_instance(&tmp);
        let logs = tmp.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("latest.log"), "line1\nline2\nline3\n").unwrap();
        std::fs::write(logs.join("old.log.gz"), b"\x1f\x8b").unwrap();

        let listed = list_files(&inst, "logs").unwrap();
        assert_eq!(listed.len(), 2);
        let previewable = listed.iter().find(|e| e.name == "latest.log").unwrap();
        assert!(previewable.previewable);
        let gz = listed.iter().find(|e| e.name == "old.log.gz").unwrap();
        assert!(!gz.previewable);

        let preview = read_preview(&inst, "logs", "latest.log").unwrap();
        assert!(preview.text.contains("line2"));
        assert!(!preview.truncated);

        assert!(read_preview(&inst, "logs", "../latest.log").is_err());
        assert!(read_preview(&inst, "logs", "old.log.gz").is_err());

        delete_file(&inst, "logs", "latest.log").unwrap();
        assert_eq!(list_files(&inst, "logs").unwrap().len(), 1);
    }
}
