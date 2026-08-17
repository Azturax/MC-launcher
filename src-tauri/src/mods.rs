//! Install catalog selections onto an instance: resolve, fetch+hash, lockfile.

use crate::catalog::{primary_file, CatalogVersion, ModrinthAdapter, SOURCE_MODRINTH};
use crate::error::{AppError, AppResult};
use crate::install::{download_verified_hashes, verify_sha512};
use crate::instances::Instance;
use crate::resolve::{
    resolve, DepKind, DepRef, Lockfile, LockfileEntry, Package, ResolveResult, Selection,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const SOURCE_LOCAL: &str = "local";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub id: String,
    pub instance_id: String,
    pub project_id: String,
    pub version_id: String,
    pub filename: String,
    pub source: String,
    pub sha512: Option<String>,
    pub sha1: Option<String>,
    pub pinned: bool,
    pub enabled: bool,
    pub channel: Option<String>,
    pub update_version_id: Option<String>,
    pub display_name: Option<String>,
    pub version_number: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
    /// Instance-scoped: `ok` | `update` | `incompatible` | `pinned` | `local`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModRequest {
    pub instance_id: String,
    pub project_id: String,
    pub version_id: Option<String>,
    pub pin: Option<bool>,
    pub channel: Option<String>,
}

fn mods_dir(instance: &Instance) -> PathBuf {
    PathBuf::from(&instance.game_dir).join("mods")
}

fn is_local_project(project_id: &str) -> bool {
    project_id.starts_with("local:")
}

fn jar_filename(name: &str) -> Option<String> {
    if let Some(stem) = name.strip_suffix(".jar.disabled") {
        return Some(format!("{stem}.jar"));
    }
    if name.ends_with(".jar") {
        return Some(name.to_string());
    }
    None
}

fn display_from_filename(filename: &str) -> String {
    filename
        .strip_suffix(".jar")
        .unwrap_or(filename)
        .to_string()
}

fn toggle_jar(mods: &Path, filename: &str, enabled: bool) -> AppResult<()> {
    let active = mods.join(filename);
    let disabled = mods.join(format!("{filename}.disabled"));
    if enabled && disabled.is_file() {
        std::fs::rename(disabled, active)?;
    } else if !enabled && active.is_file() {
        std::fs::rename(active, disabled)?;
    }
    Ok(())
}

pub async fn list_installed(pool: &SqlitePool, instance_id: &str) -> AppResult<Vec<InstalledMod>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            i64,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
        ),
    >(
        "SELECT id, instance_id, project_id, version_id, filename, source, sha512, sha1,
                pinned, enabled, channel, display_name, version_number, sort_order
         FROM instance_mods WHERE instance_id = ?
         ORDER BY sort_order ASC, filename COLLATE NOCASE ASC",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| InstalledMod {
            id: r.0,
            instance_id: r.1,
            project_id: r.2,
            version_id: r.3,
            filename: r.4,
            source: r.5,
            sha512: r.6,
            sha1: r.7,
            pinned: r.8 != 0,
            enabled: r.9 != 0,
            channel: r.10,
            update_version_id: None,
            display_name: r.11,
            version_number: r.12,
            sort_order: r.13,
            compat_status: None,
        })
        .collect())
}

/// Catalog rows plus jars dropped into `mods/` (Prism / MultiMC style).
pub async fn list_installed_with_disk(
    pool: &SqlitePool,
    instance: &Instance,
) -> AppResult<Vec<InstalledMod>> {
    let mut listed = list_installed(pool, &instance.id).await?;
    let dir = mods_dir(instance);
    std::fs::create_dir_all(&dir)?;
    let mut known: HashSet<String> = listed.iter().map(|m| m.filename.clone()).collect();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(filename) = jar_filename(&name) else {
                continue;
            };
            let enabled = !name.ends_with(".disabled");
            if let Some(existing) = listed.iter_mut().find(|m| m.filename == filename) {
                existing.enabled = enabled;
                continue;
            }
            if !known.insert(filename.clone()) {
                continue;
            }
            let order = listed.iter().map(|m| m.sort_order).max().unwrap_or(-1) + 1;
            listed.push(InstalledMod {
                id: format!("local:{filename}"),
                instance_id: instance.id.clone(),
                project_id: format!("local:{filename}"),
                version_id: String::new(),
                filename: filename.clone(),
                source: SOURCE_LOCAL.into(),
                sha512: None,
                sha1: None,
                pinned: false,
                enabled,
                channel: None,
                update_version_id: None,
                display_name: Some(display_from_filename(&filename)),
                version_number: None,
                sort_order: order,
                compat_status: Some("local".into()),
            });
        }
    }

    listed.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| {
                a.filename
                    .to_ascii_lowercase()
                    .cmp(&b.filename.to_ascii_lowercase())
            })
    });
    Ok(listed)
}

