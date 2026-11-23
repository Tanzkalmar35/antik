//! Adapters: SQLite, filesystem projection, model transport, process runner.

#![forbid(unsafe_code)]

pub mod clock;
pub mod db;
pub mod files;
pub mod ids;
pub mod logging;
pub mod model;
pub mod runner;
pub mod store;

pub use clock::SystemClock;
pub use db::{Db, StoreError};
pub use files::MarkdownFiles;
pub use ids::SystemIds;
pub use model::Ollama;
pub use runner::ProcessRunner;
