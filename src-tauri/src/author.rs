//! Author Dashboard: local drafts + Modrinth OAuth (system browser) + version publish.
//! Access tokens live in the OS keychain only. The renderer never sees them.
//!
//! Modrinth's token exchange requires a client secret (not public-client PKCE).
//! Register an app at https://modrinth.com/settings/apps and set env vars.

use crate::auth::{delete_secret, read_secret, store_secret, wait_for_code};
use crate::catalog::MODRINTH_API;
use crate::error::{AppError, AppResult};
use crate::settings;
use chrono::{Duration, Utc};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const ACCESS_KEY: &str = "modrinth-access-token";
const SETTING_USER: &str = "modrinth_username";
const SETTING_USER_ID: &str = "modrinth_user_id";
const SETTING_EXPIRES: &str = "modrinth_token_expires";
const SETTING_KIND: &str = "modrinth_auth_kind"; // oauth | pat | dry-run

const DEFAULT_SCOPES: &str =
    "USER_READ+PROJECT_READ+PROJECT_WRITE+PROJECT_CREATE+VERSION_CREATE+VERSION_READ+VERSION_WRITE";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorProject {
    pub id: String,
    pub title: String,
    pub slug: Option<String>,
    pub summary: String,
    pub description: String,
    pub project_type: String,
    /// `draft` | `checklist` | `published`
    pub status: String,
    pub modrinth_id: Option<String>,
    pub local_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAuthorProject {
    pub title: String,
    pub slug: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub project_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorProjectPatch {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub project_type: Option<String>,
    pub status: Option<String>,
    pub modrinth_id: Option<String>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorStatus {
    pub oauth_configured: bool,
    pub secret_configured: bool,
    pub connected: bool,
    /// Token present in keychain but past `expires_at` (OAuth access tokens).
    pub expired: bool,
    pub dry_run: bool,
    pub username: Option<String>,
    pub user_id: Option<String>,
    pub expires_at: Option<String>,
    pub redirect_uri: String,
    pub scopes: String,
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRemoteProjectRequest {
    pub draft_id: String,
    /// Featured categories (e.g. utility). Loaders go in categories for mods historically;
    /// we also send client_side / server_side.
    pub categories: Option<Vec<String>>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub license_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteModrinthProject {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishVersionRequest {
    pub project_id: String,
    /// Local draft id to mark published / keep link (optional).
    pub draft_id: Option<String>,
    pub name: String,
    pub version_number: String,
    pub changelog: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// `release` | `beta` | `alpha`
    pub version_type: Option<String>,
    pub featured: Option<bool>,
    /// Absolute path to jar/zip/mrpack chosen via file picker.
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishVersionResult {
    pub version_id: String,
    pub project_id: String,
    pub version_number: String,
    pub project_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadAuthorMediaRequest {
    pub project_id: String,
    pub file_path: String,
    /// Gallery only: mark as featured.
    pub featured: Option<bool>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadAuthorMediaResult {
    pub project_id: String,
    pub kind: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteGalleryImage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
    pub ordering: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImageEditRequest {
    pub project_id: String,
    /// Full CDN URL of the gallery image (from list/get project).
    pub url: String,
    pub featured: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishChecklistItem {
    pub id: String,
    pub label: String,
    pub done: bool,
}

struct ModrinthOauthConfig {
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_port: u16,
    scopes: String,
    dry_run: bool,
}

impl ModrinthOauthConfig {
    fn from_env() -> Self {
        let client_id = std::env::var("MODRINTH_CLIENT_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let client_secret = std::env::var("MODRINTH_CLIENT_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let redirect_port = std::env::var("MODRINTH_OAUTH_REDIRECT_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(17891);
        let scopes = std::env::var("MODRINTH_OAUTH_SCOPES")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_SCOPES.into());
        let forced_dry = std::env::var("MODRINTH_AUTH_DRY_RUN")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            dry_run: forced_dry,
            client_id,
            client_secret,
            redirect_port,
            scopes,
        }
    }

    fn redirect_uri(&self) -> String {
        format!(
            "http://127.0.0.1:{}/modrinth/callback",
            self.redirect_port
        )
    }

    fn oauth_ready(&self) -> bool {
        self.client_id.is_some() && self.client_secret.is_some() && !self.dry_run
    }
}

pub async fn status(pool: &SqlitePool) -> AppResult<AuthorStatus> {
    let cfg = ModrinthOauthConfig::from_env();
    let has_secret = read_secret(ACCESS_KEY).is_ok();
    let kind = settings::get(pool, SETTING_KIND)
        .await?
        .unwrap_or_default();
    let expires_at = settings::get(pool, SETTING_EXPIRES)
        .await?
        .filter(|s| !s.is_empty());
    let expired = if kind == "expired" {
        true
    } else if kind == "pat" || kind == "dry-run" {
        false
    } else if let Some(ref exp) = expires_at {
        chrono::DateTime::parse_from_rfc3339(exp)
            .map(|t| t < Utc::now())
            .unwrap_or(false)
    } else {
        false
    };
    if expired && has_secret {
        let _ = delete_secret(ACCESS_KEY);
    }
    let connected = read_secret(ACCESS_KEY).is_ok() && !expired;
    let username = settings::get(pool, SETTING_USER)
        .await?
        .filter(|s| !s.is_empty());
    let user_id = settings::get(pool, SETTING_USER_ID)
        .await?
        .filter(|s| !s.is_empty());
    let note = if cfg.dry_run {
        "MODRINTH_AUTH_DRY_RUN=1 — Connect simulates a creator session; no real Modrinth API calls."
            .into()
    } else if expired {
        format!(
            "Modrinth session expired{}. Reconnect to continue publishing.",
            username
                .as_ref()
                .map(|u| format!(" for {u}"))
                .unwrap_or_default()
        )
    } else if cfg.oauth_ready() {
        if connected {
            format!(
                "Connected as {}. Tokens stay in the OS keychain.",
                username.as_deref().unwrap_or("Modrinth user")
            )
        } else {
            "Modrinth OAuth is configured. Connect opens your browser (authorization-code + client secret on token exchange)."
                .into()
        }
    } else if cfg.client_id.is_some() && cfg.client_secret.is_none() {
        "MODRINTH_CLIENT_ID is set, but MODRINTH_CLIENT_SECRET is missing. Modrinth requires the secret for token exchange (kept in Rust / .env only). You can also paste a Personal Access Token."
            .into()
    } else {
        "Add MODRINTH_CLIENT_ID + MODRINTH_CLIENT_SECRET to .env (redirect http://127.0.0.1:17891/modrinth/callback), or connect with a Modrinth PAT. Drafts still work offline."
            .into()
    };
    Ok(AuthorStatus {
        oauth_configured: cfg.client_id.is_some(),
        secret_configured: cfg.client_secret.is_some(),
        connected,
        expired,
        dry_run: cfg.dry_run,
        username,
        user_id,
        expires_at,
        redirect_uri: cfg.redirect_uri(),
        scopes: cfg.scopes,
        note,
    })
}

pub async fn connect(pool: &SqlitePool, http: &reqwest::Client) -> AppResult<AuthorStatus> {
    let cfg = ModrinthOauthConfig::from_env();
    if cfg.dry_run {
        store_secret(ACCESS_KEY, "dry-run-token")?;
        settings::set(pool, SETTING_USER, "DryRunCreator").await?;
        settings::set(pool, SETTING_USER_ID, "dry-run").await?;
        settings::set(pool, SETTING_KIND, "dry-run").await?;
        let expires = (Utc::now() + Duration::days(30)).to_rfc3339();
        settings::set(pool, SETTING_EXPIRES, &expires).await?;
        return status(pool).await;
    }
    let client_id = cfg
        .client_id
        .as_deref()
        .ok_or_else(|| AppError::msg("Missing MODRINTH_CLIENT_ID"))?;
    let client_secret = cfg.client_secret.as_deref().ok_or_else(|| {
        AppError::msg(
            "Missing MODRINTH_CLIENT_SECRET (required by Modrinth token exchange). Or use connect_with_pat.",
        )
    })?;

    let redirect = cfg.redirect_uri();
    let state = Uuid::new_v4().to_string();
    let auth_url = format!(
        "https://modrinth.com/auth/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode(&cfg.scopes),
        urlencoding::encode(&state),
    );
    open::that(&auth_url).map_err(|e| AppError::msg(format!("Could not open browser: {e}")))?;
    let port = cfg.redirect_port;
    let expected = state.clone();
    let code = tokio::task::spawn_blocking(move || wait_for_code(port, &expected))
        .await
        .map_err(|e| AppError::msg(e.to_string()))??;

    let token = exchange_code(http, client_id, client_secret, &redirect, &code).await?;
    store_secret(ACCESS_KEY, &token.access_token)?;
    let expires = (Utc::now() + Duration::seconds(token.expires_in.unwrap_or(3600) as i64))
        .to_rfc3339();
    settings::set(pool, SETTING_EXPIRES, &expires).await?;
    settings::set(pool, SETTING_KIND, "oauth").await?;

    let user = fetch_user(http, &token.access_token).await?;
    settings::set(pool, SETTING_USER, &user.username).await?;
    settings::set(pool, SETTING_USER_ID, &user.id).await?;
    status(pool).await
}

pub async fn connect_with_pat(
    pool: &SqlitePool,
    http: &reqwest::Client,
    pat: String,
) -> AppResult<AuthorStatus> {
    let pat = pat.trim().to_string();
    if pat.is_empty() {
        return Err(AppError::msg("Personal access token is empty"));
    }
    let cfg = ModrinthOauthConfig::from_env();
    if cfg.dry_run {
        return connect(pool, http).await;
    }
    let user = fetch_user(http, &pat).await?;
    store_secret(ACCESS_KEY, &pat)?;
    settings::set(pool, SETTING_USER, &user.username).await?;
    settings::set(pool, SETTING_USER_ID, &user.id).await?;
    settings::set(pool, SETTING_KIND, "pat").await?;
    let expires = (Utc::now() + Duration::days(365)).to_rfc3339();
    settings::set(pool, SETTING_EXPIRES, &expires).await?;
    status(pool).await
}

pub async fn disconnect(pool: &SqlitePool) -> AppResult<AuthorStatus> {
    let _ = delete_secret(ACCESS_KEY);
    settings::set(pool, SETTING_USER, "").await?;
    settings::set(pool, SETTING_USER_ID, "").await?;
    settings::set(pool, SETTING_EXPIRES, "").await?;
    settings::set(pool, SETTING_KIND, "").await?;
    status(pool).await
}

async fn access_token(pool: &SqlitePool) -> AppResult<String> {
    let st = status(pool).await?;
    if st.expired {
        return Err(AppError::msg(
            "Modrinth session expired. Reconnect from the Author page.",
        ));
    }
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if kind == "dry-run" {
        return Ok("dry-run-token".into());
    }
    read_secret(ACCESS_KEY).map_err(|_| {
        AppError::msg("Not connected to Modrinth. Use Connect Modrinth or a personal access token.")
    })
}

async fn mark_session_expired(pool: &SqlitePool) -> AppResult<()> {
    let _ = delete_secret(ACCESS_KEY);
    settings::set(pool, SETTING_KIND, "expired").await?;
    settings::set(pool, SETTING_EXPIRES, "1970-01-01T00:00:00Z").await?;
    Ok(())
}

/// On 401, clear the keychain token and surface a reconnect prompt.
async fn require_modrinth_ok(
    pool: &SqlitePool,
    res: reqwest::Response,
    context: &str,
) -> AppResult<reqwest::Response> {
    let code = res.status();
    if code.as_u16() == 401 {
        let _ = mark_session_expired(pool).await;
        return Err(AppError::msg(format!(
            "{context}: Modrinth session expired (401). Reconnect from the Author page."
        )));
    }
    if !code.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(AppError::msg(format!("{context}: {body}")));
    }
    Ok(res)
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: Option<String>,
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModrinthUser {
    id: String,
    username: String,
}

async fn exchange_code(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    redirect: &str,
    code: &str,
) -> AppResult<TokenResponse> {
    let form = [
        ("code", code),
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("grant_type", "authorization_code"),
    ];
    let res = http
        .post("https://api.modrinth.com/_internal/oauth/token")
        .header("Authorization", client_secret)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(AppError::msg(format!(
            "Modrinth token exchange failed: {body}"
        )));
    }
    Ok(res.json().await?)
}

async fn fetch_user(http: &reqwest::Client, token: &str) -> AppResult<ModrinthUser> {
    let res = http
        .get(format!("{MODRINTH_API}/user"))
        .header("Authorization", token)
        .send()
        .await?;
    if res.status().as_u16() == 401 {
        return Err(AppError::msg(
            "Modrinth rejected the token (401). Check the PAT or reconnect OAuth.",
        ));
    }
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(AppError::msg(format!(
            "Could not load Modrinth user: {body}"
        )));
    }
    Ok(res.json().await?)
}

pub async fn list_remote_projects(
    pool: &SqlitePool,
    http: &reqwest::Client,
) -> AppResult<Vec<RemoteModrinthProject>> {
    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        return Ok(vec![RemoteModrinthProject {
            id: "dryrunproj".into(),
            slug: "aureum-dry-run-mod".into(),
            title: "Aureum Dry-Run Mod".into(),
            description: "Simulated Modrinth project for author UI testing.".into(),
            project_type: "mod".into(),
            icon_url: None,
        }]);
    }
    let token = access_token(pool).await?;
    let user_id = settings::get(pool, SETTING_USER_ID)
        .await?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::msg("Missing Modrinth user id — reconnect"))?;
    let url = format!(
        "{MODRINTH_API}/user/{}/projects",
        urlencoding::encode(&user_id)
    );
    let res = http
        .get(&url)
        .header("Authorization", &token)
        .send()
        .await?;
    let res = require_modrinth_ok(pool, res, "Failed to list Modrinth projects").await?;
    let v: serde_json::Value = res.json().await?;
    let list = v
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| {
            Some(RemoteModrinthProject {
                id: p.get("id")?.as_str()?.to_string(),
                slug: p.get("slug")?.as_str()?.to_string(),
                title: p
                    .get("title")
                    .and_then(|s| s.as_str())
                    .unwrap_or("Untitled")
                    .to_string(),
                description: p
                    .get("description")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string(),
                project_type: p
                    .get("project_type")
                    .and_then(|s| s.as_str())
                    .unwrap_or("mod")
                    .to_string(),
                icon_url: p
                    .get("icon_url")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
            })
        })
        .collect();
    Ok(list)
}

pub async fn link_draft_to_remote(
    pool: &SqlitePool,
    draft_id: &str,
    remote: &RemoteModrinthProject,
) -> AppResult<AuthorProject> {
    update_project(
        pool,
        draft_id,
        AuthorProjectPatch {
            title: Some(remote.title.clone()),
            slug: Some(remote.slug.clone()),
            summary: Some(remote.description.clone()),
            description: None,
            project_type: Some(remote.project_type.clone()),
            status: Some("checklist".into()),
            modrinth_id: Some(remote.id.clone()),
            local_path: None,
        },
    )
    .await
}

pub async fn import_remote_as_draft(
    pool: &SqlitePool,
    remote: &RemoteModrinthProject,
) -> AppResult<AuthorProject> {
    let existing = list_projects(pool).await?;
    if let Some(found) = existing
        .into_iter()
        .find(|p| p.modrinth_id.as_deref() == Some(remote.id.as_str()))
    {
        return Ok(found);
    }
    let created = create_project(
        pool,
        NewAuthorProject {
            title: remote.title.clone(),
            slug: Some(remote.slug.clone()),
            summary: Some(remote.description.clone()),
            description: Some(String::new()),
            project_type: Some(remote.project_type.clone()),
        },
    )
    .await?;
    update_project(
        pool,
        &created.id,
        AuthorProjectPatch {
            title: None,
            slug: None,
            summary: None,
            description: None,
            project_type: None,
            status: Some("checklist".into()),
            modrinth_id: Some(remote.id.clone()),
            local_path: None,
        },
    )
    .await
}

/// Create a Modrinth project from a local draft via `POST /project` (multipart `data` JSON).
pub async fn create_remote_from_draft(
    pool: &SqlitePool,
    http: &reqwest::Client,
    req: CreateRemoteProjectRequest,
) -> AppResult<AuthorProject> {
    let draft = get_project(pool, &req.draft_id).await?;
    if draft
        .modrinth_id
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return Err(AppError::msg(
            "Draft is already linked to a Modrinth project. Unlink first or publish a version.",
        ));
    }
    let slug = draft
        .slug
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 3)
        .ok_or_else(|| AppError::msg("Set a slug (3+ chars) before creating on Modrinth"))?;
    let title = draft.title.trim();
    if title.is_empty() {
        return Err(AppError::msg("Title is required"));
    }
    let summary = draft.summary.trim();
    if summary.len() < 3 {
        return Err(AppError::msg(
            "Short summary must be at least a few characters",
        ));
    }
    let body = if draft.description.trim().is_empty() {
        summary.to_string()
    } else {
        draft.description.clone()
    };
    let project_type = draft.project_type.clone();
    let categories = req.categories.unwrap_or_else(|| match project_type.as_str() {
        "mod" | "modpack" => vec!["utility".into()],
        "resourcepack" => vec!["16x".into()],
        "shader" => vec!["realistic".into()],
        "datapack" => vec!["utility".into()],
        _ => vec!["utility".into()],
    });
    let (client_side, server_side) = match project_type.as_str() {
        "resourcepack" | "shader" => (
            req.client_side.unwrap_or_else(|| "required".into()),
            req.server_side.unwrap_or_else(|| "unsupported".into()),
        ),
        "datapack" => (
            req.client_side.unwrap_or_else(|| "unsupported".into()),
            req.server_side.unwrap_or_else(|| "required".into()),
        ),
        _ => (
            req.client_side.unwrap_or_else(|| "required".into()),
            req.server_side.unwrap_or_else(|| "optional".into()),
        ),
    };
    let license_id = req
        .license_id
        .unwrap_or_else(|| "LicenseRef-Unknown".into());

    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        let fake_id = format!("dry{}", &Uuid::new_v4().to_string()[..8]);
        return update_project(
            pool,
            &draft.id,
            AuthorProjectPatch {
                title: None,
                slug: Some(slug),
                summary: None,
                description: None,
                project_type: None,
                status: Some("checklist".into()),
                modrinth_id: Some(fake_id),
                local_path: None,
            },
        )
        .await;
    }

    let token = access_token(pool).await?;
    let data = serde_json::json!({
        "slug": slug,
        "title": title,
        "description": summary,
        "body": body,
        "categories": categories,
        "client_side": client_side,
        "server_side": server_side,
        "license_id": license_id,
        "project_type": project_type,
        "is_draft": true,
    });
    let form = Form::new().text("data", data.to_string());
    let res = http
        .post(format!("{MODRINTH_API}/project"))
        .header("Authorization", &token)
        .multipart(form)
        .send()
        .await?;
    let res = require_modrinth_ok(pool, res, "Modrinth create project failed").await?;
    let created: serde_json::Value = res.json().await?;
    let remote_id = created
        .get("id")
        .and_then(|s| s.as_str())
        .ok_or_else(|| AppError::msg("Modrinth create project returned no id"))?
        .to_string();
    let remote_slug = created
        .get("slug")
        .and_then(|s| s.as_str())
        .unwrap_or(&slug)
        .to_string();

    update_project(
        pool,
        &draft.id,
        AuthorProjectPatch {
            title: None,
            slug: Some(remote_slug),
            summary: None,
            description: None,
            project_type: None,
            status: Some("checklist".into()),
            modrinth_id: Some(remote_id),
            local_path: None,
        },
    )
    .await
}

pub async fn publish_version(
    pool: &SqlitePool,
    http: &reqwest::Client,
    req: PublishVersionRequest,
) -> AppResult<PublishVersionResult> {
    let path = PathBuf::from(&req.file_path);
    if !path.is_file() {
        return Err(AppError::msg("Publish file not found"));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "jar" | "zip" | "mrpack" | "litemod") {
        return Err(AppError::msg(
            "File must be .jar, .zip, .mrpack, or .litemod",
        ));
    }
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::msg("Invalid file name"))?
        .to_string();
    let bytes = std::fs::read(&path)?;
    if bytes.is_empty() {
        return Err(AppError::msg("Publish file is empty"));
    }

    let version_number = req.version_number.trim().to_string();
    let name = if req.name.trim().is_empty() {
        version_number.clone()
    } else {
        req.name.trim().to_string()
    };
    let version_type = req.version_type.unwrap_or_else(|| "release".into());
    let loaders = if req.loaders.is_empty() {
        vec!["minecraft".into()]
    } else {
        req.loaders
    };
    let game_versions = req.game_versions;
    if game_versions.is_empty() {
        return Err(AppError::msg("At least one game version is required"));
    }

    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        if let Some(draft_id) = &req.draft_id {
            let _ = update_project(
                pool,
                draft_id,
                AuthorProjectPatch {
                    title: None,
                    slug: None,
                    summary: None,
                    description: None,
                    project_type: None,
                    status: Some("published".into()),
                    modrinth_id: Some(req.project_id.clone()),
                    local_path: Some(req.file_path.clone()),
                },
            )
            .await;
        }
        return Ok(PublishVersionResult {
            version_id: "dry-ver".into(),
            project_id: req.project_id.clone(),
            version_number: version_number.clone(),
            project_url: format!("https://modrinth.com/mod/{}", req.project_id),
        });
    }

    let token = access_token(pool).await?;
    let data = serde_json::json!({
        "name": name,
        "version_number": version_number,
        "changelog": req.changelog.unwrap_or_default(),
        "dependencies": [],
        "game_versions": game_versions,
        "version_type": version_type,
        "loaders": loaders,
        "featured": req.featured.unwrap_or(true),
        "status": "listed",
        "project_id": req.project_id,
        "file_parts": ["file"],
        "primary_file": "file",
    });
    let part = Part::bytes(bytes)
        .file_name(filename)
        .mime_str("application/octet-stream")
        .map_err(|e| AppError::msg(e.to_string()))?;
    let form = Form::new()
        .text("data", data.to_string())
        .part("file", part);

    let res = http
        .post(format!("{MODRINTH_API}/version"))
        .header("Authorization", &token)
        .multipart(form)
        .send()
        .await?;
    let res = require_modrinth_ok(pool, res, "Modrinth version create failed").await?;
    let created: serde_json::Value = res.json().await?;
    let version_id = created
        .get("id")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    let project_id = created
        .get("project_id")
        .and_then(|s| s.as_str())
        .unwrap_or(&req.project_id)
        .to_string();
    let project_type = created
        .get("project_type")
        .and_then(|s| s.as_str())
        .unwrap_or("mod");
    let slug = if let Some(draft_id) = &req.draft_id {
        let draft = get_project(pool, draft_id).await.ok();
        draft
            .and_then(|d| d.slug)
            .unwrap_or_else(|| project_id.clone())
    } else {
        project_id.clone()
    };

    if let Some(draft_id) = &req.draft_id {
        let _ = update_project(
            pool,
            draft_id,
            AuthorProjectPatch {
                title: None,
                slug: None,
                summary: None,
                description: None,
                project_type: None,
                status: Some("published".into()),
                modrinth_id: Some(project_id.clone()),
                local_path: Some(path_display(&path)),
            },
        )
        .await;
    }

    Ok(PublishVersionResult {
        version_id,
        project_id: project_id.clone(),
        version_number,
        project_url: format!("https://modrinth.com/{project_type}/{slug}"),
    })
}

fn path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn image_ext(path: &Path) -> AppResult<&'static str> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpg"),
        "webp" => Ok("webp"),
        "gif" => Ok("gif"),
        "bmp" => Ok("bmp"),
        "svg" => Ok("svg"),
        _ => Err(AppError::msg(
            "Image must be png, jpg, jpeg, webp, gif, bmp, or svg",
        )),
    }
}

