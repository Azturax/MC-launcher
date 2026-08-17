//! Headless Forge / NeoForge client install from official installer jars.
//! Downloads Maven metadata, extracts `version.json` + `install_profile.json`,
//! fetches libraries, and runs client-side processors (same contract as the
//! official installer). Never vendors Minecraft jars.
//!
//! ## Failure modes (still possible after hardening)
//! - Missing processor jar on disk after library download (network / Maven mirror).
//! - Processor Main-Class missing or processor exits non-zero (token/`{SIDE}`
//!   substitution issues, mismatched Minecraft client jar).
//! - Official installer CLI fallback also fails (needs a working Java, write
//!   access under the instance game dir, and a completed vanilla client jar).
//! - NeoForge version filter mismatches odd game versions (`neoforge_matches`).
//! - Status reports "installed" from version JSON alias only — it does not
//!   verify every processor artifact.
//!
//! Smoke tip: create the Forge 1.21.1 template, Install once, confirm
//! `versions/forge-1.21.1/` (or NeoForge alias) exists, then Play. Do not leave
//! `tauri dev` running indefinitely for smoke checks.

use crate::error::{AppError, AppResult};
use crate::install::{self, download_verified, maven_rel_path};
use crate::instances::Instance;
use crate::java;
use crate::resolve::{Lockfile, LockfileEntry};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;
use zip::ZipArchive;

const FORGE_INSTALLER: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/{ver}/forge-{ver}-installer.jar";
const NEOFORGE_INSTALLER: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{ver}/neoforge-{ver}-installer.jar";

pub async fn install_forge_like(
    http: &reqwest::Client,
    instance: &Instance,
    game_version: &str,
    cache_dir: &Path,
    lock: &mut Lockfile,
    app: Option<&AppHandle>,
) -> AppResult<()> {
    let loader = instance.loader.as_str();
    let loader_ver = resolve_loader_version(http, loader, instance, game_version).await?;
    install::emit(
        app,
        instance,
        "loader",
        &format!("Downloading {loader} {loader_ver} installer"),
        0.86,
    );

    let url = installer_url(loader, &loader_ver)?;
    let installer = cache_dir
        .join("installers")
        .join(format!("{loader}-{loader_ver}-installer.jar"));
    download_verified(http, &url, &installer, None).await?;
    lock.files.push(LockfileEntry {
        path: installer.to_string_lossy().into(),
        source: loader.into(),
        ..Default::default()
    });

    let (mut version_json, profile) = extract_installer_jsons(&installer)?;
    if version_json.get("inheritsFrom").is_none() {
        version_json["inheritsFrom"] = Value::String(game_version.to_string());
    }

    let game_dir = PathBuf::from(&instance.game_dir);
    let alias = format!("{loader}-{game_version}");
    let version_id = version_json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or(&alias)
        .to_string();

    write_version_json(&game_dir, &version_id, &version_json)?;
    if version_id != alias {
        write_version_json(&game_dir, &alias, &version_json)?;
    }

    let lib_root = cache_dir.join("libraries");
    install::emit(app, instance, "loader", "Downloading loader libraries", 0.88);
    install::download_libraries(http, &version_json, &lib_root, &game_dir, lock, app, instance)
        .await?;
    install::download_libraries(http, &profile, &lib_root, &game_dir, lock, app, instance).await?;

    let java = java::resolve_bin(instance.java_path.as_deref(), None)?;
    let work = cache_dir.join("forge-work").join(&instance.id);
    std::fs::create_dir_all(&work)?;

    install::emit(app, instance, "loader", "Running installer processors", 0.92);
    match run_processors(&java, &installer, &profile, &game_dir, &lib_root, &work, game_version)
    {
        Ok(()) => {}
        Err(proc_err) => {
            install::emit(
                app,
                instance,
                "loader",
                "Processors failed; trying official installer CLI",
                0.93,
            );
            // CLI fallback: headless --installClient. Common when a processor
            // needs GUI-only paths or a data token we could not resolve.
            run_official_installer(&java, &installer, &game_dir, &lib_root).map_err(|cli_err| {
                AppError::msg(format!(
                    "{loader} install failed.\nProcessors: {proc_err}\nInstaller: {cli_err}\n\
                     Ensure Java runs headless, the vanilla {game_version} jar is present, \
                     and the instance folder is writable."
                ))
            })?;
            if let Ok(installed) = find_loader_version_json(&game_dir, game_version) {
                if let Ok(bytes) = std::fs::read(&installed) {
                    if let Ok(json) = serde_json::from_slice::<Value>(&bytes) {
                        write_version_json(&game_dir, &alias, &json)?;
                    }
                }
            }
        }
    }

    install::emit(
        app,
        instance,
        "loader",
        &format!("{loader} {loader_ver} ready"),
        0.97,
    );
    Ok(())
}

