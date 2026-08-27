use std::borrow::Cow;
use std::collections::HashMap;

use sqlx::AssertSqlSafe;
use sqlx::SqlSafeStr;
use sqlx::SqlitePool;
use sqlx::migrate::Migration;
use sqlx::migrate::Migrator;

pub(crate) static STATE_MIGRATOR: Migrator = sqlx::migrate!("./migrations");
pub(crate) static LOGS_MIGRATOR: Migrator = sqlx::migrate!("./logs_migrations");
pub(crate) static GOALS_MIGRATOR: Migrator = sqlx::migrate!("./goals_migrations");
pub(crate) static MEMORIES_MIGRATOR: Migrator = sqlx::migrate!("./memory_migrations");
pub(crate) static QUEUE_MIGRATOR: Migrator = sqlx::migrate!("./queue_migrations");
pub(crate) static THREAD_HISTORY_MIGRATOR: Migrator = sqlx::migrate!("./thread_history_migrations");

fn migration_with_sql(migration: &Migration, sql: String) -> Migration {
    Migration::new(
        migration.version,
        migration.description.clone(),
        migration.migration_type,
        AssertSqlSafe(sql).into_sql_str(),
        migration.no_tx,
    )
}

fn migration_with_lf_line_endings(migration: &Migration) -> Migration {
    let sql = migration.sql.as_str().replace("\r\n", "\n");
    migration_with_sql(migration, sql)
}

fn migration_with_crlf_line_endings(migration: &Migration) -> Migration {
    let migration = migration_with_lf_line_endings(migration);
    let sql = migration.sql.as_str().replace('\n', "\r\n");
    migration_with_sql(&migration, sql)
}

fn checksum_matches_line_ending_variant(migration: &Migration, checksum: &[u8]) -> bool {
    migration_with_lf_line_endings(migration).checksum.as_ref() == checksum
        || migration_with_crlf_line_endings(migration)
            .checksum
            .as_ref()
            == checksum
}

fn migrator_with_migrations(base: &Migrator, migrations: Cow<'static, [Migration]>) -> Migrator {
    Migrator {
        migrations,
        ignore_missing: base.ignore_missing,
        locking: base.locking,
        no_tx: base.no_tx,
        table_name: base.table_name.clone(),
        create_schemas: base.create_schemas.clone(),
    }
}

/// Allow an older Codex binary to open a database that has already been
/// migrated by a newer binary running in parallel.
///
/// We intentionally ignore applied migration versions that are newer than the
/// embedded migration set. Known migration versions are still validated by
/// checksum, so this only relaxes the "database is ahead of me" case.
fn runtime_migrator(base: &'static Migrator) -> Migrator {
    let mut migrator = migrator_with_migrations(
        base,
        Cow::Owned(
            base.migrations
                .iter()
                .map(|migration| {
                    if cfg!(windows) {
                        migration_with_crlf_line_endings(migration)
                    } else {
                        migration_with_lf_line_endings(migration)
                    }
                })
                .collect(),
        ),
    );
    migrator.ignore_missing = true;
    migrator
}

pub(crate) fn runtime_state_migrator() -> Migrator {
    runtime_migrator(&STATE_MIGRATOR)
}

pub(crate) fn runtime_logs_migrator() -> Migrator {
    runtime_migrator(&LOGS_MIGRATOR)
}

pub(crate) fn runtime_goals_migrator() -> Migrator {
    runtime_migrator(&GOALS_MIGRATOR)
}

pub(crate) fn runtime_memories_migrator() -> Migrator {
    runtime_migrator(&MEMORIES_MIGRATOR)
}

pub(crate) fn runtime_queue_migrator() -> Migrator {
    runtime_migrator(&QUEUE_MIGRATOR)
}

// The paginated history projector will call this when it takes ownership of opening the database.
#[allow(dead_code)]
pub(crate) fn runtime_thread_history_migrator() -> Migrator {
    runtime_migrator(&THREAD_HISTORY_MIGRATOR)
}

async fn migrations_table_exists(pool: &SqlitePool) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?
    .is_some())
}

/// Accept migration history written with the other platform's line endings.
///
/// SQLx validates hashes of the source text, including line endings. Runtime
/// migrations use CRLF on Windows to remain compatible with released Windows
/// builds and LF elsewhere. For an exact line-ending-only variant, validate
/// against the already-applied hash. The stored history remains unchanged so
/// the binary that wrote it can still use the DB.
pub(crate) async fn migrator_for_applied_line_endings(
    pool: &SqlitePool,
    migrator: &Migrator,
) -> anyhow::Result<Migrator> {
    if !migrations_table_exists(pool).await? {
        return Ok(migrator_with_migrations(
            migrator,
            migrator.migrations.clone(),
        ));
    }

    let applied_migrations =
        sqlx::query_as::<_, (i64, Vec<u8>)>("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect::<HashMap<_, _>>();
    let migrations = migrator
        .migrations
        .iter()
        .cloned()
        .map(|mut migration| {
            if let Some(applied_checksum) = applied_migrations.get(&migration.version)
                && migration.checksum.as_ref() != applied_checksum
                && checksum_matches_line_ending_variant(&migration, applied_checksum)
            {
                migration.checksum = Cow::Owned(applied_checksum.clone());
            }
            migration
        })
        .collect();

    Ok(migrator_with_migrations(migrator, Cow::Owned(migrations)))
}

pub(crate) async fn repair_legacy_recency_migration_version(
    pool: &SqlitePool,
    migrator: &Migrator,
) -> anyhow::Result<()> {
    let Some(recency_migration) = migrator
        .migrations
        .iter()
        .find(|migration| migration.version == 39)
    else {
        return Ok(());
    };
    if !migrations_table_exists(pool).await? {
        return Ok(());
    }

    let legacy_checksum = sqlx::query_scalar::<_, Vec<u8>>(
        r#"
SELECT checksum
FROM _sqlx_migrations
WHERE version = ?
  AND NOT EXISTS (
      SELECT 1 FROM _sqlx_migrations WHERE version = ?
  )
        "#,
    )
    .bind(38_i64)
    .bind(recency_migration.version)
    .fetch_optional(pool)
    .await?;
    let Some(legacy_checksum) = legacy_checksum else {
        return Ok(());
    };
    if !checksum_matches_line_ending_variant(recency_migration, &legacy_checksum) {
        return Ok(());
    }

    sqlx::query(
        r#"
UPDATE _sqlx_migrations
SET version = ?, description = ?
WHERE version = ?
  AND checksum = ?
  AND NOT EXISTS (
      SELECT 1 FROM _sqlx_migrations WHERE version = ?
  )
        "#,
    )
    .bind(recency_migration.version)
    .bind(recency_migration.description.as_ref())
    .bind(38_i64)
    .bind(legacy_checksum)
    .bind(recency_migration.version)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "migrations_tests.rs"]
mod tests;