fn read_image_bytes(path: &Path, max_bytes: u64) -> AppResult<Vec<u8>> {
    if !path.is_file() {
        return Err(AppError::msg("Image file not found"));
    }
    let meta = std::fs::metadata(path)?;
    if meta.len() > max_bytes {
        return Err(AppError::msg(format!(
            "Image too large (max {} KiB)",
            max_bytes / 1024
        )));
    }
    let bytes = std::fs::read(path)?;
    if bytes.is_empty() {
        return Err(AppError::msg("Image file is empty"));
    }
    Ok(bytes)
}

pub fn pick_publish_file() -> AppResult<Option<String>> {
    let path = rfd::FileDialog::new()
        .add_filter(
            "Modrinth publishables",
            &["jar", "zip", "mrpack", "litemod"],
        )
        .pick_file();
    Ok(path.map(|p| p.to_string_lossy().into_owned()))
}

pub fn pick_image_file() -> AppResult<Option<String>> {
    let path = rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"])
        .pick_file();
    Ok(path.map(|p| p.to_string_lossy().into_owned()))
}

/// PATCH /project/{id}/icon?ext=… with raw image body (max 256 KiB).
pub async fn upload_icon(
    pool: &SqlitePool,
    http: &reqwest::Client,
    req: UploadAuthorMediaRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let path = PathBuf::from(&req.file_path);
    let ext = image_ext(&path)?;
    let bytes = read_image_bytes(&path, 256 * 1024)?;
    let project_id = req.project_id.trim().to_string();
    if project_id.is_empty() {
        return Err(AppError::msg("Modrinth project id is required"));
    }

    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        return Ok(UploadAuthorMediaResult {
            project_id,
            kind: "icon".into(),
            note: "Dry-run: icon upload skipped.".into(),
        });
    }

    let token = access_token(pool).await?;
    let url = format!(
        "{MODRINTH_API}/project/{}/icon?ext={}",
        urlencoding::encode(&project_id),
        ext
    );
    let res = http
        .patch(&url)
        .header("Authorization", &token)
        .header("Content-Type", format!("image/{ext}"))
        .body(bytes)
        .send()
        .await?;
    let _ = require_modrinth_ok(pool, res, "Modrinth icon upload failed").await?;
    Ok(UploadAuthorMediaResult {
        project_id,
        kind: "icon".into(),
        note: "Icon uploaded to Modrinth.".into(),
    })
}

