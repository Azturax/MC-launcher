//! Modrinth `.mrpack` import / export (formatVersion 1).
//! Spec: https://docs.modrinth.com/modpacks/format/
//! Never scrapes CurseForge. Downloads are hash-verified.

use crate::error::{AppError, AppResult};
use crate::install::{download_verified_hashes, verify_sha1, verify_sha512};
use crate::instances::{self, Instance, NewInstance};
use crate::mods;
use crate::resolve::{Lockfile, LockfileEntry};
use crate::catalog::SOURCE_MODRINTH;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<MrpackFile>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<MrpackEnv>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackEnv {
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub server: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportMrpackRequest {
    /// Absolute path to a `.mrpack` / `.zip` file.
    pub path: String,
    /// When set, install pack files into this instance (loader/game must match).
    pub instance_id: Option<String>,
    /// Optional name when creating a new instance.
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportMrpackRequest {
    pub instance_id: String,
    /// Destination `.mrpack` path.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MrpackImportResult {
    pub instance: Instance,
    pub files_installed: usize,
    pub created: bool,
}

pub fn read_index_from_path(pack_path: &Path) -> AppResult<MrpackIndex> {
    let file = File::open(pack_path)?;
    let mut zip = ZipArchive::new(file)?;
    let mut entry = zip
        .by_name("modrinth.index.json")
        .map_err(|_| AppError::msg("Pack is missing modrinth.index.json"))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    let index: MrpackIndex = serde_json::from_slice(&buf)?;
    if index.format_version != 1 {
        return Err(AppError::msg(format!(
            "Unsupported mrpack formatVersion {} (expected 1)",
            index.format_version
        )));
    }
    if index.game != "minecraft" {
        return Err(AppError::msg(format!(
            "Unsupported mrpack game '{}'",
            index.game
        )));
    }
    Ok(index)
}

fn loader_from_deps(deps: &HashMap<String, String>) -> AppResult<(String, Option<String>, String)> {
    let game = deps
        .get("minecraft")
        .cloned()
        .ok_or_else(|| AppError::msg("mrpack is missing dependencies.minecraft"))?;
    if let Some(v) = deps.get("fabric-loader") {
        return Ok(("fabric".into(), Some(v.clone()), game));
    }
    if let Some(v) = deps.get("quilt-loader") {
        return Ok(("quilt".into(), Some(v.clone()), game));
    }
    if let Some(v) = deps.get("neoforge") {
        return Ok(("neoforge".into(), Some(v.clone()), game));
    }
    if let Some(v) = deps.get("forge") {
        return Ok(("forge".into(), Some(v.clone()), game));
    }
    Ok(("vanilla".into(), None, game))
}

/// Reject path traversal and absolute paths; return a normalized relative path.
pub fn safe_rel_path(raw: &str) -> AppResult<PathBuf> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(AppError::msg(format!("Unsafe absolute pack path: {raw}")));
    }
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::msg(format!("Unsafe pack path: {raw}")));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(AppError::msg(format!("Empty pack path: {raw}")));
    }
    Ok(out)
}

fn env_skips_client(file: &MrpackFile) -> bool {
    file.env
        .as_ref()
        .map(|e| e.client.eq_ignore_ascii_case("unsupported"))
        .unwrap_or(false)
}

