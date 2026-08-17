//! Install Modrinth resource packs, shader packs, and datapacks into an instance.
//! Paths follow Minecraft / Iris / Fabric conventions. Never scrapes CurseForge.

use crate::catalog::{primary_file, ModrinthAdapter, SOURCE_MODRINTH};
use crate::error::{AppError, AppResult};
use crate::install::download_verified_hashes;
use crate::instances::Instance;
use crate::resolve::{Lockfile, LockfileEntry};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallContentRequest {
    pub instance_id: String,
    pub project_id: String,
    pub version_id: Option<String>,
    /// `resourcepack` | `shader` | `datapack`
    pub project_type: String,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentInstallResult {
    pub path: String,
    pub filename: String,
    pub project_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledContent {
    pub id: String,
    pub instance_id: String,
    pub project_type: String,
    pub filename: String,
    pub path: String,
    pub source: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    /// `file` or `dir`
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_version_id: Option<String>,
    /// Instance-scoped: `ok` | `update` | `incompatible` | `local` (datapacks only for version checks)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat_status: Option<String>,
}

pub fn content_dir(project_type: &str) -> AppResult<&'static str> {
    match project_type {
        "resourcepack" | "resourcepacks" => Ok("resourcepacks"),
        "shader" | "shaderpack" | "shaderpacks" => Ok("shaderpacks"),
        "datapack" | "datapacks" => Ok("datapacks"),
        other => Err(AppError::msg(format!(
            "Unsupported content type '{other}' (expected resourcepack, shader, or datapack)"
        ))),
    }
}

fn type_for_folder(folder: &str) -> &'static str {
    match folder {
        "resourcepacks" => "resourcepack",
        "shaderpacks" => "shader",
        "datapacks" => "datapack",
        _ => "resourcepack",
    }
}

const CONTENT_FOLDERS: &[&str] = &["resourcepacks", "shaderpacks", "datapacks"];

fn is_content_entry(name: &str, is_dir: bool) -> bool {
    if name.starts_with('.') {
        return false;
    }
    if is_dir {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip")
        || lower.ends_with(".jar")
        || lower.ends_with(".mcpack")
}

fn remove_path_recursive(path: &Path) -> AppResult<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else if path.is_file() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn list_content(instance: &Instance) -> AppResult<Vec<InstalledContent>> {
    let game_dir = PathBuf::from(&instance.game_dir);
    let lock = Lockfile::read_from(&game_dir).unwrap_or_else(|_| Lockfile::empty(&instance.id));
    let mut out = Vec::new();

    for folder in CONTENT_FOLDERS {
        let root = game_dir.join(folder);
        std::fs::create_dir_all(&root)?;
        let project_type = type_for_folder(folder).to_string();
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let is_file = meta.as_ref().map(|m| m.is_file()).unwrap_or(false);
            if !is_dir && !is_file {
                continue;
            }
            if !is_content_entry(&name, is_dir) {
                continue;
            }
            let full = entry.path();
            let full_str = full.to_string_lossy().replace('\\', "/");
            let lock_hit = lock.files.iter().find(|f| {
                let p = f.path.replace('\\', "/");
                p == full_str
                    || f.filename.as_deref() == Some(name.as_str())
                    || p.ends_with(&format!("/{folder}/{name}"))
            });
            out.push(InstalledContent {
                id: format!("{folder}:{name}"),
                instance_id: instance.id.clone(),
                project_type: project_type.clone(),
                filename: name,
                path: full.to_string_lossy().into_owned(),
                source: lock_hit
                    .map(|f| f.source.clone())
                    .unwrap_or_else(|| "local".into()),
                project_id: lock_hit.and_then(|f| f.project_id.clone()),
                version_id: lock_hit.and_then(|f| f.version_id.clone()),
                sha1: lock_hit.and_then(|f| f.sha1.clone()),
                sha512: lock_hit.and_then(|f| f.sha512.clone()),
                kind: if is_dir { "dir".into() } else { "file".into() },
                update_version_id: None,
                compat_status: None,
            });
        }
    }

    out.sort_by(|a, b| {
        a.project_type
            .cmp(&b.project_type)
            .then_with(|| a.filename.to_ascii_lowercase().cmp(&b.filename.to_ascii_lowercase()))
    });
    Ok(out)
}

