use crate::auth::{self, AuthConfig, AuthStatus, Profile};
use crate::author::{
    self, AuthorProject, AuthorProjectPatch, AuthorStatus, CreateRemoteProjectRequest,
    GalleryImageEditRequest, NewAuthorProject, PublishChecklistItem, PublishVersionRequest,
    PublishVersionResult, RemoteGalleryImage, RemoteModrinthProject, UploadAuthorMediaRequest,
    UploadAuthorMediaResult,
};
use crate::catalog::{
    self, CatalogCategory, CatalogProvider, CatalogVersion, ModrinthAdapter, Page, ProjectDetail,
    ProjectHit, SearchFilters,
};
use crate::content::{self, ContentInstallResult, InstallContentRequest, InstalledContent};
use crate::error::AppResult;
use crate::install::{self, GameVersion, LoaderVersion};
use crate::instances::{self, Instance, InstancePatch, InstanceStatus, InstanceTemplate, NewInstance};
use crate::java::{self, JavaInstall, MemoryInfo};
use crate::launch;
use crate::logfiles::{self, LogFileEntry, LogFilePreview};
use crate::media::{self, MediaFile, MediaPreview};
use crate::mods::{self, InstallModRequest, InstalledMod};
use crate::mrpack::{self, ExportMrpackRequest, ImportMrpackRequest, InstallModpackRequest, MrpackImportResult};
use crate::resolve::ResolveResult;
use crate::settings;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub disclaimer: String,
    pub disclaimer_accepted: bool,
    pub data_dir: String,
    pub instances_root: String,
}

const DISCLAIMER: &str =
    "NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT";

#[tauri::command]
pub async fn get_app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let accepted = settings::get(&state.pool, "disclaimer_accepted")
        .await?
        .as_deref()
        == Some("true");
    let root = state.instances_root.read().await.clone();
    Ok(AppInfo {
        name: "Aureum".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        disclaimer: DISCLAIMER.into(),
        disclaimer_accepted: accepted,
        data_dir: state.data_dir.to_string_lossy().into(),
        instances_root: root.to_string_lossy().into(),
    })
}

