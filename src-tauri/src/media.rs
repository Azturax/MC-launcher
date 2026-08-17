//! Instance media (screenshots) under `{game_dir}/screenshots/`.
//! All paths are validated against traversal before read/delete/import.

use crate::error::{AppError, AppResult};
use crate::instances::Instance;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

const MEDIA_FOLDER: &str = "screenshots";
const MAX_PREVIEW_BYTES: u64 = 20 * 1024 * 1024; // 20 MiB
const MAX_THUMB_EDGE: u32 = 128;
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub name: String,
    pub size: u64,
    pub modified_at: Option<String>,
    pub mime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPreview {
    pub name: String,
    pub mime: String,
    pub data_url: String,
}

fn screenshots_dir(instance: &Instance) -> PathBuf {
    PathBuf::from(&instance.game_dir).join(MEDIA_FOLDER)
}

fn ensure_dir(instance: &Instance) -> AppResult<PathBuf> {
    let dir = screenshots_dir(instance);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn is_image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn mime_for_name(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

/// Reject empty names, separators, `..`, and absolute paths. Basename only.
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
    if !is_image_name(file) {
        return Err(AppError::msg(
            "Only png, jpg, jpeg, webp, and gif are allowed",
        ));
    }
    Ok(file.to_string())
}

fn resolve_inside_screenshots(instance: &Instance, name: &str) -> AppResult<PathBuf> {
    let safe = safe_filename(name)?;
    let root = ensure_dir(instance)?;
    let candidate = root.join(&safe);
    let canon_root = root.canonicalize().unwrap_or(root.clone());
    // File may not exist yet (import) — validate parent + joined name.
    if candidate.exists() {
        let canon = candidate
            .canonicalize()
            .map_err(|e| AppError::msg(format!("Could not resolve path: {e}")))?;
        if !canon.starts_with(&canon_root) {
            return Err(AppError::msg("Refusing path outside screenshots/"));
        }
        return Ok(canon);
    }
    // New file: ensure parent is still screenshots/
    let parent = candidate
        .parent()
        .ok_or_else(|| AppError::msg("Invalid path"))?;
    let canon_parent = parent.canonicalize().unwrap_or(parent.to_path_buf());
    if !canon_parent.starts_with(&canon_root) {
        return Err(AppError::msg("Refusing path outside screenshots/"));
    }
    Ok(candidate)
}

pub fn list_media(instance: &Instance) -> AppResult<Vec<MediaFile>> {
    let dir = ensure_dir(instance)?;
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
        let modified_at = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            })
            .filter(|s| !s.is_empty());
        out.push(MediaFile {
            name: name.to_string(),
            size,
            modified_at,
            mime: mime_for_name(name).to_string(),
        });
    }
    out.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    Ok(out)
}

pub fn read_preview(instance: &Instance, name: &str) -> AppResult<MediaPreview> {
    let path = resolve_inside_screenshots(instance, name)?;
    if !path.is_file() {
        return Err(AppError::msg("Screenshot not found"));
    }
    let meta = std::fs::metadata(&path)?;
    if meta.len() > MAX_PREVIEW_BYTES {
        return Err(AppError::msg(format!(
            "File too large to preview (max {} MiB)",
            MAX_PREVIEW_BYTES / (1024 * 1024)
        )));
    }
    let bytes = std::fs::read(&path)?;
    let mime = mime_for_name(name);
    let data_url = format!("data:{mime};base64,{}", B64.encode(&bytes));
    Ok(MediaPreview {
        name: safe_filename(name)?,
        mime: mime.to_string(),
        data_url,
    })
}

/// Small JPEG thumbnail for the strip — does not load full-size into the UI list.
pub fn read_thumb(instance: &Instance, name: &str) -> AppResult<MediaPreview> {
    let path = resolve_inside_screenshots(instance, name)?;
    if !path.is_file() {
        return Err(AppError::msg("Screenshot not found"));
    }
    let meta = std::fs::metadata(&path)?;
    if meta.len() > MAX_PREVIEW_BYTES {
        return Err(AppError::msg("File too large for thumbnail"));
    }
    let bytes = std::fs::read(&path)?;
    let img = image::load_from_memory(&bytes)
        .map_err(|e| AppError::msg(format!("Could not decode image: {e}")))?;
    let thumb = img.thumbnail(MAX_THUMB_EDGE, MAX_THUMB_EDGE);
    let rgb = thumb.to_rgb8();
    let mut out = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut out);
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 70);
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| AppError::msg(format!("Could not encode thumbnail: {e}")))?;
    }
    Ok(MediaPreview {
        name: safe_filename(name)?,
        mime: "image/jpeg".into(),
        data_url: format!("data:image/jpeg;base64,{}", B64.encode(&out)),
    })
}

