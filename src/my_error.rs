#[derive(thiserror::Error, Debug)]
pub enum MyError {
    #[error("401 Unauthorized {0}")]
    IdTokenExpired(String),
    #[error("400 Bad Request Refresh Token may be expired!")]
    RefreshTokenExpired,
    #[error("It is holiday")]
    Holiday,
    #[error("out of range for slice of length")]
    OutOfRange,
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}
