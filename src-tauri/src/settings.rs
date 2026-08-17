use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SettingsMap {
    pub values: HashMap<String, String>,
}

pub async fn get_all(pool: &SqlitePool) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

pub async fn get(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    let row = sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn get_or_default(pool: &SqlitePool, key: &str, default: &str) -> AppResult<String> {
    Ok(get(pool, key).await?.unwrap_or_else(|| default.to_string()))
}