#[tauri::command]
pub async fn accept_disclaimer(state: State<'_, AppState>) -> AppResult<()> {
    settings::set(&state.pool, "disclaimer_accepted", "true").await
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<HashMap<String, String>> {
    settings::get_all(&state.pool).await
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    settings::set(&state.pool, &key, &value).await?;
    if key == "instances_root" && !value.trim().is_empty() {
        let path = PathBuf::from(&value);
        std::fs::create_dir_all(&path)?;
        *state.instances_root.write().await = path;
    }
    if key == "proxy_url" {
        let mut builder = reqwest::Client::builder().user_agent(concat!(
            "Aureum/",
            env!("CARGO_PKG_VERSION"),
        ));
        if !value.trim().is_empty() {
            if let Ok(proxy) = reqwest::Proxy::all(value.trim()) {
                builder = builder.proxy(proxy);
            }
        }
        *state.http.write().await = builder.build().unwrap_or_else(|_| reqwest::Client::new());
    }
    Ok(())
}

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> AppResult<Vec<Instance>> {
    instances::list(&state.pool).await
}

#[tauri::command]
pub async fn get_instance(state: State<'_, AppState>, id: String) -> AppResult<Instance> {
    instances::get(&state.pool, &id).await
}

#[tauri::command]
pub async fn create_instance(state: State<'_, AppState>, mut new: NewInstance) -> AppResult<Instance> {
    let root = state.instances_root.read().await.clone();
    if new.memory_mb.is_none() {
        if let Some(mb) = settings::get(&state.pool, "memory_mb")
            .await?
            .and_then(|s| s.parse().ok())
        {
            new.memory_mb = Some(mb);
        }
    }
    if new.java_path.is_none() {
        new.java_path = settings::get(&state.pool, "java_path").await?;
    }
    if new.jvm_args.is_none() {
        new.jvm_args = settings::get(&state.pool, "jvm_args").await?;
    }
    instances::create(&state.pool, &root, new).await
}

#[tauri::command]
pub async fn update_instance(
    state: State<'_, AppState>,
    id: String,
    patch: InstancePatch,
) -> AppResult<Instance> {
    instances::update(&state.pool, &id, patch).await
}

/// Persist a new Minecraft / loader version, then run the normal install pipeline.
/// Does not wipe `mods/` — libraries and version metadata are refreshed in place.
#[tauri::command]
pub async fn upgrade_instance_version(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    game_version: String,
    loader: Option<String>,
    loader_version: Option<String>,
) -> AppResult<Instance> {
    let game_version = game_version.trim().to_string();
    if game_version.is_empty() {
        return Err(crate::error::AppError::msg("Minecraft version is required"));
    }
    let instance = instances::update(
        &state.pool,
        &id,
        InstancePatch {
            name: None,
            java_path: None,
            memory_mb: None,
            jvm_args: None,
            keep_open: None,
            icon: None,
            game_version: Some(game_version),
            loader,
            loader_version,
        },
    )
    .await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    install::install_instance(&app, &http, &cache, &instance).await?;
    instances::get(&state.pool, &id).await
}

#[tauri::command]
pub async fn delete_instance(
    state: State<'_, AppState>,
    id: String,
    remove_files: bool,
) -> AppResult<()> {
    instances::delete(&state.pool, &id, remove_files).await
}

#[tauri::command]
pub async fn clone_instance(state: State<'_, AppState>, id: String) -> AppResult<Instance> {
    let root = state.instances_root.read().await.clone();
    instances::clone_instance(&state.pool, &id, &root).await
}

#[tauri::command]
pub fn list_templates() -> Vec<InstanceTemplate> {
    instances::templates()
}

#[tauri::command]
pub async fn apply_template(
    state: State<'_, AppState>,
    template_id: String,
    name: Option<String>,
) -> AppResult<Instance> {
    let template = instances::templates()
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or_else(|| crate::error::AppError::msg("Unknown template"))?;
    let http = state.http.read().await.clone();
    let game_version = install::resolve_game_version(&http, &template.game_version).await?;
    create_instance(
        state,
        NewInstance {
            name: name.unwrap_or(template.name),
            loader: template.loader,
            game_version,
            loader_version: template.loader_version,
            java_path: None,
            memory_mb: None,
            jvm_args: None,
            keep_open: Some(true),
        },
    )
    .await
}

#[tauri::command]
pub async fn instance_status(state: State<'_, AppState>, id: String) -> AppResult<InstanceStatus> {
    let instance = instances::get(&state.pool, &id).await?;
    Ok(instances::status(&instance))
}

#[tauri::command]
pub async fn list_game_versions(state: State<'_, AppState>) -> AppResult<Vec<GameVersion>> {
    let http = state.http.read().await.clone();
    install::list_game_versions(&http).await
}

#[tauri::command]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    loader: String,
    game_version: String,
) -> AppResult<Vec<LoaderVersion>> {
    let http = state.http.read().await.clone();
    install::list_loader_versions(&http, &loader, &game_version).await
}

#[tauri::command]
pub async fn install_instance(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<String> {
    let instance = instances::get(&state.pool, &id).await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    install::install_instance(&app, &http, &cache, &instance).await
}

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    profile_id: Option<String>,
) -> AppResult<u32> {
    let mut instance = instances::get(&state.pool, &id).await?;
    if !instances::status(&instance).installed {
        let http = state.http.read().await.clone();
        let cache = state.data_dir.join("cache");
        install::install_instance(&app, &http, &cache, &instance).await?;
        instance = instances::get(&state.pool, &id).await?;
    }
    let profile_id = match profile_id {
        Some(p) => p,
        None => settings::get(&state.pool, "active_profile")
            .await?
            .ok_or_else(|| {
                crate::error::AppError::msg("Select a profile in Accounts before playing")
            })?,
    };
    let http = state.http.read().await.clone();
    let session =
        auth::session_for_launch(&state.pool, &http, &state.auth, &profile_id).await?;
    instances::touch_last_played(&state.pool, &id).await?;
    let cache = state.data_dir.join("cache");
    let java_override = settings::get(&state.pool, "java_path").await?;
    let extra_jvm = settings::get(&state.pool, "jvm_args").await?;
    launch::launch(
        &app,
        &instance,
        &session,
        &cache,
        java_override.as_deref(),
        extra_jvm.as_deref(),
        &state.launches,
    )
}

