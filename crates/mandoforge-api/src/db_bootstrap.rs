use std::path::PathBuf;

use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

pub(crate) async fn run_migrations(pool: &PgPool) -> Result<()> {
    for path in migration_paths().await? {
        let display_path = path.display().to_string();
        let sql = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read migration {display_path}"))?;
        sqlx::raw_sql(&sql)
            .execute(pool)
            .await
            .with_context(|| format!("failed to execute migration {display_path}"))?;
    }
    Ok(())
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
