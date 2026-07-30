use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

const MIGRATION_LOCK_ID: i64 = 0x4D41_4E44_4F46_4F52;

pub(crate) async fn run_migrations(pool: &PgPool) -> Result<()> {
    run_migrations_from_paths(pool, migration_paths().await?).await
}

pub(crate) async fn run_migrations_from_paths(pool: &PgPool, paths: Vec<PathBuf>) -> Result<()> {
    let mut transaction = pool
        .begin()
        .await
        .context("failed to begin migration transaction")?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut *transaction)
        .await
        .context("failed to acquire migration lock")?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            filename TEXT PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(&mut *transaction)
    .await
    .context("failed to create migration ledger")?;

    for path in paths {
        let display_path = path.display().to_string();
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .with_context(|| format!("migration path has no UTF-8 filename: {display_path}"))?;
        let sql = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read migration {display_path}"))?;
        let checksum = migration_checksum(&sql);
        if let Some(applied_checksum) = sqlx::query_scalar::<_, String>(
            "SELECT checksum FROM schema_migrations WHERE filename = $1",
        )
        .bind(filename)
        .fetch_optional(&mut *transaction)
        .await
        .with_context(|| format!("failed to inspect migration ledger for {filename}"))?
        {
            if applied_checksum != checksum {
                bail!(
                    "migration checksum mismatch for {filename}: database has {applied_checksum}, source has {checksum}"
                );
            }
            continue;
        }
        sqlx::raw_sql(&sql)
            .execute(&mut *transaction)
            .await
            .with_context(|| format!("failed to execute migration {display_path}"))?;
        sqlx::query("INSERT INTO schema_migrations (filename, checksum) VALUES ($1, $2)")
            .bind(filename)
            .bind(checksum)
            .execute(&mut *transaction)
            .await
            .with_context(|| format!("failed to record migration {filename}"))?;
    }
    transaction
        .commit()
        .await
        .context("failed to commit migrations")?;
    Ok(())
}

pub(crate) fn migration_checksum(sql: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(sql.as_bytes())))
}

pub(crate) async fn migration_paths() -> Result<Vec<PathBuf>> {
    let candidates = std::env::var("MANDOFORGE_MIGRATIONS_DIR")
        .map(|path| vec![PathBuf::from(path)])
        .unwrap_or_else(|_| {
            vec![
                PathBuf::from("db/migrations"),
                PathBuf::from("../../db/migrations"),
            ]
        });
    let mut last_error = None;
    for directory in candidates {
        match tokio::fs::read_dir(&directory).await {
            Ok(mut entries) => {
                let mut paths = Vec::new();
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().and_then(|extension| extension.to_str()) == Some("sql") {
                        paths.push(path);
                    }
                }
                paths.sort();
                return Ok(paths);
            }
            Err(error) => last_error = Some((directory, error)),
        }
    }
    let (directory, error) = last_error.expect("at least one migration directory candidate");
    Err(anyhow::anyhow!(
        "failed to read migrations directory {}: {error}",
        directory.display()
    ))
}

pub(crate) async fn seed_demo_tenant(pool: &PgPool, tenant_id: Uuid) -> Result<()> {
    sqlx::query(
        "INSERT INTO tenants (id, name, slug)
         VALUES ($1, 'Demo Tenant', 'default')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;

    if let Ok(seed_sql) = tokio::fs::read_to_string("db/seed/generic_demo.sql").await {
        sqlx::raw_sql(&seed_sql).execute(pool).await?;
    }
    Ok(())
}