#[tauri::command]
pub fn stop_instance(state: State<'_, AppState>, id: String) -> AppResult<()> {
    launch::stop(&state.launches, &id)
}

#[tauri::command]
pub fn list_running_instances(state: State<'_, AppState>) -> Vec<String> {
    launch::list_running(&state.launches)
}

#[tauri::command]
pub async fn get_log_tail(
    state: State<'_, AppState>,
    id: String,
    lines: Option<usize>,
) -> AppResult<Vec<String>> {
    let instance = instances::get(&state.pool, &id).await?;
    launch::tail_log(&instance, lines.unwrap_or(80))
}

#[tauri::command]
pub async fn open_instance_folder(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let instance = instances::get(&state.pool, &id).await?;
    open::that(&instance.game_dir)
        .map_err(|e| crate::error::AppError::msg(format!("Could not open folder: {e}")))
}

#[tauri::command]
pub async fn open_crash_reports(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let instance = instances::get(&state.pool, &id).await?;
    let dir = launch::crash_reports_dir(&instance);
    std::fs::create_dir_all(&dir)?;
    open::that(dir).map_err(|e| crate::error::AppError::msg(format!("Could not open folder: {e}")))
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> AppResult<Vec<Profile>> {
    auth::list_profiles(&state.pool).await
}

#[tauri::command]
pub fn auth_status(state: State<'_, AppState>) -> AuthStatus {
    AuthStatus::from_config(&state.auth)
}

#[tauri::command]
pub async fn start_microsoft_login(state: State<'_, AppState>) -> AppResult<Profile> {
    let http = state.http.read().await.clone();
    let profile = auth::microsoft_login(&state.pool, &http, &state.auth).await?;
    settings::set(&state.pool, "active_profile", &profile.id).await?;
    Ok(profile)
}

#[tauri::command]
pub async fn create_offline_profile(
    state: State<'_, AppState>,
    display_name: String,
) -> AppResult<Profile> {
    let profile = auth::create_offline_profile(&state.pool, &display_name).await?;
    if settings::get(&state.pool, "active_profile").await?.is_none() {
        settings::set(&state.pool, "active_profile", &profile.id).await?;
    }
    Ok(profile)
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    auth::delete_profile(&state.pool, &id).await?;
    if settings::get(&state.pool, "active_profile").await?.as_deref() == Some(&id) {
        settings::set(&state.pool, "active_profile", "").await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_active_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let _ = auth::get_profile(&state.pool, &id).await?;
    settings::set(&state.pool, "active_profile", &id).await
}

#[tauri::command]
pub async fn get_active_profile(state: State<'_, AppState>) -> AppResult<Option<Profile>> {
    match settings::get(&state.pool, "active_profile").await? {
        Some(id) if !id.is_empty() => Ok(Some(auth::get_profile(&state.pool, &id).await?)),
        _ => Ok(None),
    }
}

#[tauri::command]
pub fn discover_java() -> AppResult<Vec<JavaInstall>> {
    java::discover()
}

#[tauri::command]
pub fn get_system_memory() -> MemoryInfo {
    java::system_memory()
}

#[tauri::command]
pub fn list_catalog_providers() -> Vec<CatalogProvider> {
    catalog::providers()
}

#[tauri::command]
pub async fn list_catalog_categories(
    state: State<'_, AppState>,
) -> AppResult<Vec<CatalogCategory>> {
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    catalog::list_categories(&http, &state.pool, &cache).await
}

#[tauri::command]
pub async fn search_catalog(
    state: State<'_, AppState>,
    filters: SearchFilters,
) -> AppResult<Page<ProjectHit>> {
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    catalog::search_modrinth(&http, &state.pool, &cache, &filters).await
}

#[tauri::command]
pub async fn get_catalog_project(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<ProjectDetail> {
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    ModrinthAdapter
        .project(&http, &state.pool, &cache, &id)
        .await
}

#[tauri::command]
pub async fn list_catalog_versions(
    state: State<'_, AppState>,
    id: String,
    loaders: Option<Vec<String>>,
    game_versions: Option<Vec<String>>,
    channel: Option<String>,
) -> AppResult<Vec<CatalogVersion>> {
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    ModrinthAdapter
        .versions(
            &http,
            &state.pool,
            &cache,
            &id,
            &loaders.unwrap_or_default(),
            &game_versions.unwrap_or_default(),
            channel.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn install_mod(
    state: State<'_, AppState>,
    request: InstallModRequest,
) -> AppResult<ResolveResult> {
    let instance = instances::get(&state.pool, &request.instance_id).await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    mods::install_mod(&http, &state.pool, &cache, &instance, request).await
}

#[tauri::command]
pub async fn list_instance_mods(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<Vec<InstalledMod>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    mods::list_installed_with_disk(&state.pool, &instance).await
}

#[tauri::command]
pub async fn open_mods_folder(state: State<'_, AppState>, instance_id: String) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    mods::open_mods_folder(&instance)
}

#[tauri::command]
pub async fn set_mod_pin(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    pinned: bool,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    mods::set_pin(&state.pool, &instance, &project_id, pinned).await
}

#[tauri::command]
pub async fn set_mod_enabled(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
    enabled: bool,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    mods::set_enabled(&state.pool, &instance, &project_id, enabled).await
}

#[tauri::command]
pub async fn remove_mod(
    state: State<'_, AppState>,
    instance_id: String,
    project_id: String,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    mods::remove_mod(&state.pool, &instance, &project_id).await
}

#[tauri::command]
pub async fn check_mod_updates(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<Vec<InstalledMod>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    mods::check_updates(&http, &state.pool, &cache, &instance).await
}

#[tauri::command]
pub async fn reorder_mods(
    state: State<'_, AppState>,
    instance_id: String,
    project_ids: Vec<String>,
) -> AppResult<Vec<InstalledMod>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    mods::reorder_mods(&state.pool, &instance, &project_ids).await
}

#[tauri::command]
pub async fn import_mrpack(
    state: State<'_, AppState>,
    request: ImportMrpackRequest,
) -> AppResult<MrpackImportResult> {
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    let root = state.instances_root.read().await.clone();
    mrpack::import_mrpack(&http, &state.pool, &cache, &root, request).await
}

#[tauri::command]
pub async fn export_mrpack(
    state: State<'_, AppState>,
    request: ExportMrpackRequest,
) -> AppResult<String> {
    let instance = instances::get(&state.pool, &request.instance_id).await?;
    mrpack::export_mrpack(&state.pool, &instance, request).await
}

#[tauri::command]
pub async fn pick_mrpack_file() -> AppResult<Option<String>> {
    let path = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Modrinth pack", &["mrpack", "zip"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| crate::error::AppError::msg(e.to_string()))?;
    Ok(path)
}

#[tauri::command]
pub async fn pick_mrpack_save(default_name: Option<String>) -> AppResult<Option<String>> {
    let name = default_name.unwrap_or_else(|| "pack.mrpack".into());
    let path = tauri::async_runtime::spawn_blocking(move || {
        rfd::FileDialog::new()
            .add_filter("Modrinth pack", &["mrpack"])
            .set_file_name(&name)
            .save_file()
            .map(|p| {
                let mut s = p.to_string_lossy().into_owned();
                if !s.to_ascii_lowercase().ends_with(".mrpack") {
                    s.push_str(".mrpack");
                }
                s
            })
    })
    .await
    .map_err(|e| crate::error::AppError::msg(e.to_string()))?;
    Ok(path)
}

#[tauri::command]
pub async fn install_modpack(
    state: State<'_, AppState>,
    request: InstallModpackRequest,
) -> AppResult<MrpackImportResult> {
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    let root = state.instances_root.read().await.clone();
    mrpack::install_from_catalog(&http, &state.pool, &cache, &root, request).await
}

#[tauri::command]
pub async fn install_content(
    state: State<'_, AppState>,
    request: InstallContentRequest,
) -> AppResult<ContentInstallResult> {
    let instance = instances::get(&state.pool, &request.instance_id).await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    content::install_content(&http, &state.pool, &cache, &instance, request).await
}

#[tauri::command]
pub async fn list_instance_content(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<Vec<InstalledContent>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    content::list_content(&instance)
}

#[tauri::command]
pub async fn check_content_updates(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<Vec<InstalledContent>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    content::check_updates(&http, &state.pool, &cache, &instance).await
}

#[tauri::command]
pub async fn apply_content_update(
    state: State<'_, AppState>,
    instance_id: String,
    content_id: String,
) -> AppResult<ContentInstallResult> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    let http = state.http.read().await.clone();
    let cache = state.data_dir.join("cache");
    content::apply_update(&http, &state.pool, &cache, &instance, &content_id).await
}

#[tauri::command]
pub async fn remove_content(
    state: State<'_, AppState>,
    instance_id: String,
    content_id: String,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    content::remove_content(&instance, &content_id)
}

#[tauri::command]
pub async fn open_content_folder(
    state: State<'_, AppState>,
    instance_id: String,
    project_type: Option<String>,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    content::open_content_folder(&instance, project_type.as_deref())
}

#[tauri::command]
pub async fn list_instance_media(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<Vec<MediaFile>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    media::list_media(&instance)
}

#[tauri::command]
pub async fn read_media_preview(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> AppResult<MediaPreview> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    media::read_preview(&instance, &name)
}

#[tauri::command]
pub async fn read_media_thumb(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> AppResult<MediaPreview> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    media::read_thumb(&instance, &name)
}

#[tauri::command]
pub async fn delete_media_file(
    state: State<'_, AppState>,
    instance_id: String,
    name: String,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    media::delete_media(&instance, &name)
}

#[tauri::command]
pub async fn open_media_folder(
    state: State<'_, AppState>,
    instance_id: String,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    media::open_media_folder(&instance)
}

#[tauri::command]
pub fn pick_media_files() -> AppResult<Vec<String>> {
    media::pick_media_files()
}

#[tauri::command]
pub async fn import_media_files(
    state: State<'_, AppState>,
    instance_id: String,
    paths: Vec<String>,
) -> AppResult<Vec<MediaFile>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    media::import_media(&instance, &paths)
}

#[tauri::command]
pub async fn list_log_files(
    state: State<'_, AppState>,
    instance_id: String,
    folder: String,
) -> AppResult<Vec<LogFileEntry>> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    logfiles::list_files(&instance, &folder)
}

#[tauri::command]
pub async fn read_log_file(
    state: State<'_, AppState>,
    instance_id: String,
    folder: String,
    name: String,
) -> AppResult<LogFilePreview> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    logfiles::read_preview(&instance, &folder, &name)
}

