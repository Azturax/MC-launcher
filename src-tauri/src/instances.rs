use crate::error::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub loader: String,
    pub game_version: String,
    pub loader_version: Option<String>,
    pub game_dir: String,
    pub java_path: Option<String>,
    pub memory_mb: i64,
    pub jvm_args: Option<String>,
    pub keep_open: bool,
    pub last_played: Option<String>,
    pub icon: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInstance {
    pub name: String,
    pub loader: String,
    pub game_version: String,
    pub loader_version: Option<String>,
    pub java_path: Option<String>,
    pub memory_mb: Option<i64>,
    pub jvm_args: Option<String>,
    pub keep_open: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePatch {
    pub name: Option<String>,
    pub java_path: Option<String>,
    pub memory_mb: Option<i64>,
    pub jvm_args: Option<String>,
    pub keep_open: Option<bool>,
    pub icon: Option<String>,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceTemplate {
    pub id: String,
    pub name: String,
    pub loader: String,
    pub game_version: String,
    pub loader_version: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStatus {
    pub installed: bool,
    pub client_jar: Option<String>,
}

const LOADERS: &[&str] = &["vanilla", "fabric", "forge", "neoforge", "quilt"];

pub fn templates() -> Vec<InstanceTemplate> {
    vec![
        InstanceTemplate {
            id: "vanilla-latest".into(),
            name: "Vanilla Latest".into(),
            loader: "vanilla".into(),
            game_version: "latest-release".into(),
            loader_version: None,
            description: "Official release, isolated game directory.".into(),
        },
        InstanceTemplate {
            id: "vanilla-snapshot".into(),
            name: "Vanilla Snapshot".into(),
            loader: "vanilla".into(),
            game_version: "latest-snapshot".into(),
            loader_version: None,
            description: "Latest official snapshot, pre-release, or RC.".into(),
        },
        InstanceTemplate {
            id: "vanilla-1.21.1".into(),
            name: "Vanilla 1.21.1".into(),
            loader: "vanilla".into(),
            game_version: "1.21.1".into(),
            loader_version: None,
            description: "Pinned vanilla 1.21.1.".into(),
        },
        InstanceTemplate {
            id: "fabric-1.21.1".into(),
            name: "Fabric 1.21.1".into(),
            loader: "fabric".into(),
            game_version: "1.21.1".into(),
            loader_version: None,
            description: "Fabric loader on 1.21.1. Latest loader resolved at install.".into(),
        },
        InstanceTemplate {
            id: "quilt-1.21.1".into(),
            name: "Quilt 1.21.1".into(),
            loader: "quilt".into(),
            game_version: "1.21.1".into(),
            loader_version: None,
            description: "Quilt loader on 1.21.1.".into(),
        },
        InstanceTemplate {
            id: "neoforge-1.21.1".into(),
            name: "NeoForge 1.21.1".into(),
            loader: "neoforge".into(),
            game_version: "1.21.1".into(),
            loader_version: None,
            description: "NeoForge on 1.21.1 from official installer metadata.".into(),
        },
        InstanceTemplate {
            id: "forge-1.21.1".into(),
            name: "Forge 1.21.1".into(),
            loader: "forge".into(),
            game_version: "1.21.1".into(),
            loader_version: None,
            description: "Forge on 1.21.1 from official installer metadata.".into(),
        },
    ]
}

pub fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let collapsed = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "instance".into()
    } else {
        collapsed
    }
}

fn validate_loader(loader: &str) -> AppResult<()> {
    if LOADERS.contains(&loader) {
        Ok(())
    } else {
        Err(AppError::msg(format!("Unsupported loader: {loader}")))
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn isolated_dir(root: &Path, name: &str, id: &str) -> PathBuf {
    let short = id.get(..8).unwrap_or(id);
    root.join(format!("{}-{}", slugify(name), short))
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Instance>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            i64,
            Option<String>,
            i64,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id, name, loader, game_version, loader_version, game_dir, java_path,
                memory_mb, jvm_args, keep_open, last_played, icon, created_at, updated_at
         FROM instances ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(row_to_instance).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Instance> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            i64,
            Option<String>,
            i64,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id, name, loader, game_version, loader_version, game_dir, java_path,
                memory_mb, jvm_args, keep_open, last_played, icon, created_at, updated_at
         FROM instances WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::msg("Instance not found"))?;
    Ok(row_to_instance(row))
}

pub async fn create(
    pool: &SqlitePool,
    root: &Path,
    new: NewInstance,
) -> AppResult<Instance> {
    validate_loader(&new.loader)?;
    let id = Uuid::new_v4().to_string();
    let game_dir = isolated_dir(root, &new.name, &id);
    std::fs::create_dir_all(&game_dir)?;
    for extra in [
        "mods",
        "resourcepacks",
        "saves",
        "logs",
        "crash-reports",
        "shaderpacks",
        "datapacks",
        "screenshots",
    ] {
        std::fs::create_dir_all(game_dir.join(extra))?;
    }

    let ts = now();
    let memory = new.memory_mb.unwrap_or(2048);
    let keep_open = if new.keep_open.unwrap_or(true) { 1 } else { 0 };
    let game_dir_str = game_dir.to_string_lossy().to_string();

    sqlx::query(
        "INSERT INTO instances
         (id, name, loader, game_version, loader_version, game_dir, java_path, memory_mb,
          jvm_args, keep_open, last_played, icon, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?)",
    )
    .bind(&id)
    .bind(&new.name)
    .bind(&new.loader)
    .bind(&new.game_version)
    .bind(&new.loader_version)
    .bind(&game_dir_str)
    .bind(&new.java_path)
    .bind(memory)
    .bind(&new.jvm_args)
    .bind(keep_open)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn update(pool: &SqlitePool, id: &str, patch: InstancePatch) -> AppResult<Instance> {
    let current = get(pool, id).await?;
    let name = patch.name.unwrap_or(current.name);
    let java_path = patch.java_path.or(current.java_path);
    let memory_mb = patch.memory_mb.unwrap_or(current.memory_mb);
    let jvm_args = patch.jvm_args.or(current.jvm_args);
    let keep_open = patch.keep_open.unwrap_or(current.keep_open);
    let icon = patch.icon.or(current.icon);
    let game_version = patch
        .game_version
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or(current.game_version);
    let loader = patch
        .loader
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or(current.loader);
    if !LOADERS.contains(&loader.as_str()) {
        return Err(AppError::msg(format!("Unsupported loader '{loader}'")));
    }
    let loader_version = if loader == "vanilla" {
        None
    } else if patch.loader_version.is_some() {
        patch
            .loader_version
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    } else {
        current.loader_version
    };
    let ts = now();

    sqlx::query(
        "UPDATE instances SET name = ?, java_path = ?, memory_mb = ?, jvm_args = ?,
         keep_open = ?, icon = ?, game_version = ?, loader = ?, loader_version = ?,
         updated_at = ? WHERE id = ?",
    )
    .bind(&name)
    .bind(&java_path)
    .bind(memory_mb)
    .bind(&jvm_args)
    .bind(if keep_open { 1 } else { 0 })
    .bind(&icon)
    .bind(&game_version)
    .bind(&loader)
    .bind(&loader_version)
    .bind(&ts)
    .bind(id)
    .execute(pool)
    .await?;

    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str, remove_files: bool) -> AppResult<()> {
    let instance = get(pool, id).await?;
    sqlx::query("DELETE FROM instances WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if remove_files {
        let path = PathBuf::from(&instance.game_dir);
        if path.exists() {
            std::fs::remove_dir_all(path)?;
        }
    }
    Ok(())
}

pub async fn clone_instance(
    pool: &SqlitePool,
    id: &str,
    root: &Path,
) -> AppResult<Instance> {
    let src = get(pool, id).await?;
    let new = create(
        pool,
        root,
        NewInstance {
            name: format!("{} (copy)", src.name),
            loader: src.loader,
            game_version: src.game_version,
            loader_version: src.loader_version,
            java_path: src.java_path,
            memory_mb: Some(src.memory_mb),
            jvm_args: src.jvm_args,
            keep_open: Some(src.keep_open),
        },
    )
    .await?;

    copy_dir_contents(Path::new(&src.game_dir), Path::new(&new.game_dir))?;
    Ok(new)
}

pub async fn touch_last_played(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let ts = now();
    sqlx::query("UPDATE instances SET last_played = ?, updated_at = ? WHERE id = ?")
        .bind(&ts)
        .bind(&ts)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub fn status(instance: &Instance) -> InstanceStatus {
    let dir = PathBuf::from(&instance.game_dir);
    let jar = dir
        .join("versions")
        .join(&instance.game_version)
        .join(format!("{}.jar", instance.game_version));
    let loader_ready = match instance.loader.as_str() {
        "vanilla" => true,
        other => {
            let id = format!("{other}-{}", instance.game_version);
            dir.join("versions")
                .join(&id)
                .join(format!("{id}.json"))
                .is_file()
        }
    };
    let installed = jar.is_file() && loader_ready;
    InstanceStatus {
        installed,
        client_jar: installed.then(|| jar.to_string_lossy().to_string()),
    }
}

fn copy_dir_contents(from: &Path, to: &Path) -> AppResult<()> {
    if !from.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(from).into_iter().filter_map(|e| e.ok()) {
        let rel = match entry.path().strip_prefix(from) {
            Ok(r) if !r.as_os_str().is_empty() => r,
            _ => continue,
        };
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

fn row_to_instance(
    row: (
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        i64,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
        String,
        String,
    ),
) -> Instance {
    Instance {
        id: row.0,
        name: row.1,
        loader: row.2,
        game_version: row.3,
        loader_version: row.4,
        game_dir: row.5,
        java_path: row.6,
        memory_mb: row.7,
        jvm_args: row.8,
        keep_open: row.9 != 0,
        last_played: row.10,
        icon: row.11,
        created_at: row.12,
        updated_at: row.13,
    }
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugifies_names() {
        assert_eq!(slugify("Vanilla 1.21"), "vanilla-1-21");
        assert_eq!(slugify("@@@"), "instance");
    }
}