/// POST /project/{id}/gallery?ext=&featured= with raw image body (max 5 MiB).
pub async fn upload_gallery(
    pool: &SqlitePool,
    http: &reqwest::Client,
    req: UploadAuthorMediaRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let path = PathBuf::from(&req.file_path);
    let ext = image_ext(&path)?;
    let bytes = read_image_bytes(&path, 5 * 1024 * 1024)?;
    let project_id = req.project_id.trim().to_string();
    if project_id.is_empty() {
        return Err(AppError::msg("Modrinth project id is required"));
    }

    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        return Ok(UploadAuthorMediaResult {
            project_id,
            kind: "gallery".into(),
            note: "Dry-run: gallery upload skipped.".into(),
        });
    }

    let token = access_token(pool).await?;
    let featured = req.featured.unwrap_or(false);
    let mut url = format!(
        "{MODRINTH_API}/project/{}/gallery?ext={}&featured={}",
        urlencoding::encode(&project_id),
        ext,
        featured
    );
    if let Some(title) = req.title.filter(|s| !s.trim().is_empty()) {
        url.push_str(&format!("&title={}", urlencoding::encode(title.trim())));
    }
    if let Some(description) = req.description.filter(|s| !s.trim().is_empty()) {
        url.push_str(&format!(
            "&description={}",
            urlencoding::encode(description.trim())
        ));
    }
    let res = http
        .post(&url)
        .header("Authorization", &token)
        .header("Content-Type", format!("image/{ext}"))
        .body(bytes)
        .send()
        .await?;
    let _ = require_modrinth_ok(pool, res, "Modrinth gallery upload failed").await?;
    Ok(UploadAuthorMediaResult {
        project_id,
        kind: "gallery".into(),
        note: "Gallery image uploaded to Modrinth.".into(),
    })
}

