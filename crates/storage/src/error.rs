use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum StorageError {
    #[snafu(display("storage entity '{entity}' with id '{id}' was not found"))]
    NotFound {
        stage: &'static str,
        entity: &'static str,
        id: String,
    },
    #[snafu(display("storage conflict for '{entity}': {details}"))]
    Conflict {
        stage: &'static str,
        entity: &'static str,
        details: String,
    },
    #[snafu(display("storage id '{raw}' is invalid for {id_type}"))]
    InvalidId {
        stage: &'static str,
        id_type: &'static str,
        raw: String,
        source: uuid::Error,
    },
    #[snafu(display("storage invariant violation: {details}"))]
    InvariantViolation {
        stage: &'static str,
        details: String,
    },
    #[snafu(display("failed to create sqlite directory at {path}"))]
    CreateSqliteDirectory {
        stage: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[snafu(display("failed to parse sqlite connection URL '{database_url}'"))]
    SqliteConnectOptions {
        stage: &'static str,
        database_url: String,
        source: sqlx::Error,
    },
    #[snafu(display("failed to connect sqlite database '{database_url}'"))]
    SqliteConnect {
        stage: &'static str,
        database_url: String,
        source: sqlx::Error,
    },
    #[snafu(display("failed to configure sqlite pragma '{pragma}'"))]
    SqlitePragma {
        stage: &'static str,
        pragma: &'static str,
        source: sqlx::Error,
    },
    #[snafu(display("failed to run sqlite migrations"))]
    SqliteMigrate {
        stage: &'static str,
        source: sqlx::migrate::MigrateError,
    },
    #[snafu(display("sqlite query failed at {stage}: {source}"))]
    SqliteQuery {
        stage: &'static str,
        source: sqlx::Error,
    },
    #[snafu(display("failed to spawn sqlite worker thread"))]
    SqliteThreadSpawn {
        stage: &'static str,
        source: std::io::Error,
    },
    #[snafu(display("failed to initialize sqlite worker runtime"))]
    SqliteRuntimeInit {
        stage: &'static str,
        source: std::io::Error,
    },
    #[snafu(display("failed to read legacy conversation TSV from {path}"))]
    ReadLegacyConversationTsv {
        stage: &'static str,
        path: String,
        source: std::io::Error,
    },
}

pub type StorageResult<T> = Result<T, StorageError>;
