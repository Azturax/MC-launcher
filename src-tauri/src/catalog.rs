//! Catalog adapter contract. Modrinth is the v1 source. UI never branches
//! on source except for attribution chips. CurseForge stays hidden until
//! a licensed API key exists — this module does not scrape.

use crate::error::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

pub const MODRINTH_API: &str = "https://api.modrinth.com/v2";
pub const SOURCE_MODRINTH: &str = "modrinth";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHit {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub source: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub project_type: String,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCategory {
    pub name: String,
    pub project_type: String,
    #[serde(default)]
    pub header: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonationLink {
    pub id: String,
    pub platform: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMember {
    pub user_id: String,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: Option<String>,
    pub source: String,
    pub icon_url: Option<String>,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub license: Option<String>,
    pub project_url: String,
    #[serde(default = "default_project_type")]
    pub project_type: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    #[serde(default)]
    pub members: Vec<ProjectMember>,
    pub downloads: u64,
    pub followers: u64,
    pub published: Option<String>,
    pub updated: Option<String>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    #[serde(default)]
    pub donation_urls: Vec<DonationLink>,
}

fn default_project_type() -> String {
    "mod".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub channel: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub featured: bool,
    pub files: Vec<CatalogFile>,
    pub dependencies: Vec<CatalogDependency>,
    pub date_published: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub hits: Vec<T>,
    pub offset: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    pub query: Option<String>,
    pub source: Option<String>,
    pub loaders: Option<Vec<String>>,
    pub game_versions: Option<Vec<String>>,
    pub project_types: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    /// Modrinth index: relevance, downloads, follows, newest, updated
    pub index: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
    /// `stable` (release), `beta` (release+beta), `all`
    #[allow(dead_code)]
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvider {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub reason: Option<String>,
}

pub fn providers() -> Vec<CatalogProvider> {
    vec![
        CatalogProvider {
            id: SOURCE_MODRINTH.into(),
            label: "Modrinth".into(),
            enabled: true,
            reason: None,
        },
        CatalogProvider {
            id: "curseforge".into(),
            label: "CurseForge".into(),
            enabled: false,
            reason: Some("Hidden until a licensed API key is configured. Aureum will not scrape.".into()),
        },
    ]
}

/// Every catalog source implements this shape. Methods are async on the
/// concrete adapter because they talk to the network.
#[allow(dead_code)]
pub trait CatalogAdapter {
    fn id(&self) -> &'static str;
}

pub struct ModrinthAdapter;

impl CatalogAdapter for ModrinthAdapter {
    fn id(&self) -> &'static str {
        SOURCE_MODRINTH
    }
}

impl ModrinthAdapter {
    pub async fn search(
        &self,
        http: &reqwest::Client,
        pool: &SqlitePool,
        cache_dir: &Path,
        filters: &SearchFilters,
    ) -> AppResult<Page<ProjectHit>> {
        if let Some(source) = filters.source.as_deref() {
            if source != SOURCE_MODRINTH && source != "all" {
                return Err(AppError::msg(
                    "That catalog source is not enabled. CurseForge stays hidden until a licensed API key exists.",
                ));
            }
        }
        let offset = filters.offset.unwrap_or(0);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let index = match filters.index.as_deref() {
            Some("downloads") | Some("follows") | Some("newest") | Some("updated") => {
                filters.index.as_deref().unwrap()
            }
            _ => "relevance",
        };
        let mut facets: Vec<Vec<String>> = Vec::new();
        let types: Vec<String> = filters
            .project_types
            .as_ref()
            .map(|t| {
                t.iter()
                    .filter(|s| !s.is_empty() && *s != "all")
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if types.is_empty() {
            facets.push(vec!["project_type:mod".into()]);
        } else {
            facets.push(types.iter().map(|t| format!("project_type:{t}")).collect());
        }
        if let Some(loaders) = &filters.loaders {
            let row: Vec<String> = loaders
                .iter()
                .filter(|l| !l.is_empty() && *l != "vanilla" && *l != "all")
                .map(|l| format!("categories:{l}"))
                .collect();
            if !row.is_empty() {
                facets.push(row);
            }
        }
        if let Some(versions) = &filters.game_versions {
            let row: Vec<String> = versions
                .iter()
                .filter(|v| !v.is_empty() && *v != "all")
                .map(|v| format!("versions:{v}"))
                .collect();
            if !row.is_empty() {
                facets.push(row);
            }
        }
        if let Some(categories) = &filters.categories {
            let row: Vec<String> = categories
                .iter()
                .filter(|c| !c.is_empty() && *c != "all")
                .map(|c| format!("categories:{c}"))
                .collect();
            if !row.is_empty() {
                facets.push(row);
            }
        }
        let facets_json = serde_json::to_string(&facets)?;
        let query = filters.query.clone().unwrap_or_default();
        let url = format!(
            "{MODRINTH_API}/search?query={}&limit={limit}&offset={offset}&index={index}&facets={}",
            urlencoding::encode(&query),
            urlencoding::encode(&facets_json),
        );
        let v: serde_json::Value = cached_json(http, pool, cache_dir, &url).await?;
        let hits = v
            .get("hits")
            .and_then(|h| h.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(hit_from_json)
            .collect();
        Ok(Page {
            hits,
            offset,
            total: v.get("total_hits").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        })
    }

    /// Official Modrinth tag list: https://docs.modrinth.com/api/operations/categorylist/
    pub async fn categories(
        &self,
        http: &reqwest::Client,
        pool: &SqlitePool,
        cache_dir: &Path,
    ) -> AppResult<Vec<CatalogCategory>> {
        let url = format!("{MODRINTH_API}/tag/category");
        let v: serde_json::Value = cached_json(http, pool, cache_dir, &url).await?;
        let list = v
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let name = item.get("name")?.as_str()?.to_string();
                let project_type = item
                    .get("project_type")
                    .and_then(|s| s.as_str())
                    .unwrap_or("mod")
                    .to_string();
                // Skip loader-like tags that appear as categories for some types.
                if matches!(
                    name.as_str(),
                    "fabric" | "forge" | "neoforge" | "quilt" | "liteloader" | "rift" | "datapack"
                ) {
                    return None;
                }
                Some(CatalogCategory {
                    name,
                    project_type,
                    header: item
                        .get("header")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect();
        Ok(list)
    }

    pub async fn project(
        &self,
        http: &reqwest::Client,
        pool: &SqlitePool,
        cache_dir: &Path,
        id_or_slug: &str,
    ) -> AppResult<ProjectDetail> {
        let url = format!(
            "{MODRINTH_API}/project/{}",
            urlencoding::encode(id_or_slug)
        );
        let v: serde_json::Value = cached_json(http, pool, cache_dir, &url).await?;
        let id = v
            .get("id")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::msg("Modrinth project missing id"))?
            .to_string();
        let slug = v
            .get("slug")
            .and_then(|s| s.as_str())
            .unwrap_or(&id)
            .to_string();
        let project_type = v
            .get("project_type")
            .and_then(|s| s.as_str())
            .unwrap_or("mod")
            .to_string();
        let gallery = v
            .get("gallery")
            .and_then(|g| g.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|img| {
                Some(GalleryImage {
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
                })
            })
            .collect();
        let donation_urls = v
            .get("donation_urls")
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| {
                Some(DonationLink {
                    id: d.get("id")?.as_str()?.to_string(),
                    platform: d
                        .get("platform")
                        .and_then(|s| s.as_str())
                        .unwrap_or("Donate")
                        .to_string(),
                    url: d.get("url")?.as_str()?.to_string(),
                })
            })
            .collect();
        let members = self
            .project_members(http, pool, cache_dir, &id)
            .await
            .unwrap_or_default();
        Ok(ProjectDetail {
            id,
            slug: slug.clone(),
            title: v
                .get("title")
                .and_then(|s| s.as_str())
                .unwrap_or("Untitled")
                .to_string(),
            description: v
                .get("description")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string(),
            body: v.get("body").and_then(|s| s.as_str()).map(|s| s.to_string()),
            source: SOURCE_MODRINTH.into(),
            icon_url: v
                .get("icon_url")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            loaders: string_array(&v, "loaders"),
            game_versions: string_array(&v, "game_versions"),
            license: v
                .pointer("/license/id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            project_url: format!("https://modrinth.com/{project_type}/{slug}"),
            project_type,
            categories: string_array(&v, "categories"),
            gallery,
            members,
            downloads: v.get("downloads").and_then(|n| n.as_u64()).unwrap_or(0),
            followers: v.get("followers").and_then(|n| n.as_u64()).unwrap_or(0),
            published: v
                .get("published")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            updated: v
                .get("updated")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            source_url: v
                .get("source_url")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            issues_url: v
                .get("issues_url")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            wiki_url: v
                .get("wiki_url")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            discord_url: v
                .get("discord_url")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            donation_urls,
        })
    }

    pub async fn project_members(
        &self,
        http: &reqwest::Client,
        pool: &SqlitePool,
        cache_dir: &Path,
        id_or_slug: &str,
    ) -> AppResult<Vec<ProjectMember>> {
        let url = format!(
            "{MODRINTH_API}/project/{}/members",
            urlencoding::encode(id_or_slug)
        );
        let v: serde_json::Value = cached_json(http, pool, cache_dir, &url).await?;
        let list = v
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|m| {
                let user = m.get("user")?;
                Some(ProjectMember {
                    user_id: user.get("id")?.as_str()?.to_string(),
                    name: user
                        .get("username")
                        .or_else(|| user.get("name"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    role: m
                        .get("role")
                        .and_then(|s| s.as_str())
                        .unwrap_or("Member")
                        .to_string(),
                    avatar_url: user
                        .get("avatar_url")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect();
        Ok(list)
    }

    pub async fn versions(
        &self,
        http: &reqwest::Client,
        pool: &SqlitePool,
        cache_dir: &Path,
        id_or_slug: &str,
        loaders: &[String],
        game_versions: &[String],
        channel: Option<&str>,
    ) -> AppResult<Vec<CatalogVersion>> {
        let mut url = format!(
            "{MODRINTH_API}/project/{}/version",
            urlencoding::encode(id_or_slug)
        );
        let mut qs = Vec::new();
        if !loaders.is_empty() {
            qs.push(format!(
                "loaders={}",
                urlencoding::encode(&serde_json::to_string(loaders)?)
            ));
        }
        if !game_versions.is_empty() {
            qs.push(format!(
                "game_versions={}",
                urlencoding::encode(&serde_json::to_string(game_versions)?)
            ));
        }
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs.join("&"));
        }
        let v: serde_json::Value = cached_json(http, pool, cache_dir, &url).await?;
        let mut versions: Vec<CatalogVersion> = v
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(version_from_json)
            .collect();
        if let Some(ch) = channel {
            versions.retain(|ver| channel_allows(ch, &ver.channel));
        }
        Ok(versions)
    }

    pub async fn version(
        &self,
        http: &reqwest::Client,
        pool: &SqlitePool,
        cache_dir: &Path,
        version_id: &str,
    ) -> AppResult<CatalogVersion> {
        let url = format!(
            "{MODRINTH_API}/version/{}",
            urlencoding::encode(version_id)
        );
        let v: serde_json::Value = cached_json(http, pool, cache_dir, &url).await?;
        Ok(version_from_json(v))
    }
}

pub fn channel_allows(filter: &str, version_type: &str) -> bool {
    match filter {
        "stable" => version_type == "release",
        "beta" => version_type == "release" || version_type == "beta",
        _ => true,
    }
}

pub fn primary_file(version: &CatalogVersion) -> AppResult<&CatalogFile> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| AppError::msg("Version has no downloadable files"))
}

fn hit_from_json(v: serde_json::Value) -> ProjectHit {
    let categories_raw = string_array(&v, "categories");
    let display = string_array(&v, "display_categories");
    let loader_names = [
        "fabric",
        "forge",
        "neoforge",
        "quilt",
        "liteloader",
        "rift",
        "bukkit",
        "spigot",
        "paper",
        "purpur",
        "sponge",
        "bungeecord",
        "waterfall",
        "velocity",
    ];
    let mut loaders = categories_raw
        .iter()
        .chain(display.iter())
        .filter(|c| loader_names.contains(&c.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    loaders.sort();
    loaders.dedup();
    let categories: Vec<String> = categories_raw
        .into_iter()
        .filter(|c| !loader_names.contains(&c.as_str()))
        .collect();
    ProjectHit {
        id: v
            .get("project_id")
            .or_else(|| v.get("id"))
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string(),
        slug: v
            .get("slug")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string(),
        title: v
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or("Untitled")
            .to_string(),
        description: v
            .get("description")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string(),
        source: SOURCE_MODRINTH.into(),
        loaders,
        game_versions: string_array(&v, "versions"),
        icon_url: v
            .get("icon_url")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string()),
        downloads: v.get("downloads").and_then(|n| n.as_u64()).unwrap_or(0),
        project_type: v
            .get("project_type")
            .and_then(|s| s.as_str())
            .unwrap_or("mod")
            .to_string(),
        categories,
    }
}

fn version_from_json(v: serde_json::Value) -> CatalogVersion {
    let files = v
        .get("files")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|f| CatalogFile {
            url: f.get("url").and_then(|s| s.as_str()).unwrap_or("").into(),
            filename: f
                .get("filename")
                .and_then(|s| s.as_str())
                .unwrap_or("mod.jar")
                .into(),
            primary: f.get("primary").and_then(|b| b.as_bool()).unwrap_or(false),
            sha1: f
                .pointer("/hashes/sha1")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            sha512: f
                .pointer("/hashes/sha512")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            size: f.get("size").and_then(|n| n.as_u64()).unwrap_or(0),
        })
        .collect();
    let dependencies = v
        .get("dependencies")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|d| CatalogDependency {
            project_id: d
                .get("project_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            version_id: d
                .get("version_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            dependency_type: d
                .get("dependency_type")
                .and_then(|s| s.as_str())
                .unwrap_or("required")
                .to_string(),
        })
        .collect();
    CatalogVersion {
        id: v.get("id").and_then(|s| s.as_str()).unwrap_or("").into(),
        project_id: v
            .get("project_id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .into(),
        name: v.get("name").and_then(|s| s.as_str()).unwrap_or("").into(),
        version_number: v
            .get("version_number")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .into(),
        channel: v
            .get("version_type")
            .and_then(|s| s.as_str())
            .unwrap_or("release")
            .into(),
        loaders: string_array(&v, "loaders"),
        game_versions: string_array(&v, "game_versions"),
        featured: v.get("featured").and_then(|b| b.as_bool()).unwrap_or(false),
        files,
        dependencies,
        date_published: v
            .get("date_published")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string()),
    }
}

fn string_array(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

async fn cached_json(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    url: &str,
) -> AppResult<serde_json::Value> {
    let key = format!("modrinth:{}", url);
    let cached = settings_cache_get(pool, &key).await?;
    let mut req = http.get(url);
    if let Some((etag, _)) = &cached {
        if !etag.is_empty() {
            req = req.header(reqwest::header::IF_NONE_MATCH, etag.as_str());
        }
    }
    let resp = req.send().await?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        if let Some((_, path)) = cached {
            let bytes = std::fs::read(path)?;
            return Ok(serde_json::from_slice(&bytes)?);
        }
    }
    if !resp.status().is_success() {
        return Err(AppError::msg(format!(
            "Modrinth request failed ({}): {url}",
            resp.status()
        )));
    }
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp.bytes().await?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)?;
    let dir = cache_dir.join("modrinth");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(cache_name(url));
    std::fs::write(&file, &bytes)?;
    settings_cache_set(pool, &key, &etag, &file.to_string_lossy()).await?;
    Ok(json)
}

fn cache_name(url: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{}.json", hex::encode(Sha256::digest(url.as_bytes())))
}

async fn settings_cache_get(pool: &SqlitePool, key: &str) -> AppResult<Option<(String, String)>> {
    let row = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT etag, path FROM cache_meta WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|(e, p)| Some((e.unwrap_or_default(), p?))))
}

async fn settings_cache_set(pool: &SqlitePool, key: &str, etag: &str, path: &str) -> AppResult<()> {
    let ts = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO cache_meta (key, etag, path, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET etag = excluded.etag, path = excluded.path, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(etag)
    .bind(path)
    .bind(ts)
    .execute(pool)
    .await?;
    Ok(())
}

/// Kept so older command signatures still compile during the transition.
pub async fn search_modrinth(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
    filters: &SearchFilters,
) -> AppResult<Page<ProjectHit>> {
    ModrinthAdapter.search(http, pool, cache_dir, filters).await
}

pub async fn list_categories(
    http: &reqwest::Client,
    pool: &SqlitePool,
    cache_dir: &Path,
) -> AppResult<Vec<CatalogCategory>> {
    ModrinthAdapter.categories(http, pool, cache_dir).await
}