pub fn delete_media(instance: &Instance, name: &str) -> AppResult<()> {
    let path = resolve_inside_screenshots(instance, name)?;
    if !path.is_file() {
        return Err(AppError::msg("Screenshot not found"));
    }
    std::fs::remove_file(&path)?;
    Ok(())
}

pub fn open_media_folder(instance: &Instance) -> AppResult<()> {
    let dir = ensure_dir(instance)?;
    open::that(&dir).map_err(|e| AppError::msg(format!("Could not open folder: {e}")))
}

pub fn pick_media_files() -> AppResult<Vec<String>> {
    let files = rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
        .pick_files();
    Ok(files
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Copy external image paths into `screenshots/`. Returns imported basenames.
pub fn import_media(instance: &Instance, source_paths: &[String]) -> AppResult<Vec<MediaFile>> {
    let _ = ensure_dir(instance)?;
    let mut imported = Vec::new();
    for raw in source_paths {
        let src = PathBuf::from(raw);
        if !src.is_file() {
            return Err(AppError::msg(format!(
                "Source not found: {}",
                src.display()
            )));
        }
        let base = src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::msg("Invalid source file name"))?;
        let mut dest_name = safe_filename(base)?;
        let mut dest = resolve_inside_screenshots(instance, &dest_name)?;
        if dest.exists() {
            let stem = Path::new(&dest_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("shot");
            let ext = Path::new(&dest_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            for i in 1..1000 {
                let candidate = format!("{stem}-{i}.{ext}");
                let try_path = resolve_inside_screenshots(instance, &candidate)?;
                if !try_path.exists() {
                    dest_name = candidate;
                    dest = try_path;
                    break;
                }
            }
        }
        std::fs::copy(&src, &dest)?;
        let meta = std::fs::metadata(&dest).ok();
        imported.push(MediaFile {
            name: dest_name.clone(),
            size: meta.map(|m| m.len()).unwrap_or(0),
            modified_at: None,
            mime: mime_for_name(&dest_name).to_string(),
        });
    }
    Ok(imported)
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

    #[test]
    fn rejects_path_traversal_names() {
        assert!(safe_filename("../evil.png").is_err());
        assert!(safe_filename("..\\evil.png").is_err());
        assert!(safe_filename("a/b.png").is_err());
        assert!(safe_filename("a\\b.png").is_err());
        assert!(safe_filename("/tmp/x.png").is_err());
        assert!(safe_filename("ok.png").is_ok());
        assert!(safe_filename("shot.JPG").is_ok());
        assert!(safe_filename("readme.txt").is_err());
    }

    #[test]
    fn list_import_delete_roundtrip() {
        let tmp = tempfile_dir();
        let inst = fake_instance(&tmp);
        let shots = tmp.join("screenshots");
        std::fs::create_dir_all(&shots).unwrap();
        let src = tmp.join("from.png");
        // 1×1 PNG
        let png = image::RgbImage::from_pixel(1, 1, image::Rgb([10, 20, 30]));
        png.save(&src).unwrap();

        let imported = import_media(&inst, &[src.to_string_lossy().into_owned()]).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name, "from.png");

        let listed = list_media(&inst).unwrap();
        assert_eq!(listed.len(), 1);

        let preview = read_preview(&inst, "from.png").unwrap();
        assert!(preview.data_url.starts_with("data:image/png;base64,"));

        let thumb = read_thumb(&inst, "from.png").unwrap();
        assert!(thumb.data_url.starts_with("data:image/jpeg;base64,"));

        delete_media(&inst, "from.png").unwrap();
        assert!(list_media(&inst).unwrap().is_empty());

        assert!(read_preview(&inst, "../from.png").is_err());
        assert!(delete_media(&inst, "..\\from.png").is_err());
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aureum-media-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
