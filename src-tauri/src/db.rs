use crate::error::AppResult;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS profiles (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        display_name TEXT NOT NULL,
        uuid TEXT,
        skin_url TEXT,
        secret_ref TEXT,
        expires_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS instances (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        loader TEXT NOT NULL,
        game_version TEXT NOT NULL,
        loader_version TEXT,
        game_dir TEXT NOT NULL,
        java_path TEXT,
        memory_mb INTEGER NOT NULL DEFAULT 2048,
        jvm_args TEXT,
        keep_open INTEGER NOT NULL DEFAULT 1,
        last_played TEXT,
        icon TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS instance_files (
        id TEXT PRIMARY KEY,
        instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
        lockfile_hash TEXT,
        pin TEXT,
        enabled INTEGER NOT NULL DEFAULT 1,
        sort_order INTEGER NOT NULL DEFAULT 0
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS layouts (
        id TEXT PRIMARY KEY,
        profile_id TEXT,
        preset TEXT NOT NULL DEFAULT 'compact',
        widgets_json TEXT NOT NULL DEFAULT '{}'
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS cache_meta (
        key TEXT PRIMARY KEY,
        etag TEXT,
        path TEXT,
        updated_at TEXT NOT NULL
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS jobs (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        status TEXT NOT NULL,
        progress REAL NOT NULL DEFAULT 0,
        payload TEXT,
        error TEXT,
        created_at TEXT NOT NULL
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS author_projects (
        id TEXT PRIMARY KEY,
        local_path TEXT,
        remote_ids TEXT,
        title TEXT NOT NULL DEFAULT 'Untitled',
        slug TEXT,
        summary TEXT NOT NULL DEFAULT '',
        description TEXT NOT NULL DEFAULT '',
        project_type TEXT NOT NULL DEFAULT 'mod',
        status TEXT NOT NULL DEFAULT 'draft',
        modrinth_id TEXT,
        created_at TEXT NOT NULL DEFAULT '',
        updated_at TEXT NOT NULL DEFAULT ''
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS instance_mods (
        id TEXT PRIMARY KEY,
        instance_id TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
        project_id TEXT NOT NULL,
        version_id TEXT NOT NULL,
        filename TEXT NOT NULL,
        source TEXT NOT NULL DEFAULT 'modrinth',
        sha512 TEXT,
        sha1 TEXT,
        pinned INTEGER NOT NULL DEFAULT 0,
        enabled INTEGER NOT NULL DEFAULT 1,
        channel TEXT,
        sort_order INTEGER NOT NULL DEFAULT 0,
        UNIQUE(instance_id, project_id)
    );
    "#,
];

pub async fn connect(db_path: &Path) -> AppResult<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(pool)
        .await?;
    for sql in MIGRATIONS {
        sqlx::query(sql).execute(pool).await?;
    }
    // Existing installs already have instance_mods; add display columns if missing.
    for sql in [
        "ALTER TABLE instance_mods ADD COLUMN display_name TEXT",
        "ALTER TABLE instance_mods ADD COLUMN version_number TEXT",
        "ALTER TABLE author_projects ADD COLUMN title TEXT NOT NULL DEFAULT 'Untitled'",
        "ALTER TABLE author_projects ADD COLUMN slug TEXT",
        "ALTER TABLE author_projects ADD COLUMN summary TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE author_projects ADD COLUMN description TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE author_projects ADD COLUMN project_type TEXT NOT NULL DEFAULT 'mod'",
        "ALTER TABLE author_projects ADD COLUMN status TEXT NOT NULL DEFAULT 'draft'",
        "ALTER TABLE author_projects ADD COLUMN modrinth_id TEXT",
        "ALTER TABLE author_projects ADD COLUMN created_at TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE author_projects ADD COLUMN updated_at TEXT NOT NULL DEFAULT ''",
    ] {
        let _ = sqlx::query(sql).execute(pool).await;
    }
    Ok(())
}
