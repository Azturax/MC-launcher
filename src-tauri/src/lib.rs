//! Aureum backend. Module boundaries match the intended crate split:
//! `instances` + `launch` → aureum-core
//! `auth` → aureum-auth
//! `catalog` → aureum-catalog
//! `resolve` → aureum-resolve

mod auth;
mod author;
mod catalog;
mod commands;
mod content;
mod db;
mod error;
mod forge;
mod install;
mod instances;
mod java;
mod launch;
mod logfiles;
mod media;
mod mods;
mod mrpack;
mod resolve;
mod settings;

use crate::auth::AuthConfig;
use crate::error::AppResult;
use crate::launch::LaunchTable;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

pub struct AppState {
    pub pool: SqlitePool,
    pub data_dir: PathBuf,
    pub instances_root: Arc<RwLock<PathBuf>>,
    pub http: Arc<RwLock<reqwest::Client>>,
    pub auth: AuthConfig,
    pub launches: LaunchTable,
}

fn load_dotenv() {
    let candidates = [
        PathBuf::from("../.env"),
        PathBuf::from(".env"),
        std::env::current_dir()
            .ok()
            .map(|p| p.join(".env"))
            .unwrap_or_else(|| PathBuf::from(".env")),
    ];
    for path in candidates {
        if path.is_file() {
            let _ = dotenvy::from_path(path);
            break;
        }
    }
}

fn build_http(proxy_url: Option<&str>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!(
            "Aureum/",
            env!("CARGO_PKG_VERSION"),
            " (dev.aureum.launcher; +https://modrinth.com)"
        ));
    if let Some(url) = proxy_url.filter(|s| !s.is_empty()) {
        if let Ok(proxy) = reqwest::Proxy::all(url) {
            builder = builder.proxy(proxy);
        }
    }
    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

async fn init_state(app: &tauri::AppHandle) -> AppResult<AppState> {
    load_dotenv();
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("aureum"));
    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(data_dir.join("cache"))?;

    let pool = db::connect(&data_dir.join("aureum.db")).await?;
    let stored_root = settings::get(&pool, "instances_root").await?;
    let instances_root = stored_root
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("instances"));
    std::fs::create_dir_all(&instances_root)?;

    let proxy = settings::get(&pool, "proxy_url").await?;
    let http = build_http(proxy.as_deref());

    Ok(AppState {
        pool,
        data_dir,
        instances_root: Arc::new(RwLock::new(instances_root)),
        http: Arc::new(RwLock::new(http)),
        auth: commands::auth_config(),
        launches: LaunchTable::default(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::try_init();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(init_state(&handle))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::accept_disclaimer,
            commands::get_settings,
            commands::set_setting,
            commands::list_instances,
            commands::get_instance,
            commands::create_instance,
            commands::update_instance,
            commands::upgrade_instance_version,
            commands::delete_instance,
            commands::clone_instance,
            commands::list_templates,
            commands::apply_template,
            commands::instance_status,
            commands::list_game_versions,
            commands::list_loader_versions,
            commands::install_instance,
            commands::launch_instance,
            commands::stop_instance,
            commands::list_running_instances,
            commands::get_log_tail,
            commands::open_instance_folder,
            commands::open_crash_reports,
            commands::list_profiles,
            commands::auth_status,
            commands::start_microsoft_login,
            commands::create_offline_profile,
            commands::delete_profile,
            commands::set_active_profile,
            commands::get_active_profile,
            commands::discover_java,
            commands::get_system_memory,
            commands::list_catalog_providers,
            commands::list_catalog_categories,
            commands::search_catalog,
            commands::get_catalog_project,
            commands::list_catalog_versions,
            commands::install_mod,
            commands::list_instance_mods,
            commands::open_mods_folder,
            commands::set_mod_pin,
            commands::set_mod_enabled,
            commands::remove_mod,
            commands::check_mod_updates,
            commands::reorder_mods,
            commands::import_mrpack,
            commands::export_mrpack,
            commands::pick_mrpack_file,
            commands::pick_mrpack_save,
            commands::install_modpack,
            commands::install_content,
            commands::list_instance_content,
            commands::check_content_updates,
            commands::apply_content_update,
            commands::remove_content,
            commands::open_content_folder,
            commands::list_instance_media,
            commands::read_media_preview,
            commands::delete_media_file,
            commands::open_media_folder,
            commands::pick_media_files,
            commands::import_media_files,
            commands::read_media_thumb,
            commands::list_log_files,
            commands::read_log_file,
            commands::delete_log_file,
            commands::open_log_folder,
            commands::open_external,
            commands::author_status,
            commands::start_modrinth_login,
            commands::connect_modrinth_pat,
            commands::disconnect_modrinth,
            commands::list_my_modrinth_projects,
            commands::link_author_draft,
            commands::import_modrinth_project,
            commands::create_modrinth_project,
            commands::pick_publish_file,
            commands::pick_author_image,
            commands::upload_author_icon,
            commands::upload_author_gallery,
            commands::list_author_gallery,
            commands::set_author_gallery_featured,
            commands::delete_author_gallery_image,
            commands::publish_author_version,
            commands::list_author_projects,
            commands::get_author_project,
            commands::create_author_project,
            commands::update_author_project,
            commands::delete_author_project,
            commands::author_publish_checklist,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aureum");
}
