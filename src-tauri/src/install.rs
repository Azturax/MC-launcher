//! Version + loader install from official manifests only.
//! Never vendors Minecraft jars. Every downloaded file is hash-checked
//! when the remote metadata provides a digest.

use crate::error::{AppError, AppResult};
use crate::forge;
use crate::instances::Instance;
use crate::resolve::{Lockfile, LockfileEntry};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2";
const QUILT_META: &str = "https://meta.quiltmc.org/v3";
const FORGE_META: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const NEOFORGE_META: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const RESOURCES: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub sha1: Option<String>,
    pub latest: bool,
    pub latest_snapshot: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub loader: String,
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub instance_id: String,
    pub phase: String,
    pub message: String,
    pub progress: f32,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    latest: Latest,
    versions: Vec<ManifestVersion>,
}

#[derive(Debug, Deserialize)]
struct Latest {
    release: String,
    #[serde(default)]
    snapshot: String,
}

#[derive(Debug, Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    sha1: Option<String>,
}

pub async fn list_game_versions(http: &reqwest::Client) -> AppResult<Vec<GameVersion>> {
    let manifest: Manifest = http.get(VERSION_MANIFEST).send().await?.json().await?;
    let latest_release = manifest.latest.release.clone();
    let latest_snapshot = if manifest.latest.snapshot.is_empty() {
        latest_release.clone()
    } else {
        manifest.latest.snapshot.clone()
    };
    Ok(manifest
        .versions
        .into_iter()
        .map(|v| GameVersion {
            latest: v.id == latest_release,
            latest_snapshot: v.id == latest_snapshot,
            id: v.id,
            version_type: v.version_type,
            url: v.url,
            sha1: v.sha1,
        })
        .collect())
}

pub async fn resolve_game_version(http: &reqwest::Client, requested: &str) -> AppResult<String> {
    if requested == "latest-release" || requested == "latest" {
        let versions = list_game_versions(http).await?;
        return versions
            .into_iter()
            .find(|v| v.latest)
            .map(|v| v.id)
            .ok_or_else(|| AppError::msg("Could not resolve latest release"));
    }
    if requested == "latest-snapshot" {
        let versions = list_game_versions(http).await?;
        return versions
            .iter()
            .find(|v| v.latest_snapshot)
            .or_else(|| versions.iter().find(|v| v.latest))
            .map(|v| v.id.clone())
            .ok_or_else(|| AppError::msg("Could not resolve latest snapshot"));
    }
    Ok(requested.to_string())
}

pub async fn list_loader_versions(
    http: &reqwest::Client,
    loader: &str,
    game_version: &str,
) -> AppResult<Vec<LoaderVersion>> {
    match loader {
        "vanilla" => Ok(vec![LoaderVersion {
            loader: "vanilla".into(),
            version: game_version.to_string(),
            stable: true,
        }]),
        "fabric" => fabric_or_quilt_loaders(http, FABRIC_META, "fabric", game_version).await,
        "quilt" => fabric_or_quilt_loaders(http, QUILT_META, "quilt", game_version).await,
        "forge" => maven_versions(http, FORGE_META, "forge", Some(game_version)).await,
        "neoforge" => maven_versions(http, NEOFORGE_META, "neoforge", Some(game_version)).await,
        other => Err(AppError::msg(format!("Unknown loader: {other}"))),
    }
}

async fn fabric_or_quilt_loaders(
    http: &reqwest::Client,
    base: &str,
    loader: &str,
    game: &str,
) -> AppResult<Vec<LoaderVersion>> {
    let url = format!("{base}/versions/loader/{game}");
    let v: serde_json::Value = http.get(url).send().await?.json().await?;
    let arr = v.as_array().cloned().unwrap_or_default();
    Ok(arr
        .into_iter()
        .filter_map(|item| {
            let version = item["loader"]["version"].as_str()?.to_string();
            let stable = item["loader"]["stable"].as_bool().unwrap_or(false);
            Some(LoaderVersion {
                loader: loader.to_string(),
                version,
                stable,
            })
        })
        .collect())
}

