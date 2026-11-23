use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

macro_rules! ulid_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(::ulid::Ulid);

        impl $name {
            pub fn generate() -> Self {
                Self(::ulid::Ulid::generate())
            }

            pub fn from_ulid(id: ::ulid::Ulid) -> Self {
                Self(id)
            }

            pub fn as_ulid(self) -> ::ulid::Ulid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = ::ulid::DecodeError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                ::ulid::Ulid::from_string(value).map(Self)
            }
        }
    };
}

ulid_id!(
    /// A project folder.
    ProjectId
);
ulid_id!(
    /// A batch of work inside a project.
    TaskId
);
ulid_id!(
    /// A knowledge gate attached to a task.
    GateId
);
ulid_id!(
    /// An appended audit event.
    EventId
);
ulid_id!(
    /// One sitting between a recall and a handoff.
    SessionId
);

/// Concepts are human-authored and referenced by the developer ("K-041").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConceptId(String);

impl ConceptId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConceptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Drills are configuration, identified by a stable slug ("shrink-the-input").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DrillId(String);

impl DrillId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DrillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
