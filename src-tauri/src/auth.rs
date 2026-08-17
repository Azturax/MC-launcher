//! Microsoft / Xbox / Minecraft token chain.
//! Refresh material never leaves Rust. The renderer only sees a session
//! descriptor (uuid, name, skin URL, expiry).

use crate::error::{AppError, AppResult};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration as StdDuration, Instant};
use uuid::Uuid;

const KEYRING_SERVICE: &str = "dev.aureum.launcher";

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub client_id: Option<String>,
    pub tenant: String,
    pub redirect_port: u16,
    pub dry_run: bool,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let client_id = std::env::var("AUREUM_MS_CLIENT_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let tenant =
            std::env::var("AUREUM_MS_TENANT").unwrap_or_else(|_| "consumers".into());
        let redirect_port = std::env::var("AUREUM_OAUTH_REDIRECT_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(17890);
        let forced_dry = std::env::var("AUREUM_AUTH_DRY_RUN")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            dry_run: forced_dry || client_id.is_none(),
            client_id,
            tenant,
            redirect_port,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub uuid: Option<String>,
    pub skin_url: Option<String>,
    pub expires_at: Option<String>,
    pub has_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub dry_run: bool,
    pub has_client_id: bool,
    pub tenant: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDescriptor {
    pub profile_id: String,
    pub uuid: String,
    pub name: String,
    pub skin_url: Option<String>,
    pub access_token: Option<String>,
    pub user_type: String,
    pub offline: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McProfile {
    id: String,
    name: String,
    skins: Option<Vec<McSkin>>,
}

#[derive(Debug, Deserialize)]
struct McSkin {
    url: Option<String>,
    state: Option<String>,
}

impl AuthStatus {
    pub fn from_config(cfg: &AuthConfig) -> Self {
        Self {
            dry_run: cfg.dry_run,
            has_client_id: cfg.client_id.is_some(),
            tenant: cfg.tenant.clone(),
            redirect_uri: format!("http://127.0.0.1:{}/auth/callback", cfg.redirect_port),
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub(crate) fn store_secret(secret_ref: &str, value: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, secret_ref)?;
    entry.set_password(value)?;
    Ok(())
}

pub(crate) fn read_secret(secret_ref: &str) -> AppResult<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, secret_ref)?;
    Ok(entry.get_password()?)
}

pub(crate) fn delete_secret(secret_ref: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, secret_ref)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub async fn list_profiles(pool: &SqlitePool) -> AppResult<Vec<Profile>> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        "SELECT id, kind, display_name, uuid, skin_url, secret_ref, expires_at, created_at, updated_at
         FROM profiles ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Profile {
            id: r.0,
            kind: r.1,
            display_name: r.2,
            uuid: r.3,
            skin_url: r.4,
            expires_at: r.6,
            has_secret: r.5.is_some(),
            created_at: r.7,
            updated_at: r.8,
        })
        .collect())
}

pub async fn get_profile(pool: &SqlitePool, id: &str) -> AppResult<Profile> {
    list_profiles(pool)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::msg("Profile not found"))
}

pub async fn create_offline_profile(pool: &SqlitePool, display_name: &str) -> AppResult<Profile> {
    let name = display_name.trim();
    if name.is_empty() || name.len() > 16 {
        return Err(AppError::msg(
            "Offline profile name must be 1–16 characters",
        ));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::msg(
            "Offline names may only use letters, numbers, and underscore",
        ));
    }

    let id = Uuid::new_v4().to_string();
    // Random local id — not the OfflinePlayer: UUID used by cracked servers.
    let uuid = Uuid::new_v4().to_string();
    let ts = now();

    sqlx::query(
        "INSERT INTO profiles (id, kind, display_name, uuid, skin_url, secret_ref, expires_at, created_at, updated_at)
         VALUES (?, 'offline', ?, ?, NULL, NULL, NULL, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&uuid)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;

    get_profile(pool, &id).await
}

