use serde::{Deserialize, Serialize};

/// Typed application error. Every variant maps to one IPC code.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Locked(String),
    #[error("{0}")]
    ModelUnavailable(String),
    #[error("the run timed out")]
    RunnerTimeout,
    #[error("{0}")]
    Io(String),
    /// Carries internals for the log only. Never surfaced verbatim to the UI.
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Invalid(_) => "invalid",
            Self::Conflict(_) => "conflict",
            Self::Locked(_) => "locked",
            Self::ModelUnavailable(_) => "model_unavailable",
            Self::RunnerTimeout => "runner_timeout",
            Self::Io(_) => "io",
            Self::Internal(_) => "internal",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, Self::ModelUnavailable(_) | Self::RunnerTimeout)
    }
}

/// The single error shape the frontend handles.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
}

impl AppErrorDto {
    /// For adapter failures at the command boundary; the detail is logged, not sent.
    pub fn internal(detail: impl Into<String>) -> Self {
        let detail = detail.into();
        tracing::error!(detail = %detail, "internal error at the IPC boundary");
        Self {
            code: "internal".to_owned(),
            message: "Internal error.".to_owned(),
            detail: None,
            retryable: false,
        }
    }
}

impl From<AppError> for AppErrorDto {
    fn from(error: AppError) -> Self {
        let code = error.code().to_owned();
        let retryable = error.retryable();
        let (message, detail) = match error {
            AppError::NotFound(message)
            | AppError::Invalid(message)
            | AppError::Conflict(message)
            | AppError::Locked(message)
            | AppError::ModelUnavailable(message) => (message, None),
            AppError::RunnerTimeout => ("The run timed out.".to_owned(), None),
            AppError::Io(detail) => ("A file operation failed.".to_owned(), Some(detail)),
            AppError::Internal(detail) => {
                tracing::error!(detail = %detail, "internal error");
                ("Internal error.".to_owned(), None)
            }
        };
        Self {
            code,
            message,
            detail,
            retryable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_errors_do_not_leak_detail() {
        let dto = AppErrorDto::from(AppError::Internal(
            "sqlite: SELECT * FROM secrets".to_owned(),
        ));
        assert_eq!(dto.code, "internal");
        assert_eq!(dto.message, "Internal error.");
        assert_eq!(dto.detail, None);
    }

    #[test]
    fn codes_are_stable() {
        assert_eq!(AppError::NotFound(String::new()).code(), "not_found");
        assert_eq!(AppError::Locked(String::new()).code(), "locked");
        assert!(AppError::RunnerTimeout.retryable());
    }
}