#[tauri::command]
pub async fn delete_log_file(
    state: State<'_, AppState>,
    instance_id: String,
    folder: String,
    name: String,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    logfiles::delete_file(&instance, &folder, &name)
}

#[tauri::command]
pub async fn open_log_folder(
    state: State<'_, AppState>,
    instance_id: String,
    folder: String,
) -> AppResult<()> {
    let instance = instances::get(&state.pool, &instance_id).await?;
    logfiles::open_folder(&instance, &folder)
}

#[tauri::command]
pub fn open_external(url: String) -> AppResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(crate::error::AppError::msg("Only http(s) URLs can be opened"));
    }
    open::that(url).map_err(|e| crate::error::AppError::msg(e.to_string()))
}

#[tauri::command]
pub async fn author_status(state: State<'_, AppState>) -> AppResult<AuthorStatus> {
    author::status(&state.pool).await
}

#[tauri::command]
pub async fn start_modrinth_login(state: State<'_, AppState>) -> AppResult<AuthorStatus> {
    let http = state.http.read().await.clone();
    author::connect(&state.pool, &http).await
}

#[tauri::command]
pub async fn connect_modrinth_pat(
    state: State<'_, AppState>,
    pat: String,
) -> AppResult<AuthorStatus> {
    let http = state.http.read().await.clone();
    author::connect_with_pat(&state.pool, &http, pat).await
}