pub async fn delete_profile(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let row = sqlx::query_as::<_, (Option<String>,)>("SELECT secret_ref FROM profiles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::msg("Profile not found"))?;
    if let Some(secret_ref) = row.0 {
        let _ = delete_secret(&secret_ref);
    }
    sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn microsoft_login(
    pool: &SqlitePool,
    http: &reqwest::Client,
    cfg: &AuthConfig,
) -> AppResult<Profile> {
    if cfg.dry_run {
        return create_dry_run_profile(pool).await;
    }
    let client_id = cfg
        .client_id
        .as_deref()
        .ok_or_else(|| AppError::msg("Missing AUREUM_MS_CLIENT_ID"))?;

    let (verifier, challenge) = pkce_pair();
    let redirect = format!("http://127.0.0.1:{}/auth/callback", cfg.redirect_port);
    let state = Uuid::new_v4().to_string();
    let auth_url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize?client_id={}&response_type=code&redirect_uri={}&response_mode=query&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        urlencoding::encode(&cfg.tenant),
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode("XboxLive.signin offline_access"),
        urlencoding::encode(&state),
        urlencoding::encode(&challenge),
    );

    open::that(&auth_url).map_err(|e| AppError::msg(format!("Could not open browser: {e}")))?;
    let port = cfg.redirect_port;
    let expected = state.clone();
    let code = tokio::task::spawn_blocking(move || wait_for_code(port, &expected))
        .await
        .map_err(|e| AppError::msg(e.to_string()))??;
    let tokens = exchange_code(http, client_id, &cfg.tenant, &redirect, &code, &verifier).await?;
    let xbox = xbox_auth(http, &tokens.access_token).await?;
    let xsts = xsts_auth(http, &xbox).await?;
    let mc_token = minecraft_login(http, &xsts.0, &xsts.1).await?;
    let profile = minecraft_profile(http, &mc_token).await?;

    persist_microsoft_profile(pool, &profile, tokens.refresh_token.as_deref()).await
}

async fn create_dry_run_profile(pool: &SqlitePool) -> AppResult<Profile> {
    let id = Uuid::new_v4().to_string();
    let uuid = Uuid::new_v4().to_string();
    let ts = now();
    let name = "Dev Player";
    sqlx::query(
        "INSERT INTO profiles (id, kind, display_name, uuid, skin_url, secret_ref, expires_at, created_at, updated_at)
         VALUES (?, 'microsoft-dry-run', ?, ?, NULL, NULL, NULL, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(&uuid)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    get_profile(pool, &id).await
}

async fn persist_microsoft_profile(
    pool: &SqlitePool,
    mc: &McProfile,
    refresh: Option<&str>,
) -> AppResult<Profile> {
    let id = Uuid::new_v4().to_string();
    let secret_ref = format!("ms-refresh-{id}");
    if let Some(refresh) = refresh {
        store_secret(&secret_ref, refresh)?;
    }
    let skin = mc.skins.as_ref().and_then(|skins| {
        skins
            .iter()
            .find(|s| s.state.as_deref() == Some("ACTIVE"))
            .and_then(|s| s.url.clone())
    });
    let expires = (Utc::now() + Duration::hours(24)).to_rfc3339();
    let ts = now();
    let uuid = normalize_uuid(&mc.id);

    sqlx::query(
        "INSERT INTO profiles (id, kind, display_name, uuid, skin_url, secret_ref, expires_at, created_at, updated_at)
         VALUES (?, 'microsoft', ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&mc.name)
    .bind(&uuid)
    .bind(&skin)
    .bind(if refresh.is_some() {
        Some(secret_ref)
    } else {
        None
    })
    .bind(&expires)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;

    get_profile(pool, &id).await
}

fn normalize_uuid(raw: &str) -> String {
    if raw.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &raw[0..8],
            &raw[8..12],
            &raw[12..16],
            &raw[16..20],
            &raw[20..32]
        )
    } else {
        raw.to_string()
    }
}

pub(crate) fn wait_for_code(port: u16, expected_state: &str) -> AppResult<String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| AppError::msg(format!("OAuth callback bind failed: {e}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| AppError::msg(e.to_string()))?;
    let deadline = Instant::now() + StdDuration::from_secs(180);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(pair) => break pair,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(AppError::msg("Sign-in timed out or was cancelled"));
                }
                thread::sleep(StdDuration::from_millis(100));
            }
            Err(_) => return Err(AppError::msg("Sign-in timed out or was cancelled")),
        }
    };
    let _ = stream.set_read_timeout(Some(StdDuration::from_secs(10)));
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let req = String::from_utf8_lossy(&buf[..n]);
    let first = req.lines().next().unwrap_or("");
    let path = first.split_whitespace().nth(1).unwrap_or("");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let params: std::collections::HashMap<String, String> = query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((
                urlencoding::decode(k).ok()?.into_owned(),
                urlencoding::decode(v).ok()?.into_owned(),
            ))
        })
        .collect();

    let state_ok = params.get("state").map(String::as_str) == Some(expected_state);
    let body = if !state_ok {
        "State mismatch. You can close this tab."
    } else if params.contains_key("code") {
        "Aureum received your Microsoft sign-in. You can close this tab."
    } else {
        "Sign-in failed. You can close this tab."
    };
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html><body style=\"font-family:sans-serif;padding:2rem\">{body}</body></html>"
    );
    let _ = stream.write_all(resp.as_bytes());

    if !state_ok {
        return Err(AppError::msg("OAuth state mismatch"));
    }
    if let Some(err) = params.get("error") {
        return Err(AppError::msg(format!("OAuth sign-in error: {err}")));
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| AppError::msg("No authorization code returned"))
}

