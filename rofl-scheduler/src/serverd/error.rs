use axum::{http::StatusCode, response};

/// API error.
#[derive(Debug)]
pub enum Error {
    BadAuthToken,
    NotFound,
    Forbidden,
    Other(anyhow::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadAuthToken => write!(f, "bad authentication token"),
            Self::NotFound => write!(f, "not found"),
            Self::Forbidden => write!(f, "forbidden"),
            Self::Other(err) => err.fmt(f),
        }
    }
}

impl response::IntoResponse for Error {
    fn into_response(self) -> response::Response {
        let status_code = match self {
            Self::BadAuthToken => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Forbidden => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self}")).into_response()
    }
}

impl<E> From<E> for Error
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self::Other(err.into())
    }
}
