use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Milliseconds since the Unix epoch, UTC.
///
/// Stored as `INTEGER` in SQLite so sorting and range scans stay cheap. Display
/// formatting belongs to the presentation layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    pub const EPOCH: Self = Self(0);

    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    pub fn as_millis(self) -> i64 {
        self.0
    }

    pub fn now() -> Self {
        Self::from_offset(OffsetDateTime::now_utc())
    }

    pub fn from_offset(at: OffsetDateTime) -> Self {
        Self((at.unix_timestamp_nanos() / 1_000_000) as i64)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::EPOCH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_millis() {
        let stamp = Timestamp::from_millis(1_700_000_000_123);
        assert_eq!(stamp.as_millis(), 1_700_000_000_123);
    }

    #[test]
    fn now_is_after_the_epoch() {
        assert!(Timestamp::now().as_millis() > 0);
    }
}