fn extract_override_tree(
    zip: &mut ZipArchive<File>,
    prefix: &str,
    game_dir: &Path,
) -> AppResult<usize> {
    let mut count = 0usize;
    let mut names = Vec::new();
    for i in 0..zip.len() {
        if let Ok(entry) = zip.by_index(i) {
            names.push(entry.name().to_string());
        }
    }
    for name in names {
        let Some(rel) = name.strip_prefix(prefix) else {
            continue;
        };
        if rel.is_empty() || name.ends_with('/') {
            continue;
        }
        let dest_rel = safe_rel_path(rel)?;
        let dest = game_dir.join(&dest_rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut entry = zip
            .by_name(&name)
            .map_err(|_| AppError::msg(format!("Missing zip entry {name}")))?;
        let mut out = File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
        count += 1;
    }
    Ok(count)
}

async fn upsert_downloaded_mod(
    pool: &SqlitePool,
    instance: &Instance,
    rel: &Path,
    sha1: Option<&str>,
    sha512: Option<&str>,
    sort_order: i64,
) -> AppResult<()> {
    let filename = rel
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mod.jar")
        .to_string();
    // Prefer Modrinth project id from CDN URL path if later enriched; for pack
    // files use a stable path-based id so re-import updates the same row.
    let project_id = format!("mrpack:{}", rel.to_string_lossy().replace('\\', "/"));
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM instance_mods WHERE instance_id = ? AND project_id = ?",
    )
    .bind(&instance.id)
    .bind(&project_id)
    .fetch_optional(pool)
    .await?;
    let display = filename
        .strip_suffix(".jar")
        .unwrap_or(&filename)
        .to_string();
    if let Some((id,)) = existing {
        sqlx::query(
            "UPDATE instance_mods SET filename = ?, sha1 = ?, sha512 = ?, source = ?,
             sort_order = ?, display_name = ?, enabled = 1 WHERE id = ?",
        )
        .bind(&filename)
        .bind(sha1)
        .bind(sha512)
        .bind(SOURCE_MODRINTH)
        .bind(sort_order)
        .bind(&display)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO instance_mods
             (id, instance_id, project_id, version_id, filename, source, sha512, sha1, pinned, enabled, channel, sort_order, display_name, version_number)
             VALUES (?, ?, ?, '', ?, ?, ?, ?, 0, 1, NULL, ?, ?, NULL)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&instance.id)
        .bind(&project_id)
        .bind(&filename)
        .bind(SOURCE_MODRINTH)
        .bind(sha512)
        .bind(sha1)
        .bind(sort_order)
        .bind(&display)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn import_mrpack(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instances_root: &Path,
    req: ImportMrpackRequest,
) -> AppResult<MrpackImportResult> {
    let pack_path = PathBuf::from(&req.path);
    if !pack_path.is_file() {
        return Err(AppError::msg(format!(
            "mrpack not found: {}",
            pack_path.display()
        )));
    }
    let index = read_index_from_path(&pack_path)?;
    let (loader, loader_version, game_version) = loader_from_deps(&index.dependencies)?;

    let (instance, created) = if let Some(id) = &req.instance_id {
        let existing = instances::get(pool, id).await?;
        if existing.game_version != game_version {
            return Err(AppError::msg(format!(
                "Pack targets Minecraft {game_version}, instance is {}",
                existing.game_version
            )));
        }
        if existing.loader != loader && existing.loader != "vanilla" {
            return Err(AppError::msg(format!(
                "Pack targets {loader}, instance is {}",
                existing.loader
            )));
        }
        // Upgrade vanilla → pack loader when importing into an empty-ish shell.
        let instance = if existing.loader == "vanilla" && loader != "vanilla" {
            sqlx::query(
                "UPDATE instances SET loader = ?, loader_version = ?, updated_at = datetime('now') WHERE id = ?",
            )
            .bind(&loader)
            .bind(&loader_version)
            .bind(&existing.id)
            .execute(pool)
            .await?;
            instances::get(pool, &existing.id).await?
        } else {
            existing
        };
        (instance, false)
    } else {
        let name = req
            .name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| index.name.clone());
        let instance = instances::create(
            pool,
            instances_root,
            NewInstance {
                name,
                loader: loader.clone(),
                game_version: game_version.clone(),
                loader_version: loader_version.clone(),
                java_path: None,
                memory_mb: None,
                jvm_args: None,
                keep_open: Some(true),
            },
        )
        .await?;
        (instance, true)
    };

    let game_dir = PathBuf::from(&instance.game_dir);
    std::fs::create_dir_all(game_dir.join("mods"))?;

    let mut lock = Lockfile::read_from(&game_dir)?;
    lock.instance_id = instance.id.clone();

    let mut installed = 0usize;
    let mut order = 0i64;
    for file in &index.files {
        if env_skips_client(file) {
            continue;
        }
        let rel = safe_rel_path(&file.path)?;
        let dest = game_dir.join(&rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let sha1 = file.hashes.get("sha1").map(|s| s.as_str());
        let sha512 = file.hashes.get("sha512").map(|s| s.as_str());
        if sha1.is_none() && sha512.is_none() {
            return Err(AppError::msg(format!(
                "Pack file {} is missing sha1/sha512",
                file.path
            )));
        }

        let url = file
            .downloads
            .iter()
            .find(|u| u.starts_with("https://") || u.starts_with("http://"))
            .ok_or_else(|| AppError::msg(format!("No download URL for {}", file.path)))?;

        let cache_key = sha512.or(sha1).unwrap_or(&file.path);
        let cached = cache_dir
            .join("mrpack")
            .join(cache_key.chars().take(64).collect::<String>())
            .join(rel.file_name().unwrap_or_default());
        download_verified_hashes(http, url, &cached, sha1, sha512).await?;
        if dest != cached {
            std::fs::copy(&cached, &dest)?;
        }
        if let Some(s) = sha512 {
            verify_sha512(&dest, s)?;
        } else if let Some(s) = sha1 {
            verify_sha1(&dest, s)?;
        }

        let is_mod = rel.starts_with("mods")
            || rel
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jar"))
                .unwrap_or(false);
        if is_mod {
            upsert_downloaded_mod(pool, &instance, &rel, sha1, sha512, order).await?;
            order += 1;
        }

        lock.upsert_mod(LockfileEntry {
            path: dest.to_string_lossy().into(),
            sha1: sha1.map(|s| s.to_string()),
            sha512: sha512.map(|s| s.to_string()),
            source: SOURCE_MODRINTH.into(),
            project_id: Some(format!(
                "mrpack:{}",
                rel.to_string_lossy().replace('\\', "/")
            )),
            version_id: None,
            filename: rel
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string()),
            pinned: false,
            enabled: true,
        });
        installed += 1;
    }

    // Overrides (client-side).
    {
        let file = File::open(&pack_path)?;
        let mut zip = ZipArchive::new(file)?;
        extract_override_tree(&mut zip, "overrides/", &game_dir)?;
        extract_override_tree(&mut zip, "client-overrides/", &game_dir)?;
    }

    lock.write_to(&game_dir)?;
    Ok(MrpackImportResult {
        instance,
        files_installed: installed,
        created,
    })
}

