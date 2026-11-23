use crate::time::Timestamp;

/// Injected time so domain and application tests never sleep.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