async fn maven_versions(
    http: &reqwest::Client,
    url: &str,
    loader: &str,
    game_filter: Option<&str>,
) -> AppResult<Vec<LoaderVersion>> {
    let xml = http.get(url).send().await?.text().await?;
    let mut versions = Vec::new();
    for raw in xml.split("<version>").skip(1) {
        let Some(end) = raw.find("</version>") else {
            continue;
        };
        let ver = raw[..end].trim();
        if let Some(game) = game_filter {
            let matches = match loader {
                "forge" => ver.starts_with(&format!("{game}-")),
                "neoforge" => neoforge_matches(ver, game),
                _ => true,
            };
            if !matches {
                continue;
            }
        }
        versions.push(LoaderVersion {
            loader: loader.to_string(),
            version: ver.to_string(),
            stable: !ver.contains("beta") && !ver.contains("alpha"),
        });
    }
    versions.reverse();
    versions.truncate(40);
    Ok(versions)
}

fn neoforge_matches(ver: &str, game: &str) -> bool {
    // NeoForge 21.1.x corresponds to Minecraft 1.21.1
    let parts: Vec<&str> = game.trim_start_matches('1').trim_start_matches('.').split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0];
        let minor = parts[1];
        ver.starts_with(&format!("{major}.{minor}."))
    } else {
        ver.contains(game)
    }
}

pub async fn install_instance(
    app: &AppHandle,
    http: &reqwest::Client,
    cache_dir: &Path,
    instance: &Instance,
) -> AppResult<String> {
    let game_version = resolve_game_version(http, &instance.game_version).await?;
    emit(
        Some(app),
        instance,
        "manifest",
        &format!("Resolving {game_version}"),
        0.02,
    );

    let versions = list_game_versions(http).await?;
    let meta = versions
        .iter()
        .find(|v| v.id == game_version)
        .ok_or_else(|| AppError::msg(format!("Unknown game version {game_version}")))?;

    let bytes = http.get(&meta.url).send().await?.bytes().await?;
    if let Some(expected) = &meta.sha1 {
        let digest = hex::encode(Sha1::digest(&bytes));
        if !digest.eq_ignore_ascii_case(expected) {
            return Err(AppError::msg(format!(
                "Version manifest SHA1 mismatch for {game_version}"
            )));
        }
    }
    let version_json: serde_json::Value = serde_json::from_slice(&bytes)?;

    let game_dir = PathBuf::from(&instance.game_dir);
    std::fs::create_dir_all(&game_dir)?;
    let version_dir = game_dir.join("versions").join(&game_version);
    std::fs::create_dir_all(&version_dir)?;
    std::fs::write(
        version_dir.join(format!("{game_version}.json")),
        serde_json::to_vec_pretty(&version_json)?,
    )?;

    let mut lock = Lockfile::empty(&instance.id);

    emit(Some(app), instance, "client", "Downloading client jar", 0.08);
    if let Some(client) = version_json.pointer("/downloads/client") {
        let url = client["url"]
            .as_str()
            .ok_or_else(|| AppError::msg("Client URL missing"))?;
        let sha = client["sha1"].as_str();
        let dest = version_dir.join(format!("{game_version}.jar"));
        download_verified(http, url, &dest, sha).await?;
        lock.files.push(LockfileEntry {
            path: dest.to_string_lossy().into(),
            sha1: sha.map(|s| s.to_string()),
            sha512: None,
            source: "mojang".into(),
            ..Default::default()
        });
    }

    emit(Some(app), instance, "libraries", "Downloading libraries", 0.2);
    let lib_root = cache_dir.join("libraries");
    download_libraries(http, &version_json, &lib_root, &game_dir, &mut lock, Some(app), instance).await?;

    emit(Some(app), instance, "assets", "Downloading assets", 0.55);
    download_assets(http, &version_json, cache_dir, &mut lock, Some(app), instance).await?;

    match instance.loader.as_str() {
        "vanilla" => {}
        "fabric" => {
            install_fabric_like(
                http,
                FABRIC_META,
                "fabric",
                instance,
                &game_version,
                cache_dir,
                &mut lock,
                app,
            )
            .await?;
        }
        "quilt" => {
            install_fabric_like(
                http,
                QUILT_META,
                "quilt",
                instance,
                &game_version,
                cache_dir,
                &mut lock,
                app,
            )
            .await?;
        }
        "forge" | "neoforge" => {
            forge::install_forge_like(
                http,
                instance,
                &game_version,
                cache_dir,
                &mut lock,
                Some(app),
            )
            .await?;
        }
        other => return Err(AppError::msg(format!("Unsupported loader {other}"))),
    }

    let hash = lock.write_to(&game_dir)?;
    emit(Some(app), instance, "done", "Install complete", 1.0);
    Ok(hash)
}