pub async fn list_gallery(
    pool: &SqlitePool,
    http: &reqwest::Client,
    project_id: &str,
) -> AppResult<Vec<RemoteGalleryImage>> {
    let project_id = project_id.trim();
    if project_id.is_empty() {
        return Err(AppError::msg("Modrinth project id is required"));
    }
    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        return Ok(vec![RemoteGalleryImage {
            url: "https://cdn.modrinth.com/data/dry/images/dry-gallery.png".into(),
            title: Some("Dry-run gallery".into()),
            description: None,
            featured: true,
            ordering: Some(0),
        }]);
    }
    let token = access_token(pool).await?;
    let res = http
        .get(format!(
            "{MODRINTH_API}/project/{}",
            urlencoding::encode(project_id)
        ))
        .header("Authorization", &token)
        .send()
        .await?;
    let res = require_modrinth_ok(pool, res, "Failed to load project gallery").await?;
    let v: serde_json::Value = res.json().await?;
    Ok(v.get("gallery")
        .and_then(|g| g.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|img| {
            Some(RemoteGalleryImage {
                url: img.get("url")?.as_str()?.to_string(),
                title: img
                    .get("title")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                description: img
                    .get("description")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                featured: img
                    .get("featured")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
                ordering: img.get("ordering").and_then(|n| n.as_i64()),
            })
        })
        .collect())
}

