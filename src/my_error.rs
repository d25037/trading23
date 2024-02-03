#[derive(thiserror::Error, Debug)]
pub enum MyError {
    #[error("401 Unauthorized {0}")]
    IdTokenExpired(String),
    #[error("400 Bad Request Refresh Token may be expired!")]
    RefreshTokenExpired,
    #[error("It is holiday")]
    Holiday,
    // #[error("Not Latest Data")]
    // NotLatestData,
    #[error("out of range for slice of length")]
    OutOfRange,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),
    #[error(transparent)]
    VarError(#[from] std::env::VarError),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Csv(#[from] csv::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}