pub fn remove_content(instance: &Instance, content_id: &str) -> AppResult<()> {
    let Some((folder, name)) = content_id.split_once(':') else {
        return Err(AppError::msg("Invalid content id"));
    };
    if !CONTENT_FOLDERS.contains(&folder) {
        return Err(AppError::msg("Unknown content folder"));
    }
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(AppError::msg("Unsafe content name"));
    }
    let game_dir = PathBuf::from(&instance.game_dir);
    let path = game_dir.join(folder).join(name);
    if !path.exists() {
        return Err(AppError::msg(format!("Content not found: {name}")));
    }
    let canonical_root = game_dir.join(folder).canonicalize().unwrap_or(game_dir.join(folder));
    let canonical = path.canonicalize().unwrap_or(path.clone());
    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::msg("Refusing to remove path outside content folder"));
    }
    remove_path_recursive(&path)?;

    let mut lock = Lockfile::read_from(&game_dir)?;
    let path_str = path.to_string_lossy().replace('\\', "/");
    lock.files.retain(|f| {
        let p = f.path.replace('\\', "/");
        p != path_str && f.filename.as_deref() != Some(name)
    });
    lock.write_to(&game_dir)?;
    Ok(())
}

pub fn open_content_folder(instance: &Instance, project_type: Option<&str>) -> AppResult<()> {
    let game_dir = PathBuf::from(&instance.game_dir);
    let dir = match project_type {
        Some(t) => game_dir.join(content_dir(t)?),
        None => game_dir.clone(),
    };
    std::fs::create_dir_all(&dir)?;
    open::that(&dir).map_err(|e| AppError::msg(format!("Could not open folder: {e}")))
}

pub async fn install_content(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instance: &Instance,
    req: InstallContentRequest,
) -> AppResult<ContentInstallResult> {
    let folder = content_dir(&req.project_type)?;
    let adapter = ModrinthAdapter;
    let detail = adapter
        .project(http, pool, cache_dir, &req.project_id)
        .await?;
    let expected = type_for_folder(folder);
    if detail.project_type != expected && detail.project_type != req.project_type {
        if matches!(detail.project_type.as_str(), "mod" | "modpack" | "plugin") {
            return Err(AppError::msg(format!(
                "'{}' is a {}, not a {expected}",
                detail.title, detail.project_type
            )));
        }
    }

    let channel = req.channel.as_deref().unwrap_or("stable");
    // Resource packs and shaders: do not filter by MC version or loader.
    // Datapacks still prefer the instance game version (+ loader when modded).
    let ignore_version = matches!(folder, "resourcepacks" | "shaderpacks");
    let loaders = if ignore_version || instance.loader == "vanilla" {
        Vec::new()
    } else {
        vec![instance.loader.clone()]
    };
    let games = if ignore_version {
        Vec::new()
    } else {
        vec![instance.game_version.clone()]
    };

    let version = match &req.version_id {
        Some(id) => adapter.version(http, pool, cache_dir, id).await?,
        None => {
            let list = adapter
                .versions(
                    http,
                    pool,
                    cache_dir,
                    &req.project_id,
                    &loaders,
                    &games,
                    Some(channel),
                )
                .await?;
            list.into_iter().next().ok_or_else(|| {
                if ignore_version {
                    AppError::msg(format!("No {expected} versions published on Modrinth"))
                } else {
                    AppError::msg(format!(
                        "No matching {expected} version for {} {}",
                        instance.loader, instance.game_version
                    ))
                }
            })?
        }
    };

    let file = primary_file(&version)?;
    let dest_root = PathBuf::from(&instance.game_dir).join(folder);
    std::fs::create_dir_all(&dest_root)?;
    let dest = dest_root.join(&file.filename);

    let cached = cache_dir
        .join("content")
        .join(folder)
        .join(file.sha512.as_deref().or(file.sha1.as_deref()).unwrap_or(&version.id))
        .join(&file.filename);
    download_verified_hashes(
        http,
        &file.url,
        &cached,
        file.sha1.as_deref(),
        file.sha512.as_deref(),
    )
    .await?;
    if dest != cached {
        std::fs::copy(&cached, &dest)?;
    }

    let mut lock = Lockfile::read_from(Path::new(&instance.game_dir))?;
    lock.instance_id = instance.id.clone();
    lock.upsert_mod(LockfileEntry {
        path: dest.to_string_lossy().into(),
        sha1: file.sha1.clone(),
        sha512: file.sha512.clone(),
        source: SOURCE_MODRINTH.into(),
        project_id: Some(req.project_id.clone()),
        version_id: Some(version.id.clone()),
        filename: Some(file.filename.clone()),
        pinned: false,
        enabled: true,
    });
    lock.write_to(Path::new(&instance.game_dir))?;

    Ok(ContentInstallResult {
        path: dest.to_string_lossy().into_owned(),
        filename: file.filename.clone(),
        project_type: expected.into(),
    })
}