async fn resolve_loader_version(
    http: &reqwest::Client,
    loader: &str,
    instance: &Instance,
    game_version: &str,
) -> AppResult<String> {
    match &instance.loader_version {
        Some(v) if !v.is_empty() => Ok(v.clone()),
        _ => {
            let list = install::list_loader_versions(http, loader, game_version).await?;
            list.iter()
                .find(|v| v.stable)
                .or_else(|| list.first())
                .map(|v| v.version.clone())
                .ok_or_else(|| {
                    AppError::msg(format!("No {loader} versions published for {game_version}"))
                })
        }
    }
}

fn installer_url(loader: &str, version: &str) -> AppResult<String> {
    match loader {
        "forge" => Ok(FORGE_INSTALLER.replace("{ver}", version)),
        "neoforge" => Ok(NEOFORGE_INSTALLER.replace("{ver}", version)),
        other => Err(AppError::msg(format!("Not a Forge-family loader: {other}"))),
    }
}

fn extract_installer_jsons(installer: &Path) -> AppResult<(Value, Value)> {
    let file = std::fs::File::open(installer)?;
    let mut zip = ZipArchive::new(file)?;
    let profile = read_zip_json(&mut zip, "install_profile.json")?;
    let version = if let Some(rel) = profile.get("json").and_then(|v| v.as_str()) {
        let rel = rel.trim_start_matches('/');
        read_zip_json(&mut zip, rel).or_else(|_| read_zip_json(&mut zip, "version.json"))?
    } else {
        read_zip_json(&mut zip, "version.json")?
    };
    Ok((version, profile))
}

fn read_zip_json(zip: &mut ZipArchive<std::fs::File>, name: &str) -> AppResult<Value> {
    let mut entry = zip
        .by_name(name)
        .map_err(|_| AppError::msg(format!("Installer is missing {name}")))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(serde_json::from_slice(&buf)?)
}

fn write_version_json(game_dir: &Path, id: &str, json: &Value) -> AppResult<()> {
    let dir = game_dir.join("versions").join(id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{id}.json")), serde_json::to_vec_pretty(json)?)?;
    Ok(())
}

fn find_loader_version_json(game_dir: &Path, vanilla: &str) -> AppResult<PathBuf> {
    let versions = game_dir.join("versions");
    if let Ok(entries) = std::fs::read_dir(&versions) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == vanilla {
                continue;
            }
            let json = entry.path().join(format!("{name}.json"));
            if json.is_file() {
                return Ok(json);
            }
        }
    }
    Err(AppError::msg("Installer did not write a loader version profile"))
}