pub async fn set_gallery_featured(
    pool: &SqlitePool,
    http: &reqwest::Client,
    req: GalleryImageEditRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let project_id = req.project_id.trim().to_string();
    let url = req.url.trim().to_string();
    if project_id.is_empty() || url.is_empty() {
        return Err(AppError::msg("project id and gallery image url are required"));
    }
    let featured = req.featured.unwrap_or(true);
    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        return Ok(UploadAuthorMediaResult {
            project_id,
            kind: "gallery".into(),
            note: format!("Dry-run: set featured={featured}."),
        });
    }
    let token = access_token(pool).await?;
    let endpoint = format!(
        "{MODRINTH_API}/project/{}/gallery?url={}&featured={}",
        urlencoding::encode(&project_id),
        urlencoding::encode(&url),
        featured
    );
    let res = http
        .patch(&endpoint)
        .header("Authorization", &token)
        .send()
        .await?;
    let _ = require_modrinth_ok(pool, res, "Modrinth gallery update failed").await?;
    Ok(UploadAuthorMediaResult {
        project_id,
        kind: "gallery".into(),
        note: if featured {
            "Gallery image set as featured.".into()
        } else {
            "Gallery image unfeatured.".into()
        },
    })
}

pub async fn delete_gallery_image(
    pool: &SqlitePool,
    http: &reqwest::Client,
    req: GalleryImageEditRequest,
) -> AppResult<UploadAuthorMediaResult> {
    let project_id = req.project_id.trim().to_string();
    let url = req.url.trim().to_string();
    if project_id.is_empty() || url.is_empty() {
        return Err(AppError::msg("project id and gallery image url are required"));
    }
    let cfg = ModrinthOauthConfig::from_env();
    let kind = settings::get(pool, SETTING_KIND).await?.unwrap_or_default();
    if cfg.dry_run || kind == "dry-run" {
        return Ok(UploadAuthorMediaResult {
            project_id,
            kind: "gallery".into(),
            note: "Dry-run: gallery image delete skipped.".into(),
        });
    }
    let token = access_token(pool).await?;
    let endpoint = format!(
        "{MODRINTH_API}/project/{}/gallery?url={}",
        urlencoding::encode(&project_id),
        urlencoding::encode(&url)
    );
    let res = http
        .delete(&endpoint)
        .header("Authorization", &token)
        .send()
        .await?;
    let _ = require_modrinth_ok(pool, res, "Modrinth gallery delete failed").await?;
    Ok(UploadAuthorMediaResult {
        project_id,
        kind: "gallery".into(),
        note: "Gallery image deleted.".into(),
    })
}