fn file_sha_hex(path: &Path) -> AppResult<(Option<String>, Option<String>, u64)> {
    let bytes = std::fs::read(path)?;
    let size = bytes.len() as u64;
    use sha1::{Digest as _, Sha1};
    use sha2::Sha512;
    let sha1 = hex::encode(Sha1::digest(&bytes));
    let sha512 = hex::encode(Sha512::digest(&bytes));
    Ok((Some(sha1), Some(sha512), size))
}

/// Export installed mods as a Modrinth pack. Files without a downloadable CDN
/// URL are embedded under `overrides/` so the pack remains installable.
pub async fn export_mrpack(
    pool: &SqlitePool,
    instance: &Instance,
    req: ExportMrpackRequest,
) -> AppResult<String> {
    let mods = mods::list_installed_with_disk(pool, instance).await?;
    let game_dir = PathBuf::from(&instance.game_dir);
    let out_path = PathBuf::from(&req.path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut deps = HashMap::new();
    deps.insert("minecraft".into(), instance.game_version.clone());
    match instance.loader.as_str() {
        "fabric" => {
            if let Some(v) = &instance.loader_version {
                deps.insert("fabric-loader".into(), v.clone());
            }
        }
        "quilt" => {
            if let Some(v) = &instance.loader_version {
                deps.insert("quilt-loader".into(), v.clone());
            }
        }
        "forge" => {
            if let Some(v) = &instance.loader_version {
                deps.insert("forge".into(), v.clone());
            }
        }
        "neoforge" => {
            if let Some(v) = &instance.loader_version {
                deps.insert("neoforge".into(), v.clone());
            }
        }
        _ => {}
    }

    let mut index = MrpackIndex {
        format_version: 1,
        game: "minecraft".into(),
        version_id: req
            .version_id
            .unwrap_or_else(|| format!("aureum-{}", &instance.id[..8.min(instance.id.len())])),
        name: req.name.unwrap_or_else(|| instance.name.clone()),
        summary: req.summary.or_else(|| {
            Some(format!(
                "Exported from Aureum ({})",
                instance.game_version
            ))
        }),
        files: Vec::new(),
        dependencies: deps,
    };

    let out_file = File::create(&out_path)?;
    let mut zip = ZipWriter::new(out_file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for m in &mods {
        if !m.enabled {
            continue;
        }
        let jar = game_dir.join("mods").join(&m.filename);
        let jar = if jar.is_file() {
            jar
        } else {
            let disabled = game_dir.join("mods").join(format!("{}.disabled", m.filename));
            if disabled.is_file() {
                continue;
            }
            continue;
        };

        let rel = format!("mods/{}", m.filename);
        let (sha1, sha512, size) = file_sha_hex(&jar)?;
        let mut hashes = HashMap::new();
        if let Some(s) = &sha1 {
            hashes.insert("sha1".into(), s.clone());
        }
        if let Some(s) = &sha512 {
            hashes.insert("sha512".into(), s.clone());
        }

        // Prefer CDN URL reconstruction when we have a real Modrinth version id.
        let downloads = if m.source == SOURCE_MODRINTH
            && !m.version_id.is_empty()
            && !m.project_id.starts_with("mrpack:")
            && !m.project_id.starts_with("local:")
        {
            vec![format!(
                "https://cdn.modrinth.com/data/{}/versions/{}/{}",
                m.project_id, m.version_id, m.filename
            )]
        } else {
            Vec::new()
        };

        if downloads.is_empty() {
            // Embed under overrides so importers still get the jar.
            let entry_name = format!("overrides/{rel}");
            zip.start_file(entry_name, opts)?;
            zip.write_all(&std::fs::read(&jar)?)?;
        } else {
            index.files.push(MrpackFile {
                path: rel,
                hashes,
                downloads,
                file_size: size,
                env: Some(MrpackEnv {
                    client: "required".into(),
                    server: "unsupported".into(),
                }),
            });
        }
    }

    let index_bytes = serde_json::to_vec_pretty(&index)?;
    zip.start_file("modrinth.index.json", opts)?;
    zip.write_all(&index_bytes)?;
    zip.finish()?;
    Ok(out_path.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallModpackRequest {
    pub project_id: String,
    pub version_id: Option<String>,
    pub name: Option<String>,
    pub channel: Option<String>,
}

/// Download a Modrinth modpack version (`.mrpack`) and import it as a new instance.
pub async fn install_from_catalog(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    instances_root: &Path,
    req: InstallModpackRequest,
) -> AppResult<MrpackImportResult> {
    use crate::catalog::{primary_file, ModrinthAdapter};

    let adapter = ModrinthAdapter;
    let detail = adapter
        .project(http, pool, cache_dir, &req.project_id)
        .await?;
    if detail.project_type != "modpack" {
        return Err(AppError::msg(format!(
            "Project '{}' is a {}, not a modpack",
            detail.title, detail.project_type
        )));
    }

    let channel = req.channel.as_deref().unwrap_or("stable");
    let version = match &req.version_id {
        Some(id) => adapter.version(http, pool, cache_dir, id).await?,
        None => {
            let list = adapter
                .versions(
                    http,
                    pool,
                    cache_dir,
                    &req.project_id,
                    &[],
                    &[],
                    Some(channel),
                )
                .await?;
            list.into_iter()
                .next()
                .ok_or_else(|| AppError::msg("No Modrinth versions for this modpack"))?
        }
    };

    let file = primary_file(&version)?;
    if !file.filename.to_ascii_lowercase().ends_with(".mrpack")
        && !file.filename.to_ascii_lowercase().ends_with(".zip")
    {
        // Still try — some packs use .zip; import validates modrinth.index.json.
        log::warn!(
            "Modpack primary file is {}; expecting .mrpack",
            file.filename
        );
    }

    let pack_cache = cache_dir
        .join("mrpack-catalog")
        .join(&version.id)
        .join(&file.filename);
    download_verified_hashes(
        http,
        &file.url,
        &pack_cache,
        file.sha1.as_deref(),
        file.sha512.as_deref(),
    )
    .await?;

    let name = req
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| detail.title.clone());

    import_mrpack(
        http,
        pool,
        cache_dir,
        instances_root,
        ImportMrpackRequest {
            path: pack_cache.to_string_lossy().into_owned(),
            instance_id: None,
            name: Some(name),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_rel_path("../evil").is_err());
        assert!(safe_rel_path("mods/../../evil").is_err());
        assert!(safe_rel_path("mods/sodium.jar").is_ok());
    }

    #[test]
    fn parses_minimal_index() {
        let dir = std::env::temp_dir().join(format!("aureum-mrpack-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("test.mrpack");
        {
            let file = File::create(&pack).unwrap();
            let mut zip = ZipWriter::new(file);
            let opts = SimpleFileOptions::default();
            let index = r#"{
              "formatVersion": 1,
              "game": "minecraft",
              "versionId": "1.0.0",
              "name": "Test Pack",
              "files": [],
              "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.0" }
            }"#;
            zip.start_file("modrinth.index.json", opts).unwrap();
            zip.write_all(index.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let parsed = read_index_from_path(&pack).unwrap();
        assert_eq!(parsed.name, "Test Pack");
        let (loader, ver, game) = loader_from_deps(&parsed.dependencies).unwrap();
        assert_eq!(loader, "fabric");
        assert_eq!(ver.as_deref(), Some("0.16.0"));
        assert_eq!(game, "1.21.1");
        let _ = std::fs::remove_dir_all(dir);
    }
}