async fn exchange_code(
    http: &reqwest::Client,
    client_id: &str,
    tenant: &str,
    redirect: &str,
    code: &str,
    verifier: &str,
) -> AppResult<TokenResponse> {
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let form = [
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect),
        ("code_verifier", verifier),
    ];
    let resp = http.post(url).form(&form).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::msg(format!(
            "Token exchange failed: {}",
            resp.text().await.unwrap_or_default()
        )));
    }
    Ok(resp.json().await?)
}

async fn xbox_auth(http: &reqwest::Client, ms_token: &str) -> AppResult<String> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let resp = http
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::msg(
            "Xbox Live auth failed. The Azure app may lack Xbox Live access.",
        ));
    }
    let v: serde_json::Value = resp.json().await?;
    v["Token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::msg("Xbox token missing"))
}

async fn xsts_auth(http: &reqwest::Client, xbox_token: &str) -> AppResult<(String, String)> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbox_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let resp = http
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::msg("XSTS authorize failed"));
    }
    let v: serde_json::Value = resp.json().await?;
    let token = v["Token"]
        .as_str()
        .ok_or_else(|| AppError::msg("XSTS token missing"))?
        .to_string();
    let uhs = v["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| AppError::msg("XSTS uhs missing"))?
        .to_string();
    Ok((token, uhs))
}

async fn minecraft_login(http: &reqwest::Client, xsts: &str, uhs: &str) -> AppResult<String> {
    let body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={uhs};{xsts}")
    });
    let resp = http
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::msg("Minecraft Services login failed"));
    }
    let v: serde_json::Value = resp.json().await?;
    v["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::msg("Minecraft access token missing"))
}

async fn minecraft_profile(http: &reqwest::Client, mc_token: &str) -> AppResult<McProfile> {
    let resp = http
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::msg(
            "No Minecraft Java profile on this Microsoft account",
        ));
    }
    Ok(resp.json().await?)
}

/// Build a launch session. Refresh tokens stay in the keychain.
/// Offline and dry-run profiles never receive a session token.
pub async fn session_for_launch(
    pool: &SqlitePool,
    http: &reqwest::Client,
    cfg: &AuthConfig,
    profile_id: &str,
) -> AppResult<SessionDescriptor> {
    let profile = get_profile(pool, profile_id).await?;
    let uuid = profile
        .uuid
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if profile.kind == "offline" || profile.kind == "microsoft-dry-run" || cfg.dry_run {
        return Ok(SessionDescriptor {
            profile_id: profile.id,
            uuid,
            name: profile.display_name,
            skin_url: profile.skin_url,
            access_token: None,
            user_type: "legacy".into(),
            offline: true,
        });
    }

    let secret_ref = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT secret_ref FROM profiles WHERE id = ?",
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await?
    .0
    .ok_or_else(|| AppError::msg("No refresh token in keychain for this profile"))?;

    let refresh = read_secret(&secret_ref)?;
    let client_id = cfg
        .client_id
        .as_deref()
        .ok_or_else(|| AppError::msg("Missing AUREUM_MS_CLIENT_ID"))?;
    let ms = refresh_ms_token(http, client_id, &cfg.tenant, &refresh).await?;
    if let Some(new_refresh) = ms.refresh_token.as_deref() {
        store_secret(&secret_ref, new_refresh)?;
    }
    let xbox = xbox_auth(http, &ms.access_token).await?;
    let xsts = xsts_auth(http, &xbox).await?;
    let mc_token = minecraft_login(http, &xsts.0, &xsts.1).await?;
    let mc = minecraft_profile(http, &mc_token).await?;

    Ok(SessionDescriptor {
        profile_id: profile.id,
        uuid: normalize_uuid(&mc.id),
        name: mc.name,
        skin_url: profile.skin_url,
        access_token: Some(mc_token),
        user_type: "msa".into(),
        offline: false,
    })
}

async fn refresh_ms_token(
    http: &reqwest::Client,
    client_id: &str,
    tenant: &str,
    refresh: &str,
) -> AppResult<TokenResponse> {
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let form = [
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh),
        ("scope", "XboxLive.signin offline_access"),
    ];
    let resp = http.post(url).form(&form).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::msg("Microsoft token refresh failed"));
    }
    Ok(resp.json().await?)
}
