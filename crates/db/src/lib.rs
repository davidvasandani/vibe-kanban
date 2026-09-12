use std::{str::FromStr, sync::Arc, time::Duration};

use sqlx::{
    ConnectOptions, Connection, Error, Pool, Sqlite, SqlitePool,
    migrate::MigrateError,
    query_scalar,
    sqlite::{SqliteConnectOptions, SqliteConnection, SqlitePoolOptions, SqliteSynchronous},
};
use utils::assets::asset_dir;

pub mod models;

const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

fn sqlite_connect_options(database_url: &str) -> Result<SqliteConnectOptions, Error> {
    sqlite_connect_options_with_timeout(database_url, SQLITE_BUSY_TIMEOUT)
}

fn sqlite_connect_options_with_timeout(
    database_url: &str,
    busy_timeout: Duration,
) -> Result<SqliteConnectOptions, Error> {
    Ok(SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(busy_timeout))
}

/// Performs the persistent DELETE-to-WAL transition once, before a pool can
/// create concurrent connections. The transition needs an exclusive database
/// lock; startup therefore fails clearly if an older process still owns an
/// active transaction instead of letting mixed journal modes race.
///
/// WAL requires a local filesystem with reliable shared-memory locking. The
/// application data directory must not be placed on NFS or a similar network
/// filesystem.
async fn prepare_database_for_concurrency(database_url: &str) -> Result<(), Error> {
    prepare_database_for_concurrency_with_timeout(database_url, SQLITE_BUSY_TIMEOUT).await
}

async fn prepare_database_for_concurrency_with_timeout(
    database_url: &str,
    busy_timeout: Duration,
) -> Result<(), Error> {
    let mut connection = SqliteConnection::connect_with(&sqlite_connect_options_with_timeout(
        database_url,
        busy_timeout,
    )?)
    .await?;
    let current_mode: String = query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut connection)
        .await?;

    if current_mode != "wal" {
        let selected_mode: String = query_scalar("PRAGMA journal_mode = WAL")
            .fetch_one(&mut connection)
            .await?;
        if selected_mode != "wal" {
            return Err(Error::Protocol(format!(
                "failed to enable SQLite WAL mode; database selected {selected_mode}"
            )));
        }
    }

    connection.close().await
}