pub async fn list_projects(pool: &SqlitePool) -> AppResult<Vec<AuthorProject>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id, title, slug, summary, description, project_type, status,
                modrinth_id, local_path, created_at, updated_at
         FROM author_projects
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| AuthorProject {
            id: r.0,
            title: r.1,
            slug: r.2,
            summary: r.3,
            description: r.4,
            project_type: r.5,
            status: r.6,
            modrinth_id: r.7.filter(|s| !s.is_empty()),
            local_path: r.8.filter(|s| !s.is_empty()),
            created_at: r.9,
            updated_at: r.10,
        })
        .collect())
}

pub async fn get_project(pool: &SqlitePool, id: &str) -> AppResult<AuthorProject> {
    list_projects(pool)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::msg("Author project not found"))
}

pub async fn create_project(pool: &SqlitePool, new: NewAuthorProject) -> AppResult<AuthorProject> {
    let title = new.title.trim();
    if title.is_empty() {
        return Err(AppError::msg("Title is required"));
    }
    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    let project_type = new
        .project_type
        .unwrap_or_else(|| "mod".into())
        .trim()
        .to_string();
    let slug = new
        .slug
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let summary = new.summary.unwrap_or_default();
    let description = new.description.unwrap_or_default();
    sqlx::query(
        "INSERT INTO author_projects
         (id, title, slug, summary, description, project_type, status, modrinth_id, local_path, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 'draft', NULL, '', ?, ?)",
    )
    .bind(&id)
    .bind(title)
    .bind(&slug)
    .bind(&summary)
    .bind(&description)
    .bind(&project_type)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    get_project(pool, &id).await
}