#[tauri::command]
pub async fn disconnect_modrinth(state: State<'_, AppState>) -> AppResult<AuthorStatus> {
    author::disconnect(&state.pool).await
}

#[tauri::command]
pub async fn list_my_modrinth_projects(
    state: State<'_, AppState>,
) -> AppResult<Vec<RemoteModrinthProject>> {
    let http = state.http.read().await.clone();
    author::list_remote_projects(&state.pool, &http).await
}

#[tauri::command]
pub async fn link_author_draft(
    state: State<'_, AppState>,
    draft_id: String,
    remote_id: String,
) -> AppResult<AuthorProject> {
    let http = state.http.read().await.clone();
    let remotes = author::list_remote_projects(&state.pool, &http).await?;
    let remote = remotes
        .into_iter()
        .find(|p| p.id == remote_id)
        .ok_or_else(|| crate::error::AppError::msg("Remote project not found"))?;
    author::link_draft_to_remote(&state.pool, &draft_id, &remote).await
}

#[tauri::command]
pub async fn import_modrinth_project(
    state: State<'_, AppState>,
    remote_id: String,
) -> AppResult<AuthorProject> {
    let http = state.http.read().await.clone();
    let remotes = author::list_remote_projects(&state.pool, &http).await?;
    let remote = remotes
        .into_iter()
        .find(|p| p.id == remote_id)
        .ok_or_else(|| crate::error::AppError::msg("Remote project not found"))?;
    author::import_remote_as_draft(&state.pool, &remote).await
}