async fn run_migrations(pool: &Pool<Sqlite>) -> Result<(), Error> {
    use std::collections::HashSet;

    let migrator = sqlx::migrate!("./migrations");
    let mut processed_versions: HashSet<i64> = HashSet::new();

    loop {
        match migrator.run(pool).await {
            Ok(()) => return Ok(()),
            Err(MigrateError::VersionMismatch(version)) => {
                if cfg!(debug_assertions) {
                    // return the error in debug mode to catch migration issues early
                    return Err(sqlx::Error::Migrate(Box::new(
                        MigrateError::VersionMismatch(version),
                    )));
                }

                if !cfg!(windows) {
                    // On non-Windows platforms, we do not attempt to auto-fix checksum mismatches
                    return Err(sqlx::Error::Migrate(Box::new(
                        MigrateError::VersionMismatch(version),
                    )));
                }

                // Guard against infinite loop
                if !processed_versions.insert(version) {
                    return Err(sqlx::Error::Migrate(Box::new(
                        MigrateError::VersionMismatch(version),
                    )));
                }

                // On Windows, there can be checksum mismatches due to line ending differences
                // or other platform-specific issues. Update the stored checksum and retry.
                tracing::warn!(
                    "Migration version {} has checksum mismatch, updating stored checksum (likely platform-specific difference)",
                    version
                );

                // Find the migration with the mismatched version and get its current checksum
                if let Some(migration) = migrator.iter().find(|m| m.version == version) {
                    // Update the checksum in _sqlx_migrations to match the current file
                    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                        .bind(&*migration.checksum)
                        .bind(version)
                        .execute(pool)
                        .await?;
                } else {
                    // Migration not found in current set, can't fix
                    return Err(sqlx::Error::Migrate(Box::new(
                        MigrateError::VersionMismatch(version),
                    )));
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[derive(Clone)]
pub struct DBService {
    pub pool: Pool<Sqlite>,
}

impl DBService {
    pub async fn new() -> Result<DBService, Error> {
        let database_url = format!(
            "sqlite://{}",
            asset_dir().join("db.v2.sqlite").to_string_lossy()
        );
        prepare_database_for_concurrency(&database_url).await?;
        let options = sqlite_connect_options(&database_url)?;
        let pool = SqlitePool::connect_with(options).await?;
        run_migrations(&pool).await?;
        Ok(DBService { pool })
    }

    pub async fn new_migration_pool() -> Result<Pool<Sqlite>, Error> {
        let database_url = format!(
            "sqlite://{}",
            asset_dir().join("db.v2.sqlite").to_string_lossy()
        );
        prepare_database_for_concurrency(&database_url).await?;
        let options = sqlite_connect_options(&database_url)?.disable_statement_logging();
        SqlitePoolOptions::new()
            .max_connections(64)
            .connect_with(options)
            .await
    }

    pub async fn new_with_after_connect<F>(after_connect: F) -> Result<DBService, Error>
    where
        F: for<'a> Fn(
                &'a mut SqliteConnection,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>,
            > + Send
            + Sync
            + 'static,
    {
        let pool = Self::create_pool(Some(Arc::new(after_connect))).await?;
        Ok(DBService { pool })
    }

    async fn create_pool<F>(after_connect: Option<Arc<F>>) -> Result<Pool<Sqlite>, Error>
    where
        F: for<'a> Fn(
                &'a mut SqliteConnection,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>,
            > + Send
            + Sync
            + 'static,
    {
        let database_url = format!(
            "sqlite://{}",
            asset_dir().join("db.v2.sqlite").to_string_lossy()
        );
        prepare_database_for_concurrency(&database_url).await?;
        let options = sqlite_connect_options(&database_url)?;

        let pool = if let Some(hook) = after_connect {
            SqlitePoolOptions::new()
                .after_connect(move |conn, _meta| {
                    let hook = hook.clone();
                    Box::pin(async move {
                        hook(conn).await?;
                        Ok(())
                    })
                })
                .connect_with(options)
                .await?
        } else {
            SqlitePool::connect_with(options).await?
        };

        run_migrations(&pool).await?;
        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        str::FromStr,
        time::{Duration, SystemTime},
    };

    use sqlx::{
        SqlitePool, query, query_scalar,
        sqlite::{SqliteConnectOptions, SqliteJournalMode},
    };

    use super::{
        SQLITE_BUSY_TIMEOUT, prepare_database_for_concurrency,
        prepare_database_for_concurrency_with_timeout, sqlite_connect_options,
    };

    #[tokio::test]
    async fn wal_transition_is_explicit_and_preserves_an_existing_database() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let database_path = std::env::temp_dir().join(format!(
            "vibe-kanban-sqlite-transition-{}-{unique}.sqlite",
            std::process::id()
        ));
        let database_url = format!("sqlite://{}", database_path.to_string_lossy());

        let delete_options = SqliteConnectOptions::from_str(&database_url)
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Delete);
        let old_pool = SqlitePool::connect_with(delete_options).await.unwrap();
        query("CREATE TABLE existing_data (value TEXT NOT NULL)")
            .execute(&old_pool)
            .await
            .unwrap();
        query("INSERT INTO existing_data(value) VALUES ('preserved')")
            .execute(&old_pool)
            .await
            .unwrap();

        let mut old_reader = old_pool.acquire().await.unwrap();
        query("BEGIN").execute(&mut *old_reader).await.unwrap();
        query_scalar::<_, i64>("SELECT count(*) FROM existing_data")
            .fetch_one(&mut *old_reader)
            .await
            .unwrap();
        assert!(
            prepare_database_for_concurrency_with_timeout(
                &database_url,
                Duration::from_millis(25),
            )
            .await
            .is_err(),
            "journal transition must fail while an old transaction is active"
        );
        query("ROLLBACK").execute(&mut *old_reader).await.unwrap();
        drop(old_reader);
        old_pool.close().await;

        prepare_database_for_concurrency(&database_url)
            .await
            .unwrap();
        let pool = SqlitePool::connect_with(sqlite_connect_options(&database_url).unwrap())
            .await
            .unwrap();

        let journal_mode: String = query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap();
        let synchronous: i64 = query_scalar("PRAGMA synchronous")
            .fetch_one(&pool)
            .await
            .unwrap();
        let busy_timeout_ms: i64 = query_scalar("PRAGMA busy_timeout")
            .fetch_one(&pool)
            .await
            .unwrap();
        let preserved: String = query_scalar("SELECT value FROM existing_data")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(journal_mode, "wal");
        assert_eq!(synchronous, 1, "SQLITE_SYNC_NORMAL");
        assert_eq!(busy_timeout_ms, SQLITE_BUSY_TIMEOUT.as_millis() as i64);
        assert_eq!(preserved, "preserved");

        pool.close().await;
        for suffix in ["", "-shm", "-wal"] {
            let _ = std::fs::remove_file(format!("{}{}", database_path.display(), suffix));
        }
    }
}
