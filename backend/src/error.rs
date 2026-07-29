use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    External(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            Self::Validation(message) => (StatusCode::BAD_REQUEST, "validation_error", message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, "unauthorized", message),
            Self::External(message) => (StatusCode::BAD_GATEWAY, "external_service_error", message),
            Self::Database(error) => {
                tracing::error!(?error, "database request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "数据库操作失败".to_owned(),
                )
            }
        };

        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