async fn install_fabric_like(
    http: &reqwest::Client,
    base: &str,
    loader_name: &str,
    instance: &Instance,
    game_version: &str,
    cache_dir: &Path,
    lock: &mut Lockfile,
    app: &AppHandle,
) -> AppResult<()> {
    let loader_ver = match &instance.loader_version {
        Some(v) if !v.is_empty() => v.clone(),
        _ => {
            let list = list_loader_versions(http, loader_name, game_version).await?;
            list.iter()
                .find(|v| v.stable)
                .or_else(|| list.first())
                .map(|v| v.version.clone())
                .ok_or_else(|| {
                    AppError::msg("No loader versions published for this game version")
                })?
        }
    };

    emit(
        Some(app),
        instance,
        "loader",
        &format!("Fetching {loader_name} {loader_ver} profile"),
        0.88,
    );
    let url = format!("{base}/versions/loader/{game_version}/{loader_ver}/profile/json");
    let profile: serde_json::Value = http.get(&url).send().await?.json().await?;
    let dest = PathBuf::from(&instance.game_dir)
        .join("versions")
        .join(format!("{loader_name}-{game_version}"))
        .join(format!("{loader_name}-{game_version}.json"));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&dest, serde_json::to_vec_pretty(&profile)?)?;

    let lib_root = cache_dir.join("libraries");
    download_libraries(
        http,
        &profile,
        &lib_root,
        Path::new(&instance.game_dir),
        lock,
        Some(app),
        instance,
    )
    .await?;
    Ok(())
}