pub async fn update_project(
    pool: &SqlitePool,
    id: &str,
    patch: AuthorProjectPatch,
) -> AppResult<AuthorProject> {
    let mut current = get_project(pool, id).await?;
    if let Some(t) = patch.title {
        let t = t.trim().to_string();
        if t.is_empty() {
            return Err(AppError::msg("Title is required"));
        }
        current.title = t;
    }
    if let Some(s) = patch.slug {
        current.slug = Some(s).filter(|s| !s.trim().is_empty());
    }
    if let Some(s) = patch.summary {
        current.summary = s;
    }
    if let Some(d) = patch.description {
        current.description = d;
    }
    if let Some(t) = patch.project_type {
        current.project_type = t;
    }
    if let Some(s) = patch.status {
        if matches!(s.as_str(), "draft" | "checklist" | "published") {
            current.status = s;
        }
    }
    if let Some(m) = patch.modrinth_id {
        current.modrinth_id = Some(m).filter(|s| !s.is_empty());
    }
    if let Some(p) = patch.local_path {
        current.local_path = Some(p).filter(|s| !s.is_empty());
    }
    current.updated_at = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE author_projects SET title=?, slug=?, summary=?, description=?, project_type=?,
         status=?, modrinth_id=?, local_path=?, updated_at=? WHERE id=?",
    )
    .bind(&current.title)
    .bind(&current.slug)
    .bind(&current.summary)
    .bind(&current.description)
    .bind(&current.project_type)
    .bind(&current.status)
    .bind(&current.modrinth_id)
    .bind(&current.local_path)
    .bind(&current.updated_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(current)
}