fn run_processors(
    java: &str,
    installer: &Path,
    profile: &Value,
    game_dir: &Path,
    lib_root: &Path,
    work: &Path,
    game_version: &str,
) -> AppResult<()> {
    let Some(processors) = profile.get("processors").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    let data = build_data_map(profile, installer, game_dir, lib_root, work, game_version)?;
    let sep = if cfg!(windows) { ";" } else { ":" };

    for (i, proc) in processors.iter().enumerate() {
        if let Some(sides) = proc.get("sides").and_then(|v| v.as_array()) {
            let client = sides.iter().any(|s| s.as_str() == Some("client"));
            if !client {
                continue;
            }
        }
        if outputs_exist(proc, &data) {
            continue;
        }

        let jar_name = proc
            .get("jar")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::msg("Processor is missing jar"))?;
        let jar = lib_root.join(maven_rel_path(jar_name));
        if !jar.is_file() {
            return Err(AppError::msg(format!(
                "Processor jar missing: {}",
                jar.display()
            )));
        }
        let main = jar_main_class(&jar)?;

        let mut cp = vec![jar.to_string_lossy().into_owned()];
        if let Some(extra) = proc.get("classpath").and_then(|v| v.as_array()) {
            for item in extra {
                if let Some(name) = item.as_str() {
                    cp.push(
                        lib_root
                            .join(maven_rel_path(name))
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }

        let args: Vec<String> = proc
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| subst(s, &data, lib_root))
                    .collect()
            })
            .unwrap_or_default();

        let output = Command::new(java)
            .arg("-cp")
            .arg(cp.join(sep))
            .arg(&main)
            .args(&args)
            .current_dir(game_dir)
            .output()
            .map_err(|e| AppError::msg(format!("Could not start processor: {e}")))?;
        if !output.status.success() {
            return Err(AppError::msg(format!(
                "Processor {} ({main}) failed ({}/{}):\n{}\n{}",
                i + 1,
                i + 1,
                processors.len(),
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            )));
        }
    }
    Ok(())
}

fn outputs_exist(proc: &Value, data: &HashMap<String, String>) -> bool {
    let Some(outputs) = proc.get("outputs").and_then(|v| v.as_object()) else {
        return false;
    };
    if outputs.is_empty() {
        return false;
    }
    outputs.keys().all(|key| {
        let path = subst(key, data, Path::new("."));
        Path::new(&path).is_file()
    })
}

fn build_data_map(
    profile: &Value,
    installer: &Path,
    game_dir: &Path,
    lib_root: &Path,
    work: &Path,
    game_version: &str,
) -> AppResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    let client_jar = game_dir
        .join("versions")
        .join(game_version)
        .join(format!("{game_version}.jar"));
    map.insert("SIDE".into(), "client".into());
    map.insert(
        "MINECRAFT_JAR".into(),
        client_jar.to_string_lossy().into_owned(),
    );
    map.insert("ROOT".into(), game_dir.to_string_lossy().into_owned());
    map.insert(
        "INSTALLER".into(),
        installer.to_string_lossy().into_owned(),
    );
    map.insert(
        "LIBRARY_DIR".into(),
        lib_root.to_string_lossy().into_owned(),
    );

    if let Some(data) = profile.get("data").and_then(|v| v.as_object()) {
        for (key, val) in data {
            let raw = val
                .get("client")
                .and_then(|v| v.as_str())
                .or_else(|| val.as_str())
                .unwrap_or("");
            map.insert(
                key.clone(),
                resolve_data_value(raw, installer, lib_root, work)?,
            );
        }
    }
    Ok(map)
}

fn resolve_data_value(
    raw: &str,
    installer: &Path,
    lib_root: &Path,
    work: &Path,
) -> AppResult<String> {
    let raw = raw.trim();
    if raw.starts_with('[') && raw.ends_with(']') {
        let coord = &raw[1..raw.len() - 1];
        return Ok(lib_root
            .join(maven_rel_path(coord))
            .to_string_lossy()
            .into_owned());
    }
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return Ok(raw[1..raw.len() - 1].to_string());
    }
    if raw.starts_with('/') {
        let rel = raw.trim_start_matches('/');
        let dest = work.join(rel);
        extract_from_installer(installer, rel, &dest)?;
        return Ok(dest.to_string_lossy().into_owned());
    }
    Ok(raw.to_string())
}

