//! Errors sent to the UI as a code plus parameters, e.g. `{"code": "http", "status": 503}`.
//! The engine never translates: the UI looks up `errors.<code>` in its locale files, so a
//! message always follows the current language, even when it was saved to disk earlier.

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AppError {
    // Library
    CollectionNotFound,
    SourceNotFound,
    EmptyName,
    DuplicateName,
    NestedSubcollection,
    MoveAcrossLevels,
    SameSourceAndDestination,
    LinkNotRecognized,
    FileListMissing,
    LibraryReadOnly { detail: String },
    LibraryUnreadable { detail: String },
    LibraryMovedAside,
    LibraryCorruptNotMoved { detail: String },
    SaveLibraryFailed { detail: String },
    SaveSourceFailed { detail: String },
    // archive.org
    Network { detail: String },
    Http { status: u16 },
    InvalidResponse { detail: String },
    ItemUnavailable,
    ItemUnavailableLogIn,
    RedirectWithoutLocation,
    TooManyRedirects,
    AccountNotFound,
    WrongPassword,
    LoginFailed { reason: String },
    IncompleteLoginResponse,
    // Downloads
    IncompleteFile { got: u64, expected: u64 },
    WrongSize { got: u64, expected: u64 },
    AccessDeniedRestricted,
    AccessDeniedLogIn,
    FileNotFound,
    ServerBusy { status: u16 },
    ConnectionDropped { detail: String },
    DiskProblem { detail: String },
    JobNotInQueue,
    // Files and settings
    InvalidFolder,
    OpenFolderFailed { detail: String },
    ReadFileFailed { detail: String },
    SaveFileFailed { detail: String },
    SaveSettingsFailed { detail: String },
    SaveSessionFailed { detail: String },
    /// Unexpected failures, shown with their technical detail only.
    Other { detail: String },
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn other(e: impl Display) -> Self {
        AppError::Other { detail: e.to_string() }
    }
}

/// Errors saved before they had a code were plain text: they are kept as `Other`, so an old
/// queue.json or library.json still loads instead of being discarded.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Coded(AppError),
    Text(String),
}

impl From<Stored> for AppError {
    fn from(s: Stored) -> Self {
        match s {
            Stored::Coded(e) => e,
            Stored::Text(detail) => AppError::Other { detail },
        }
    }
}

pub fn lenient<'de, D: Deserializer<'de>>(d: D) -> Result<AppError, D::Error> {
    Stored::deserialize(d).map(Into::into)
}

pub fn lenient_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<AppError>, D::Error> {
    Ok(Option::<Stored>::deserialize(d)?.map(Into::into))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_as_code_and_params() {
        assert_eq!(serde_json::to_value(AppError::Http { status: 503 }).unwrap(), json!({"code": "http", "status": 503}));
        assert_eq!(serde_json::to_value(AppError::CollectionNotFound).unwrap(), json!({"code": "collection_not_found"}));
    }

    #[test]
    fn old_plain_text_errors_still_load() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "lenient")]
            e: AppError,
            #[serde(default, deserialize_with = "lenient_opt")]
            o: Option<AppError>,
        }
        let s: S = serde_json::from_value(json!({"e": "Errore HTTP 503", "o": {"code": "wrong_password"}})).unwrap();
        assert_eq!(s.e, AppError::Other { detail: "Errore HTTP 503".into() });
        assert_eq!(s.o, Some(AppError::WrongPassword));
        let s: S = serde_json::from_value(json!({"e": {"code": "http", "status": 404}, "o": null})).unwrap();
        assert_eq!((s.e, s.o), (AppError::Http { status: 404 }, None));
        let s: S = serde_json::from_value(json!({"e": {"code": "file_not_found"}})).unwrap();
        assert_eq!(s.o, None);
    }
}