/// Datapacks: evaluate updates against the instance Minecraft version.
/// Resource packs / shaders: skip version filters (still report catalog latest when linked).
pub async fn check_updates(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instance: &Instance,
) -> AppResult<Vec<InstalledContent>> {
    let mut listed = list_content(instance)?;
    let adapter = ModrinthAdapter;
    for item in &mut listed {
        if item.source == "local" || item.project_id.is_none() {
            item.compat_status = Some("local".into());
            continue;
        }
        let Some(project_id) = item.project_id.clone() else {
            continue;
        };
        let ignore_version = matches!(item.project_type.as_str(), "resourcepack" | "shader");
        let loaders = if ignore_version || instance.loader == "vanilla" {
            Vec::new()
        } else {
            vec![instance.loader.clone()]
        };
        let games = if ignore_version {
            Vec::new()
        } else {
            vec![instance.game_version.clone()]
        };
        let list = adapter
            .versions(
                http,
                pool,
                cache_dir,
                &project_id,
                &loaders,
                &games,
                Some("stable"),
            )
            .await
            .unwrap_or_default();
        if list.is_empty() {
            item.compat_status = if ignore_version {
                Some("ok".into())
            } else {
                Some("incompatible".into())
            };
            continue;
        }
        if let Some(latest) = list.first() {
            let current = item.version_id.as_deref().unwrap_or("");
            if latest.id != current {
                item.update_version_id = Some(latest.id.clone());
                item.compat_status = Some("update".into());
            } else {
                item.compat_status = Some("ok".into());
            }
        }
    }
    Ok(listed)
}

/// Apply a catalog content update: remove the old file, then install the new version.
pub async fn apply_update(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instance: &Instance,
    content_id: &str,
) -> AppResult<ContentInstallResult> {
    let listed = list_content(instance)?;
    let item = listed
        .into_iter()
        .find(|c| c.id == content_id)
        .ok_or_else(|| AppError::msg("Content not found"))?;
    let project_id = item
        .project_id
        .clone()
        .ok_or_else(|| AppError::msg("Local content has no Modrinth project id"))?;
    let version_id = item.update_version_id.clone().ok_or_else(|| {
        AppError::msg("No update available — run Check updates first")
    })?;
    // Drop the old file/lock entry before installing so renamed packages do not orphan.
    let _ = remove_content(instance, content_id);
    install_content(
        http,
        pool,
        cache_dir,
        instance,
        InstallContentRequest {
            instance_id: instance.id.clone(),
            project_id,
            version_id: Some(version_id),
            project_type: item.project_type,
            channel: Some("stable".into()),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn maps_folders() {
        assert_eq!(content_dir("resourcepack").unwrap(), "resourcepacks");
        assert_eq!(content_dir("shader").unwrap(), "shaderpacks");
        assert_eq!(content_dir("datapack").unwrap(), "datapacks");
        assert!(content_dir("mod").is_err());
    }

    #[test]
    fn lists_and_removes_content() {
        let root = std::env::temp_dir().join(format!("aureum-content-{}", uuid::Uuid::new_v4()));
        let game = root.join("game");
        fs::create_dir_all(game.join("resourcepacks")).unwrap();
        fs::write(game.join("resourcepacks").join("pack.zip"), b"fake").unwrap();
        fs::create_dir_all(game.join("shaderpacks").join("fancy")).unwrap();

        let instance = Instance {
            id: "i1".into(),
            name: "t".into(),
            loader: "fabric".into(),
            game_version: "1.21.1".into(),
            loader_version: None,
            game_dir: game.to_string_lossy().into_owned(),
            java_path: None,
            memory_mb: 2048,
            jvm_args: None,
            keep_open: true,
            last_played: None,
            icon: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let listed = list_content(&instance).unwrap();
        assert_eq!(listed.len(), 2);
        remove_content(&instance, "resourcepacks:pack.zip").unwrap();
        let listed = list_content(&instance).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].filename, "fancy");
        let _ = fs::remove_dir_all(root);
    }
}
