use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::error;

#[derive(Debug)]
pub(crate) struct AppError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl AppError {
    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        let detail: String = message.into();
        tracing::error!(detail, "internal server error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        error!(%error, "internal error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::from(anyhow::Error::from(error))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::bad_request(error.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        error!(%error, "database error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        error!(%error, "provider transport error");
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: "provider transport error".to_string(),
        }
    }
}

impl From<tokio::time::error::Elapsed> for AppError {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Self::bad_request("operation timed out")
    }
}

impl From<StatusCode> for AppError {
    fn from(status: StatusCode) -> Self {
        Self {
            status,
            message: status.to_string(),
        }
    }
}

impl From<anyhow::Error> for Box<AppError> {
    fn from(error: anyhow::Error) -> Self {
        Box::new(AppError::from(error))
    }
}
