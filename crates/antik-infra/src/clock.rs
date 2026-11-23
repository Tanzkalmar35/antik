use antik_domain::{Clock, Timestamp};

/// Wall-clock time for the running application.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}