fn extract_from_installer(installer: &Path, rel: &str, dest: &Path) -> AppResult<()> {
    if dest.is_file() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::open(installer)?;
    let mut zip = ZipArchive::new(file)?;
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.name_for_index(i).map(|s| s.to_string()))
        .collect();
    let name = names
        .iter()
        .find(|n| n.as_str() == rel || n.as_str() == format!("/{rel}") || n.ends_with(rel))
        .cloned()
        .ok_or_else(|| AppError::msg(format!("Installer is missing {rel}")))?;
    let mut entry = zip
        .by_name(&name)
        .map_err(|_| AppError::msg(format!("Installer is missing {rel}")))?;
    let mut out = std::fs::File::create(dest)?;
    std::io::copy(&mut entry, &mut out)?;
    out.flush()?;
    Ok(())
}

fn subst(input: &str, data: &HashMap<String, String>, lib_root: &Path) -> String {
    let mut out = input.to_string();
    for (k, v) in data {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    while let Some(start) = out.find('[') {
        let rest = &out[start + 1..];
        let Some(end) = rest.find(']') else {
            break;
        };
        let coord = rest[..end].to_string();
        let path = lib_root
            .join(maven_rel_path(&coord))
            .to_string_lossy()
            .into_owned();
        out.replace_range(start..start + end + 2, &path);
    }
    out
}

fn jar_main_class(jar: &Path) -> AppResult<String> {
    let file = std::fs::File::open(jar)?;
    let mut zip = ZipArchive::new(file)?;
    let mut mf = zip
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|_| AppError::msg(format!("No manifest in {}", jar.display())))?;
    let mut text = String::new();
    mf.read_to_string(&mut text)?;
    let mut continued = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(' ') {
            continued.push_str(rest);
            continue;
        }
        if !continued.is_empty() {
            if let Some(cls) = continued.strip_prefix("Main-Class:") {
                return Ok(cls.trim().to_string());
            }
            continued.clear();
        }
        continued.push_str(line);
    }
    if let Some(cls) = continued.strip_prefix("Main-Class:") {
        return Ok(cls.trim().to_string());
    }
    Err(AppError::msg(format!(
        "No Main-Class in {}",
        jar.display()
    )))
}

