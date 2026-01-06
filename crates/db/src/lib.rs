use std::{future::Future, pin::Pin, str::FromStr, sync::Arc};

use sqlx::{
    Error, Pool, Sqlite,
    sqlite::{SqliteConnectOptions, SqliteConnection, SqlitePoolOptions},
};
use utils::assets::asset_dir;

pub mod models;

#[derive(Clone)]
pub struct DBService {
    pub pool: Pool<Sqlite>,
}

type AfterConnectHook = dyn for<'a> Fn(
        &'a mut SqliteConnection,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>
    + Send
    + Sync
    + 'static;

impl DBService {
    pub async fn new() -> Result<DBService, Error> {
        let pool = Self::create_pool(None).await?;
        Ok(DBService { pool })
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
        let hook: Arc<AfterConnectHook> = Arc::new(after_connect);
        let pool = Self::create_pool(Some(hook)).await?;
        Ok(DBService { pool })
    }

    async fn apply_sqlite_pragmas(conn: &mut SqliteConnection) -> Result<(), Error> {
        // SQLite can return "database is locked" under concurrent load, especially with
        // write-heavy tables like execution_process_logs. These pragmas improve concurrency:
        // - WAL: readers do not block writers (and vice versa) for most operations
        // - busy_timeout: wait for locks instead of failing immediately
        // - synchronous=NORMAL: recommended baseline for WAL mode (durability vs perf)
        sqlx::query("PRAGMA journal_mode = WAL;")
            .execute(&mut *conn)
            .await?;
        sqlx::query("PRAGMA synchronous = NORMAL;")
            .execute(&mut *conn)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 10000;")
            .execute(&mut *conn)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    async fn create_pool(
        after_connect: Option<Arc<AfterConnectHook>>,
    ) -> Result<Pool<Sqlite>, Error> {
        let database_url = format!(
            "sqlite://{}",
            asset_dir().join("db.sqlite").to_string_lossy()
        );
        let options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);

        let pool = if let Some(hook) = after_connect {
            SqlitePoolOptions::new()
                .max_connections(4)
                .after_connect(move |conn, _meta| {
                    let hook = hook.clone();
                    Box::pin(async move {
                        Self::apply_sqlite_pragmas(conn).await?;
                        hook(conn).await?;
                        Ok(())
                    })
                })
                .connect_with(options)
                .await?
        } else {
            SqlitePoolOptions::new()
                .max_connections(4)
                .after_connect(|conn, _meta| {
                    Box::pin(async move {
                        Self::apply_sqlite_pragmas(conn).await?;
                        Ok(())
                    })
                })
                .connect_with(options)
                .await?
        };

        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(pool)
    }
}