pub async fn delete_project(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let res = sqlx::query("DELETE FROM author_projects WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::msg("Author project not found"));
    }
    Ok(())
}

pub async fn publish_checklist(
    pool: &SqlitePool,
    project: &AuthorProject,
) -> Vec<PublishChecklistItem> {
    let st = status(pool).await.ok();
    let connected = st.as_ref().map(|s| s.connected).unwrap_or(false);
    vec![
        PublishChecklistItem {
            id: "title".into(),
            label: "Project title set".into(),
            done: !project.title.trim().is_empty(),
        },
        PublishChecklistItem {
            id: "summary".into(),
            label: "Short summary (Modrinth description)".into(),
            done: project.summary.trim().len() >= 16,
        },
        PublishChecklistItem {
            id: "body".into(),
            label: "Long description drafted".into(),
            done: project.description.trim().len() >= 40,
        },
        PublishChecklistItem {
            id: "slug".into(),
            label: "URL slug chosen".into(),
            done: project
                .slug
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
        },
        PublishChecklistItem {
            id: "linked".into(),
            label: "Linked to a Modrinth project".into(),
            done: project
                .modrinth_id
                .as_ref()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
        },
        PublishChecklistItem {
            id: "oauth".into(),
            label: "Modrinth account connected".into(),
            done: connected,
        },
    ]
}