fn run_official_installer(
    java: &str,
    installer: &Path,
    game_dir: &Path,
    lib_root: &Path,
) -> AppResult<()> {
    let dest_libs = game_dir.join("libraries");
    if !dest_libs.exists() {
        let _ = copy_dir_best_effort(lib_root, &dest_libs);
    }
    let output = Command::new(java)
        .arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(installer)
        .arg("--installClient")
        .arg(game_dir)
        .current_dir(game_dir)
        .output()
        .map_err(|e| AppError::msg(format!("Could not start installer: {e}")))?;
    if !output.status.success() {
        return Err(AppError::msg(format!(
            "Installer CLI failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        )));
    }
    let _ = copy_dir_best_effort(&dest_libs, lib_root);
    Ok(())
}

fn copy_dir_best_effort(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(from).into_iter().flatten() {
        let rel = match entry.path().strip_prefix(from) {
            Ok(r) if !r.as_os_str().is_empty() => r,
            _ => continue,
        };
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() && !dest.exists() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let _ = std::fs::copy(entry.path(), dest);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::maven_rel_path;
    use crate::instances::Instance;
    use crate::resolve::Lockfile;
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn substitutes_tokens_and_coords() {
        let mut data = HashMap::new();
        data.insert("SIDE".into(), "client".into());
        let out = subst("{SIDE} [net.foo:bar:1.0]", &data, Path::new("libs"));
        let normalized = out.replace('\\', "/");
        assert!(normalized.starts_with("client "));
        assert!(normalized.contains("net/foo/bar/1.0/bar-1.0.jar"));
    }

    #[test]
    fn maven_ext() {
        let p = maven_rel_path("net.minecraft:client:1.21.1:mappings@txt");
        assert_eq!(
            p,
            Path::new("net/minecraft/client/1.21.1/client-1.21.1-mappings.txt")
        );
    }

    /// Network smoke: vanilla 1.21.1 client jar + Forge installer processors/CLI.
    /// Skips Mojang assets. Does not launch Minecraft or start `tauri dev`.
    ///
    /// ```text
    /// set AUREUM_FORGE_SMOKE=1
    /// cargo test forge_1211_install_smoke -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "network smoke; set AUREUM_FORGE_SMOKE=1 and pass --ignored"]
    async fn forge_1211_install_smoke() {
        if std::env::var("AUREUM_FORGE_SMOKE").ok().as_deref() != Some("1") {
            eprintln!("skip: export AUREUM_FORGE_SMOKE=1 to run this smoke");
            return;
        }

        let http = reqwest::Client::builder()
            .user_agent("Aureum/0.1.0-forge-smoke")
            .build()
            .expect("http");
        let root = std::env::temp_dir().join(format!(
            "aureum-forge-smoke-{}",
            uuid::Uuid::new_v4()
        ));
        let game_dir = root.join("game");
        let cache = root.join("cache");
        std::fs::create_dir_all(&game_dir).unwrap();
        std::fs::create_dir_all(&cache).unwrap();

        let game_version = "1.21.1";
        let versions = install::list_game_versions(&http)
            .await
            .expect("list game versions");
        let meta = versions
            .iter()
            .find(|v| v.id == game_version)
            .expect("1.21.1 in Mojang manifest");
        let version_json: Value = http
            .get(&meta.url)
            .send()
            .await
            .expect("version json")
            .json()
            .await
            .expect("parse version json");
        let vdir = game_dir.join("versions").join(game_version);
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(
            vdir.join(format!("{game_version}.json")),
            serde_json::to_vec_pretty(&version_json).unwrap(),
        )
        .unwrap();
        let client = version_json
            .pointer("/downloads/client")
            .expect("client download");
        let url = client["url"].as_str().expect("client url");
        let sha = client["sha1"].as_str();
        let jar = vdir.join(format!("{game_version}.jar"));
        install::download_verified(&http, url, &jar, sha)
            .await
            .expect("download client jar");
        assert!(jar.is_file(), "vanilla client jar missing");

        let instance = Instance {
            id: "smoke-forge".into(),
            name: "Forge smoke".into(),
            loader: "forge".into(),
            game_version: game_version.into(),
            loader_version: None,
            game_dir: game_dir.to_string_lossy().into_owned(),
            java_path: None,
            memory_mb: 2048,
            jvm_args: None,
            keep_open: true,
            last_played: None,
            icon: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let mut lock = Lockfile::empty(&instance.id);
        install_forge_like(&http, &instance, game_version, &cache, &mut lock, None)
            .await
            .expect("forge install");

        let alias = game_dir
            .join("versions")
            .join(format!("forge-{game_version}"))
            .join(format!("forge-{game_version}.json"));
        assert!(
            alias.is_file(),
            "missing forge alias profile at {}",
            alias.display()
        );
        let alias_json: Value =
            serde_json::from_slice(&std::fs::read(&alias).unwrap()).expect("alias json");
        assert!(
            alias_json.get("id").is_some() || alias_json.get("inheritsFrom").is_some(),
            "alias profile incomplete"
        );

        let libs = cache.join("libraries");
        assert!(libs.is_dir(), "libraries cache missing");
        let mut forge_lib = false;
        for entry in walkdir::WalkDir::new(&libs).into_iter().flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains("forge") && name.ends_with(".jar") {
                forge_lib = true;
                break;
            }
        }
        assert!(forge_lib, "expected at least one forge*.jar under libraries");

        let _ = std::fs::remove_dir_all(&root);
    }
}
