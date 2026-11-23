//! Pure domain types. No IO, no async, no database, no Tauri.

#![forbid(unsafe_code)]

pub mod clock;
pub mod concept;
pub mod gate;
pub mod ids;
pub mod model;
pub mod scope;
pub mod sensor;
pub mod time;

pub use clock::Clock;
pub use ids::{ConceptId, DrillId, EventId, GateId, ProjectId, SessionId, TaskId};
pub use time::Timestamp;