/// Persist display / preferred load order. Fabric/Quilt still own class loading;
/// this order is for Aureum UI and export metadata only.
pub async fn reorder_mods(
    pool: &SqlitePool,
    instance: &Instance,
    project_ids: &[String],
) -> AppResult<Vec<InstalledMod>> {
    let current = list_installed_with_disk(pool, instance).await?;
    let by_id: HashMap<&str, &InstalledMod> = current
        .iter()
        .map(|m| (m.project_id.as_str(), m))
        .collect();
    let mut tx = pool.begin().await?;
    for (idx, project_id) in project_ids.iter().enumerate() {
        if let Some(local_name) = project_id.strip_prefix("local:") {
            let Some(mod_row) = by_id.get(project_id.as_str()) else {
                continue;
            };
            let existing = sqlx::query_as::<_, (String,)>(
                "SELECT id FROM instance_mods WHERE instance_id = ? AND project_id = ?",
            )
            .bind(&instance.id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some((id,)) = existing {
                sqlx::query("UPDATE instance_mods SET sort_order = ? WHERE id = ?")
                    .bind(idx as i64)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query(
                    "INSERT INTO instance_mods
                     (id, instance_id, project_id, version_id, filename, source, sha512, sha1, pinned, enabled, channel, sort_order, display_name, version_number)
                     VALUES (?, ?, ?, '', ?, ?, NULL, NULL, 0, ?, NULL, ?, ?, NULL)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(&instance.id)
                .bind(project_id)
                .bind(local_name)
                .bind(SOURCE_LOCAL)
                .bind(if mod_row.enabled { 1 } else { 0 })
                .bind(idx as i64)
                .bind(&mod_row.display_name)
                .execute(&mut *tx)
                .await?;
            }
            continue;
        }
        sqlx::query(
            "UPDATE instance_mods SET sort_order = ? WHERE instance_id = ? AND project_id = ?",
        )
        .bind(idx as i64)
        .bind(&instance.id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    list_installed_with_disk(pool, instance).await
}

pub fn open_mods_folder(instance: &Instance) -> AppResult<()> {
    let dir = mods_dir(instance);
    std::fs::create_dir_all(&dir)?;
    open::that(&dir).map_err(|e| AppError::msg(format!("Could not open mods folder: {e}")))
}

pub async fn install_mod(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instance: &Instance,
    req: InstallModRequest,
) -> AppResult<ResolveResult> {
    if instance.loader == "vanilla" {
        return Err(AppError::msg(
            "Install a Fabric, Quilt, Forge, or NeoForge instance before adding mods",
        ));
    }
    let adapter = ModrinthAdapter;
    let channel = req.channel.as_deref().unwrap_or("stable");
    let loaders = vec![instance.loader.clone()];
    let games = vec![instance.game_version.clone()];

    let root_version = match &req.version_id {
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
            list.into_iter()
                .next()
                .ok_or_else(|| AppError::msg("No matching Modrinth version for this instance"))?
        }
    };

    let mut versions: HashMap<String, CatalogVersion> = HashMap::new();
    versions.insert(root_version.project_id.clone(), root_version.clone());

    let mut queue = vec![root_version.clone()];
    while let Some(current) = queue.pop() {
        for dep in &current.dependencies {
            if DepKind::parse(&dep.dependency_type) != DepKind::Required {
                continue;
            }
            let Some(pid) = &dep.project_id else {
                continue;
            };
            if versions.contains_key(pid) {
                continue;
            }
            let fetched = if let Some(vid) = &dep.version_id {
                adapter.version(http, pool, cache_dir, vid).await?
            } else {
                let list = adapter
                    .versions(http, pool, cache_dir, pid, &loaders, &games, Some(channel))
                    .await?;
                list.into_iter().next().ok_or_else(|| {
                    AppError::msg(format!("Could not resolve required dependency {pid}"))
                })?
            };
            versions.insert(fetched.project_id.clone(), fetched.clone());
            queue.push(fetched);
        }
    }

    let mut packages = HashMap::new();
    let mut latest = HashMap::new();
    for (pid, ver) in &versions {
        latest.insert(pid.clone(), ver.id.clone());
        packages.insert(
            (pid.clone(), ver.id.clone()),
            Package {
                project_id: pid.clone(),
                version_id: ver.id.clone(),
                name: ver.name.clone(),
                deps: ver
                    .dependencies
                    .iter()
                    .filter_map(|d| {
                        Some(DepRef {
                            project_id: d.project_id.clone()?,
                            version_id: d.version_id.clone(),
                            kind: DepKind::parse(&d.dependency_type),
                        })
                    })
                    .collect(),
            },
        );
    }

    let existing = list_installed(pool, &instance.id).await?;
    let mut pins: HashMap<String, String> = existing
        .iter()
        .filter(|m| m.pinned)
        .map(|m| (m.project_id.clone(), m.version_id.clone()))
        .collect();
    let pin_root = req.pin.unwrap_or(false);
    if pin_root {
        pins.insert(root_version.project_id.clone(), root_version.id.clone());
    }

    let outcome = resolve(
        &instance.id,
        &[Selection {
            project_id: root_version.project_id.clone(),
            version_id: Some(root_version.id.clone()),
            pin: pin_root,
        }],
        &packages,
        &latest,
        &pins,
    )?;
    if !outcome.conflicts.is_empty() {
        return Err(AppError::msg(format!(
            "Resolve conflict: {}",
            outcome
                .conflicts
                .iter()
                .map(|c| format!("{} ({})", c.project_id, c.reason))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    let mods_dir = PathBuf::from(&instance.game_dir).join("mods");
    std::fs::create_dir_all(&mods_dir)?;
    let mut lock = Lockfile::read_from(Path::new(&instance.game_dir))?;
    lock.instance_id = instance.id.clone();

    for (project_id, version_id) in &outcome.selected {
        let ver = versions.get(project_id).ok_or_else(|| {
            AppError::msg(format!("Resolved {project_id} is missing from the fetch set"))
        })?;
        if ver.id != *version_id {
            return Err(AppError::msg(format!(
                "Version mismatch for {project_id}: resolved {version_id}, fetched {}",
                ver.id
            )));
        }
        let file = primary_file(ver)?;
        let cached = cache_dir
            .join("mods")
            .join(file.sha512.as_deref().or(file.sha1.as_deref()).unwrap_or(&ver.id))
            .join(&file.filename);
        download_verified_hashes(
            http,
            &file.url,
            &cached,
            file.sha1.as_deref(),
            file.sha512.as_deref(),
        )
        .await?;
        let dest = mods_dir.join(&file.filename);
        if dest != cached {
            std::fs::copy(&cached, &dest)?;
        }
        if let Some(sha) = file.sha512.as_deref() {
            verify_sha512(&dest, sha)?;
        }
        let pinned = pins.get(project_id).is_some();
        upsert_mod_row(
            pool,
            instance,
            ver,
            file.filename.as_str(),
            file.sha1.as_deref(),
            file.sha512.as_deref(),
            pinned,
        )
        .await?;
        lock.upsert_mod(LockfileEntry {
            path: dest.to_string_lossy().into(),
            sha1: file.sha1.clone(),
            sha512: file.sha512.clone(),
            source: SOURCE_MODRINTH.into(),
            project_id: Some(project_id.clone()),
            version_id: Some(version_id.clone()),
            filename: Some(file.filename.clone()),
            pinned,
            enabled: true,
        });
    }

    lock.write_to(Path::new(&instance.game_dir))?;
    Ok(outcome)
}

async fn upsert_mod_row(
    pool: &SqlitePool,
    instance: &Instance,
    ver: &CatalogVersion,
    filename: &str,
    sha1: Option<&str>,
    sha512: Option<&str>,
    pinned: bool,
) -> AppResult<()> {
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM instance_mods WHERE instance_id = ? AND project_id = ?",
    )
    .bind(&instance.id)
    .bind(&ver.project_id)
    .fetch_optional(pool)
    .await?;
    let display = if ver.name.is_empty() {
        display_from_filename(filename)
    } else {
        ver.name.clone()
    };
    let version_number = if ver.version_number.is_empty() {
        None
    } else {
        Some(ver.version_number.clone())
    };
    if let Some((id,)) = existing {
        sqlx::query(
            "UPDATE instance_mods SET version_id = ?, filename = ?, sha1 = ?, sha512 = ?,
             pinned = ?, channel = ?, source = ?, display_name = ?, version_number = ? WHERE id = ?",
        )
        .bind(&ver.id)
        .bind(filename)
        .bind(sha1)
        .bind(sha512)
        .bind(if pinned { 1 } else { 0 })
        .bind(&ver.channel)
        .bind(SOURCE_MODRINTH)
        .bind(&display)
        .bind(&version_number)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        let next_order: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM instance_mods WHERE instance_id = ?",
        )
        .bind(&instance.id)
        .fetch_one(pool)
        .await?
        .0;
        sqlx::query(
            "INSERT INTO instance_mods
             (id, instance_id, project_id, version_id, filename, source, sha512, sha1, pinned, enabled, channel, sort_order, display_name, version_number)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&instance.id)
        .bind(&ver.project_id)
        .bind(&ver.id)
        .bind(filename)
        .bind(SOURCE_MODRINTH)
        .bind(sha512)
        .bind(sha1)
        .bind(if pinned { 1 } else { 0 })
        .bind(&ver.channel)
        .bind(next_order)
        .bind(&display)
        .bind(&version_number)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn set_pin(pool: &SqlitePool, instance: &Instance, project_id: &str, pinned: bool) -> AppResult<()> {
    if is_local_project(project_id) {
        return Ok(());
    }
    sqlx::query(
        "UPDATE instance_mods SET pinned = ? WHERE instance_id = ? AND project_id = ?",
    )
    .bind(if pinned { 1 } else { 0 })
    .bind(&instance.id)
    .bind(project_id)
    .execute(pool)
    .await?;
    let mut lock = Lockfile::read_from(Path::new(&instance.game_dir))?;
    for entry in &mut lock.files {
        if entry.project_id.as_deref() == Some(project_id) {
            entry.pinned = pinned;
        }
    }
    lock.write_to(Path::new(&instance.game_dir))?;
    Ok(())
}

pub async fn set_enabled(
    pool: &SqlitePool,
    instance: &Instance,
    project_id: &str,
    enabled: bool,
) -> AppResult<()> {
    let mods = mods_dir(instance);
    if let Some(filename) = project_id.strip_prefix("local:") {
        toggle_jar(&mods, filename, enabled)?;
        return Ok(());
    }
    sqlx::query(
        "UPDATE instance_mods SET enabled = ? WHERE instance_id = ? AND project_id = ?",
    )
    .bind(if enabled { 1 } else { 0 })
    .bind(&instance.id)
    .bind(project_id)
    .execute(pool)
    .await?;
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT filename FROM instance_mods WHERE instance_id = ? AND project_id = ?",
    )
    .bind(&instance.id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if let Some((filename,)) = row {
        toggle_jar(&mods, &filename, enabled)?;
    }
    let mut lock = Lockfile::read_from(Path::new(&instance.game_dir))?;
    for entry in &mut lock.files {
        if entry.project_id.as_deref() == Some(project_id) {
            entry.enabled = enabled;
        }
    }
    lock.write_to(Path::new(&instance.game_dir))?;
    Ok(())
}

pub async fn remove_mod(pool: &SqlitePool, instance: &Instance, project_id: &str) -> AppResult<()> {
    let mods = mods_dir(instance);
    if let Some(filename) = project_id.strip_prefix("local:") {
        let _ = std::fs::remove_file(mods.join(filename));
        let _ = std::fs::remove_file(mods.join(format!("{filename}.disabled")));
        return Ok(());
    }
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT filename FROM instance_mods WHERE instance_id = ? AND project_id = ?",
    )
    .bind(&instance.id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    sqlx::query("DELETE FROM instance_mods WHERE instance_id = ? AND project_id = ?")
        .bind(&instance.id)
        .bind(project_id)
        .execute(pool)
        .await?;
    if let Some((filename,)) = row {
        let _ = std::fs::remove_file(mods.join(&filename));
        let _ = std::fs::remove_file(mods.join(format!("{filename}.disabled")));
    }
    let mut lock = Lockfile::read_from(Path::new(&instance.game_dir))?;
    lock.remove_project(project_id);
    lock.write_to(Path::new(&instance.game_dir))?;
    Ok(())
}

pub async fn check_updates(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instance: &Instance,
) -> AppResult<Vec<InstalledMod>> {
    let mut installed = list_installed_with_disk(pool, instance).await?;
    let adapter = ModrinthAdapter;
    // Always evaluate against this instance's loader + Minecraft version — not global latest.
    let loaders = if instance.loader == "vanilla" {
        Vec::new()
    } else {
        vec![instance.loader.clone()]
    };
    let games = vec![instance.game_version.clone()];
    for m in &mut installed {
        if m.source == SOURCE_LOCAL || is_local_project(&m.project_id) {
            m.compat_status = Some("local".into());
            continue;
        }
        if m.pinned {
            m.compat_status = Some("pinned".into());
            continue;
        }
        if instance.loader == "vanilla" {
            m.compat_status = Some("incompatible".into());
            continue;
        }
        let list = adapter
            .versions(
                http,
                pool,
                cache_dir,
                &m.project_id,
                &loaders,
                &games,
                Some("stable"),
            )
            .await
            .unwrap_or_default();
        if list.is_empty() {
            m.compat_status = Some("incompatible".into());
            continue;
        }
        let current_matches = list.iter().any(|v| v.id == m.version_id);
        if let Some(latest) = list.first() {
            if latest.id != m.version_id {
                m.update_version_id = Some(latest.id.clone());
                m.compat_status = Some("update".into());
            } else {
                m.compat_status = Some("ok".into());
            }
        }
        if !current_matches && m.update_version_id.is_none() {
            m.compat_status = Some("incompatible".into());
        }
    }
    Ok(installed)
}