#[tauri::command]
pub async fn create_modrinth_project(
    state: State<'_, AppState>,
    request: CreateRemoteProjectRequest,
) -> AppResult<AuthorProject> {
    let http = state.http.read().await.clone();
    author::create_remote_from_draft(&state.pool, &http, request).await
}

#[tauri::command]
pub fn pick_publish_file() -> AppResult<Option<String>> {
    author::pick_publish_file()
}

#[tauri::command]
pub fn pick_author_image() -> AppResult<Option<String>> {
    author::pick_image_file()
}

#[tauri::command]
pub async fn upload_author_icon(
    state: State<'_, AppState>,
    request: UploadAuthorMediaRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let http = state.http.read().await.clone();
    author::upload_icon(&state.pool, &http, request).await
}

#[tauri::command]
pub async fn upload_author_gallery(
    state: State<'_, AppState>,
    request: UploadAuthorMediaRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let http = state.http.read().await.clone();
    author::upload_gallery(&state.pool, &http, request).await
}

#[tauri::command]
pub async fn list_author_gallery(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<RemoteGalleryImage>> {
    let http = state.http.read().await.clone();
    author::list_gallery(&state.pool, &http, &project_id).await
}

#[tauri::command]
pub async fn set_author_gallery_featured(
    state: State<'_, AppState>,
    request: GalleryImageEditRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let http = state.http.read().await.clone();
    author::set_gallery_featured(&state.pool, &http, request).await
}

#[tauri::command]
pub async fn delete_author_gallery_image(
    state: State<'_, AppState>,
    request: GalleryImageEditRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let http = state.http.read().await.clone();
    author::delete_gallery_image(&state.pool, &http, request).await
}

#[tauri::command]
pub async fn publish_author_version(
    state: State<'_, AppState>,
    request: PublishVersionRequest,
) -> AppResult<PublishVersionResult> {
    let http = state.http.read().await.clone();
    author::publish_version(&state.pool, &http, request).await
}

#[tauri::command]
pub async fn list_author_projects(state: State<'_, AppState>) -> AppResult<Vec<AuthorProject>> {
    author::list_projects(&state.pool).await
}

#[tauri::command]
pub async fn get_author_project(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<AuthorProject> {
    author::get_project(&state.pool, &id).await
}

#[tauri::command]
pub async fn create_author_project(
    state: State<'_, AppState>,
    new: NewAuthorProject,
) -> AppResult<AuthorProject> {
    author::create_project(&state.pool, new).await
}

#[tauri::command]
pub async fn update_author_project(
    state: State<'_, AppState>,
    id: String,
    patch: AuthorProjectPatch,
) -> AppResult<AuthorProject> {
    author::update_project(&state.pool, &id, patch).await
}

#[tauri::command]
pub async fn delete_author_project(state: State<'_, AppState>, id: String) -> AppResult<()> {
    author::delete_project(&state.pool, &id).await
}

#[tauri::command]
pub async fn author_publish_checklist(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<PublishChecklistItem>> {
    let project = author::get_project(&state.pool, &id).await?;
    Ok(author::publish_checklist(&state.pool, &project).await)
}

pub fn auth_config() -> AuthConfig {
    AuthConfig::from_env()
}