pub(crate) async fn download_libraries(
    http: &reqwest::Client,
    version_json: &serde_json::Value,
    lib_root: &Path,
    game_dir: &Path,
    lock: &mut Lockfile,
    app: Option<&AppHandle>,
    instance: &Instance,
) -> AppResult<()> {
    let Some(libs) = version_json.get("libraries").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    let natives_dir = game_dir.join("natives");
    std::fs::create_dir_all(&natives_dir)?;
    let total = libs.len().max(1);
    for (i, lib) in libs.iter().enumerate() {
        if !library_allowed(lib) {
            continue;
        }
        emit(
            app,
            instance,
            "libraries",
            &format!("Library {}/{}", i + 1, total),
            0.2 + 0.3 * (i as f32 / total as f32),
        );
        if let Some(artifact) = lib.pointer("/downloads/artifact") {
            if let Some(path) = artifact["path"].as_str() {
                let dest = lib_root.join(path);
                match artifact["url"].as_str() {
                    // Absolute CDN / Maven URL.
                    Some(url) if url.starts_with("https://") || url.starts_with("http://") => {
                        download_verified(http, url, &dest, artifact["sha1"].as_str()).await?;
                        lock.files.push(LockfileEntry {
                            path: dest.to_string_lossy().into(),
                            sha1: artifact["sha1"].as_str().map(|s| s.to_string()),
                            sha512: None,
                            source: "mojang".into(),
                            ..Default::default()
                        });
                    }
                    // Empty URL: Forge/NeoForge client jars are produced by processors.
                    Some("") | None => {}
                    // Relative / odd URLs — rebuild from known hosts when possible.
                    Some(_) => {
                        if let Some(url) = resolve_library_url(lib, artifact["url"].as_str(), path) {
                            download_verified(http, &url, &dest, artifact["sha1"].as_str()).await?;
                            lock.files.push(LockfileEntry {
                                path: dest.to_string_lossy().into(),
                                sha1: artifact["sha1"].as_str().map(|s| s.to_string()),
                                sha512: None,
                                source: "loader".into(),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        } else if let Some(name) = lib["name"].as_str() {
            if let Some(url) = maven_url(lib, name) {
                let dest = lib_root.join(maven_path(name));
                download_verified(http, &url, &dest, None).await?;
                lock.files.push(LockfileEntry {
                    path: dest.to_string_lossy().into(),
                    sha1: None,
                    sha512: None,
                    source: "loader".into(),
                    ..Default::default()
                });
            }
        }

        if let Some(classifier) = native_classifier(lib) {
            if let Some(artifact) = lib.pointer(&format!("/downloads/classifiers/{classifier}")) {
                if let Some(path) = artifact["path"].as_str() {
                    let dest = lib_root.join(path);
                    match artifact["url"].as_str() {
                        Some(url) if url.starts_with("https://") || url.starts_with("http://") => {
                            download_verified(http, url, &dest, artifact["sha1"].as_str()).await?;
                            lock.files.push(LockfileEntry {
                                path: dest.to_string_lossy().into(),
                                sha1: artifact["sha1"].as_str().map(|s| s.to_string()),
                                source: "natives".into(),
                                ..Default::default()
                            });
                        }
                        Some("") | None => {}
                        Some(_) => {
                            if let Some(url) =
                                resolve_library_url(lib, artifact["url"].as_str(), path)
                            {
                                download_verified(http, &url, &dest, artifact["sha1"].as_str())
                                    .await?;
                                lock.files.push(LockfileEntry {
                                    path: dest.to_string_lossy().into(),
                                    sha1: artifact["sha1"].as_str().map(|s| s.to_string()),
                                    source: "natives".into(),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
            }
        }

        extract_natives(lib, lib_root, &natives_dir)?;
    }
    Ok(())
}

pub(crate) fn maven_rel_path(name: &str) -> PathBuf {
    maven_path(name)
}

pub(crate) fn maven_path(name: &str) -> PathBuf {
    let (coord, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return PathBuf::from(name);
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts.get(3);
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    PathBuf::from(group).join(artifact).join(version).join(file)
}

pub(crate) fn maven_url(lib: &serde_json::Value, name: &str) -> Option<String> {
    let raw_base = lib["url"].as_str().unwrap_or("https://libraries.minecraft.net/");
    let base = if raw_base.starts_with("http://") || raw_base.starts_with("https://") {
        raw_base
    } else {
        "https://libraries.minecraft.net/"
    };
    let path = maven_path(name);
    Some(format!(
        "{}{}",
        if base.ends_with('/') {
            base.to_string()
        } else {
            format!("{base}/")
        },
        path.to_string_lossy().replace('\\', "/")
    ))
}

/// Prefer absolute artifact URLs; otherwise rebuild from Maven coords / known hosts.
fn resolve_library_url(
    lib: &serde_json::Value,
    artifact_url: Option<&str>,
    path: &str,
) -> Option<String> {
    if let Some(url) = artifact_url {
        if url.starts_with("https://") || url.starts_with("http://") {
            return Some(url.to_string());
        }
    }
    // Explicit maven base on the library entry (common in Forge / NeoForge profiles).
    if lib["url"]
        .as_str()
        .is_some_and(|u| u.starts_with("http://") || u.starts_with("https://"))
    {
        if let Some(name) = lib["name"].as_str() {
            return maven_url(lib, name);
        }
    }
    let norm = path.replace('\\', "/");
    let host = if norm.contains("net/minecraftforge/") || norm.contains("de/oceanlabs/") {
        "https://maven.minecraftforge.net/"
    } else if norm.contains("net/neoforged/") {
        "https://maven.neoforged.net/releases/"
    } else {
        "https://libraries.minecraft.net/"
    };
    if let Some(name) = lib["name"].as_str() {
        let mut patched = lib.clone();
        patched["url"] = serde_json::Value::String(host.into());
        return maven_url(&patched, name);
    }
    Some(format!("{host}{norm}"))
}

fn native_arch() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    }
}

fn native_classifier(lib: &serde_json::Value) -> Option<String> {
    lib.pointer("/natives")
        .and_then(|n| n.get(current_os()))
        .and_then(|v| v.as_str())
        .map(|c| c.replace("${arch}", native_arch()))
}

fn extract_natives(lib: &serde_json::Value, lib_root: &Path, natives_dir: &Path) -> AppResult<()> {
    let Some(classifier) = native_classifier(lib) else {
        return Ok(());
    };
    let Some(artifact) = lib.pointer(&format!("/downloads/classifiers/{classifier}")) else {
        return Ok(());
    };
    let Some(path) = artifact["path"].as_str() else {
        return Ok(());
    };
    let jar = lib_root.join(path);
    if !jar.is_file() {
        return Ok(());
    }
    let file = std::fs::File::open(&jar)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.ends_with('/') || name.contains("META-INF") {
            continue;
        }
        let dest = natives_dir.join(name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(dest)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

async fn download_assets(
    http: &reqwest::Client,
    version_json: &serde_json::Value,
    cache_dir: &Path,
    lock: &mut Lockfile,
    app: Option<&AppHandle>,
    instance: &Instance,
) -> AppResult<()> {
    let Some(index_url) = version_json.pointer("/assetIndex/url").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let index_id = version_json
        .pointer("/assetIndex/id")
        .and_then(|v| v.as_str())
        .unwrap_or("legacy");
    let index_sha = version_json
        .pointer("/assetIndex/sha1")
        .and_then(|v| v.as_str());
    let indexes = cache_dir.join("assets").join("indexes");
    std::fs::create_dir_all(&indexes)?;
    let index_path = indexes.join(format!("{index_id}.json"));
    download_verified(http, index_url, &index_path, index_sha).await?;

    let index: serde_json::Value = serde_json::from_slice(&std::fs::read(&index_path)?)?;
    let Some(objects) = index.get("objects").and_then(|v| v.as_object()) else {
        return Ok(());
    };
    let objects_dir = cache_dir.join("assets").join("objects");
    let total = objects.len().max(1);
    for (i, (_name, obj)) in objects.iter().enumerate() {
        let hash = obj["hash"].as_str().unwrap_or_default();
        if hash.len() < 2 {
            continue;
        }
        let dest = objects_dir.join(&hash[..2]).join(hash);
        if dest.is_file() && verify_sha1(&dest, hash).is_ok() {
            continue;
        }
        let url = format!("{RESOURCES}/{}/{}", &hash[..2], hash);
        download_verified(http, &url, &dest, Some(hash)).await?;
        if i % 25 == 0 {
            emit(
                app,
                instance,
                "assets",
                &format!("Assets {i}/{total}"),
                0.55 + 0.3 * (i as f32 / total as f32),
            );
        }
    }
    lock.files.push(LockfileEntry {
        path: index_path.to_string_lossy().into(),
        sha1: index_sha.map(|s| s.to_string()),
        sha512: None,
        source: "mojang-assets".into(),
        ..Default::default()
    });
    Ok(())
}

pub async fn download_verified(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
) -> AppResult<()> {
    download_verified_hashes(http, url, dest, sha1, None).await
}

pub async fn download_verified_hashes(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
    sha512: Option<&str>,
) -> AppResult<()> {
    if dest.is_file() {
        let sha1_ok = sha1.map(|e| verify_sha1(dest, e).is_ok()).unwrap_or(true);
        let sha512_ok = sha512
            .map(|e| verify_sha512(dest, e).is_ok())
            .unwrap_or(true);
        if sha1_ok && sha512_ok && (sha1.is_some() || sha512.is_some()) {
            return Ok(());
        }
        if sha1.is_none() && sha512.is_none() {
            return Ok(());
        }
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let resp = http.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::msg(format!(
            "Download failed ({}) {}",
            resp.status(),
            url
        )));
    }
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp)?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?)?;
    }
    file.flush()?;
    drop(file);
    if let Some(expected) = sha1 {
        verify_sha1(&tmp, expected)?;
    }
    if let Some(expected) = sha512 {
        verify_sha512(&tmp, expected)?;
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn verify_sha512(path: &Path, expected: &str) -> AppResult<()> {
    let bytes = std::fs::read(path)?;
    let digest = hex::encode(<Sha512 as sha2::Digest>::digest(&bytes));
    if digest.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(AppError::msg(format!(
            "SHA512 mismatch for {}: expected {expected}, got {digest}",
            path.display()
        )))
    }
}

pub fn verify_sha1(path: &Path, expected: &str) -> AppResult<()> {
    let bytes = std::fs::read(path)?;
    let digest = hex::encode(Sha1::digest(&bytes));
    if digest.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(AppError::msg(format!(
            "SHA1 mismatch for {}: expected {expected}, got {digest}",
            path.display()
        )))
    }
}

fn library_allowed(lib: &serde_json::Value) -> bool {
    let Some(rules) = lib.get("rules").and_then(|v| v.as_array()) else {
        return true;
    };
    let mut allow = false;
    for rule in rules {
        let action = rule["action"].as_str().unwrap_or("allow");
        let os_ok = match rule.get("os") {
            None => true,
            Some(os) => os["name"].as_str() == Some(current_os()),
        };
        if os_ok {
            allow = action == "allow";
        }
    }
    allow
}

fn current_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub(crate) fn emit(app: Option<&AppHandle>, instance: &Instance, phase: &str, message: &str, progress: f32) {
    if let Some(app) = app {
        let _ = app.emit(
            "install-progress",
            InstallProgress {
                instance_id: instance.id.clone(),
                phase: phase.into(),
                message: message.into(),
                progress,
            },
        );
    } else {
        log::info!(
            "install [{}] {} ({:.0}%) id={}",
            phase,
            message,
            progress * 100.0,
            instance.id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{neoforge_matches, resolve_library_url, verify_sha1};
    use sha1::{Digest, Sha1};
    use std::io::Write;

    #[test]
    fn neoforge_filter() {
        assert!(neoforge_matches("21.1.66", "1.21.1"));
        assert!(!neoforge_matches("21.0.1", "1.21.1"));
    }

    #[test]
    fn sha1_roundtrip() {
        let dir = std::env::temp_dir();
        let path = dir.join("aureum-sha1-test.bin");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"hello").unwrap();
        drop(f);
        let expected = hex::encode(Sha1::digest(b"hello"));
        verify_sha1(&path, &expected).unwrap();
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolves_forge_maven_host() {
        let lib = serde_json::json!({
            "name": "net.minecraftforge:forge:1.21.1-52.1.0:client"
        });
        let url = resolve_library_url(
            &lib,
            Some(""),
            "net/minecraftforge/forge/1.21.1-52.1.0/forge-1.21.1-52.1.0-client.jar",
        )
        .unwrap();
        assert!(url.starts_with("https://maven.minecraftforge.net/"));
        assert!(url.ends_with("forge-1.21.1-52.1.0-client.jar"));
    }
}
