use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Mutex;

#[async_trait]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

#[async_trait]
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Default)]
pub struct FixedClock {
    at: Mutex<Option<DateTime<Utc>>>,
}

impl FixedClock {
    pub fn new(at: DateTime<Utc>) -> Self {
        Self {
            at: Mutex::new(Some(at)),
        }
    }

    pub fn set(&self, at: DateTime<Utc>) {
        *self.at.lock().expect("clock lock") = Some(at);
    }

    pub fn advance(&self, by: chrono::Duration) {
        let mut slot = self.at.lock().expect("clock lock");
        let current = slot.unwrap_or_else(Utc::now);
        *slot = Some(current + by);
    }
}

#[async_trait]
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.at.lock().expect("clock lock").unwrap_or_else(Utc::now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_clock_moves_forward() {
        let clock = SystemClock;
        let first = clock.now();
        let second = clock.now();
        assert!(second >= first);
    }

    #[test]
    fn the_fixed_clock_stays_where_it_is_put() {
        let start = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let clock = FixedClock::new(start);
        assert_eq!(clock.now(), start);
        assert_eq!(clock.now(), start);
    }

    #[test]
    fn the_fixed_clock_advances_on_demand() {
        let start = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let clock = FixedClock::new(start);
        clock.advance(chrono::Duration::minutes(30));
        assert_eq!(clock.now(), start + chrono::Duration::minutes(30));
    }

    #[test]
    fn the_fixed_clock_can_be_reset() {
        let start = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let clock = FixedClock::new(start);
        let later = start + chrono::Duration::hours(1);
        clock.set(later);
        assert_eq!(clock.now(), later);
    }

    #[test]
    fn an_unset_fixed_clock_falls_back_to_now() {
        let clock = FixedClock::default();
        assert!(clock.now() <= Utc::now());
    }
}
