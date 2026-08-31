use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

use crate::TenantRuntimeMode;

const MIGRATION_LOCK_ID: i64 = 0x4D41_4E44_4F46_4F52;

pub(crate) async fn run_startup_migrations(
    runtime_pool: &PgPool,
    database_url: &str,
    tenant_runtime_mode: TenantRuntimeMode,
) -> Result<()> {
    let migration_database_url = migration_database_url_from_lookup(
        |key| std::env::var(key).ok(),
        database_url,
        tenant_runtime_mode,
    )?;
    let migration_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&migration_database_url)
        .await
        .context("failed to connect to Postgres with the migration role")?;
    verify_same_database(runtime_pool, &migration_pool).await?;
    let (migration_role, migration_bypasses_rls) = database_role(&migration_pool).await?;
    if !migration_bypasses_rls {
        bail!(
            "database migration role {migration_role} must have BYPASSRLS so the global migration ledger cannot record tenant-partial data migrations"
        );
    }
    if tenant_runtime_mode == TenantRuntimeMode::TenantRouted {
        let runtime_role = require_rls_bound_runtime_role(runtime_pool).await?;
        if runtime_role == migration_role {
            bail!("tenant-routed startup requires distinct runtime and migration database roles");
        }
    }
    let result = run_migrations(&migration_pool).await;
    migration_pool.close().await;
    result?;
    verify_migrations_applied(runtime_pool, tenant_runtime_mode).await
}

pub(crate) async fn verify_same_database(
    runtime_pool: &PgPool,
    migration_pool: &PgPool,
) -> Result<()> {
    let identity_lock_id = Uuid::new_v4().as_u128() as i64;
    let mut migration_transaction = migration_pool
        .begin()
        .await
        .context("failed to begin migration database identity transaction")?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(identity_lock_id)
        .execute(&mut *migration_transaction)
        .await
        .context("failed to mark the migration database identity")?;

    let mut runtime_transaction = runtime_pool
        .begin()
        .await
        .context("failed to begin runtime database identity transaction")?;
    let runtime_acquired_identity_lock: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(identity_lock_id)
            .fetch_one(&mut *runtime_transaction)
            .await
            .context("failed to compare runtime and migration database identities")?;
    runtime_transaction
        .rollback()
        .await
        .context("failed to release the runtime database identity transaction")?;
    migration_transaction
        .rollback()
        .await
        .context("failed to release the migration database identity transaction")?;

    if runtime_acquired_identity_lock {
        bail!("migration and runtime connections must target the same PostgreSQL database");
    }
    Ok(())
}

pub(crate) async fn verify_migrations_applied(
    runtime_pool: &PgPool,
    tenant_runtime_mode: TenantRuntimeMode,
) -> Result<()> {
    if tenant_runtime_mode == TenantRuntimeMode::TenantRouted {
        require_rls_bound_runtime_role(runtime_pool).await?;
    }
    let paths = migration_paths().await?;
    let mut transaction = runtime_pool
        .begin()
        .await
        .context("failed to begin migration verification transaction")?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut *transaction)
        .await
        .context("failed to acquire migration verification lock")?;
    let ledger_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.schema_migrations') IS NOT NULL")
            .fetch_one(&mut *transaction)
            .await?;
    if !ledger_exists {
        bail!("database migrations have not been applied; start the API migration owner first");
    }
    for path in paths {
        let display_path = path.display().to_string();
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .with_context(|| format!("migration path has no UTF-8 filename: {display_path}"))?;
        let sql = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read migration {display_path}"))?;
        let expected = migration_checksum(&sql);
        let applied = sqlx::query_scalar::<_, String>(
            "SELECT checksum FROM schema_migrations WHERE filename = $1",
        )
        .bind(filename)
        .fetch_optional(&mut *transaction)
        .await
        .with_context(|| format!("failed to verify migration ledger for {filename}"))?;
        match applied {
            Some(applied) if applied == expected => {}
            Some(applied) => bail!(
                "migration checksum mismatch for {filename}: database has {applied}, source has {expected}"
            ),
            None => bail!("database migration has not been applied: {filename}"),
        }
    }
    transaction.commit().await?;
    Ok(())
}

pub(crate) fn migration_database_url_from_lookup<F>(
    lookup: F,
    database_url: &str,
    tenant_runtime_mode: TenantRuntimeMode,
) -> Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    let configured = lookup("MANDOFORGE_MIGRATION_DATABASE_URL")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    match (tenant_runtime_mode, configured) {
        (TenantRuntimeMode::TenantRouted, None) => {
            bail!("MANDOFORGE_MIGRATION_DATABASE_URL is required in tenant-routed mode")
        }
        (_, Some(configured)) => Ok(configured),
        (_, None) => Ok(database_url.to_string()),
    }
}

async fn database_role(pool: &PgPool) -> Result<(String, bool)> {
    sqlx::query_as::<_, (String, bool)>(
        "SELECT current_user::text, rolsuper OR rolbypassrls
         FROM pg_roles
         WHERE rolname = current_user",
    )
    .fetch_one(pool)
    .await
    .context("failed to inspect database role row-level security privileges")
}

async fn require_rls_bound_runtime_role(pool: &PgPool) -> Result<String> {
    let (runtime_role, runtime_bypasses_rls) = database_role(pool).await?;
    if runtime_bypasses_rls {
        bail!(
            "tenant-routed database runtime role {runtime_role} must not bypass row-level security"
        );
    }
    Ok(runtime_role)
}

pub(crate) async fn run_migrations(pool: &PgPool) -> Result<()> {
    run_migrations_from_paths(pool, migration_paths().await?).await
}

pub(crate) async fn run_migrations_from_paths(pool: &PgPool, paths: Vec<PathBuf>) -> Result<()> {
    let mut transaction = pool
        .begin()
        .await
        .context("failed to begin migration transaction")?;
    sqlx::query("SET LOCAL row_security = off")
        .execute(&mut *transaction)
        .await
        .context("failed to require global row visibility for database migrations")?;
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
    sqlx::query("GRANT SELECT ON schema_migrations TO PUBLIC")
        .execute(&mut *transaction)
        .await
        .context("failed to grant read-only migration ledger verification")?;

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
