//! In-process throttling of failed sign-in attempts.
//!
//! Bounds online password guessing. Argon2 already makes each attempt costly,
//! but nothing otherwise limits how many an attacker may make.
//!
//! State is per-process and lost on restart, which is adequate for the
//! single-binary deployment model. A multi-instance deployment would need a
//! shared store.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Failures tolerated within [`WINDOW`] before the key is refused.
pub const MAX_FAILURES: u32 = 5;

/// Sliding window over which failures accumulate.
pub const WINDOW: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy)]
struct Attempts {
    count: u32,
    window_started: Instant,
}

static FAILURES: LazyLock<Mutex<HashMap<String, Attempts>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Key a throttle bucket by account *and* source, so one attacker cannot lock
/// every user out and one account cannot be attacked from many addresses freely.
pub fn key(username: &str, source: &str) -> String {
    format!("{}|{}", username.trim().to_lowercase(), source)
}

fn with_state<R>(f: impl FnOnce(&mut HashMap<String, Attempts>) -> R) -> R {
    // A poisoned lock must not disable throttling, so recover the guard.
    let mut guard = FAILURES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn locked_at(state: &mut HashMap<String, Attempts>, key: &str, now: Instant) -> bool {
    match state.get(key) {
        Some(attempts) if now.duration_since(attempts.window_started) >= WINDOW => {
            state.remove(key);
            false
        }
        Some(attempts) => attempts.count >= MAX_FAILURES,
        None => false,
    }
}

fn record_failure_at(state: &mut HashMap<String, Attempts>, key: &str, now: Instant) {
    // Opportunistic pruning keeps the map bounded without a background task.
    state.retain(|_, attempts| now.duration_since(attempts.window_started) < WINDOW);

    let entry = state.entry(key.to_string()).or_insert(Attempts {
        count: 0,
        window_started: now,
    });
    entry.count = entry.count.saturating_add(1);
}

/// Whether the key has exhausted its allowance
pub fn is_locked(key: &str) -> bool {
    let now = Instant::now();
    with_state(|state| locked_at(state, key, now))
}

/// Count one failed attempt against the key
pub fn record_failure(key: &str) {
    let now = Instant::now();
    with_state(|state| record_failure_at(state, key, now));
}

/// Forget a key's failures after a successful sign-in
pub fn clear(key: &str) {
    with_state(|state| state.remove(key));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> HashMap<String, Attempts> {
        HashMap::new()
    }

    #[test]
    fn allows_attempts_below_the_limit() {
        let mut s = state();
        let now = Instant::now();

        for _ in 0..MAX_FAILURES - 1 {
            record_failure_at(&mut s, "k", now);
        }

        assert!(!locked_at(&mut s, "k", now));
    }

    #[test]
    fn locks_once_the_limit_is_reached() {
        let mut s = state();
        let now = Instant::now();

        for _ in 0..MAX_FAILURES {
            record_failure_at(&mut s, "k", now);
        }

        assert!(locked_at(&mut s, "k", now));
    }

    #[test]
    fn the_window_expires() {
        let mut s = state();
        let start = Instant::now();

        for _ in 0..MAX_FAILURES {
            record_failure_at(&mut s, "k", start);
        }
        assert!(locked_at(&mut s, "k", start));

        let later = start + WINDOW + Duration::from_secs(1);
        assert!(!locked_at(&mut s, "k", later));
    }

    #[test]
    fn keys_are_independent() {
        let mut s = state();
        let now = Instant::now();

        for _ in 0..MAX_FAILURES {
            record_failure_at(&mut s, "victim", now);
        }

        assert!(locked_at(&mut s, "victim", now));
        assert!(!locked_at(&mut s, "bystander", now));
    }

    #[test]
    fn expired_entries_are_pruned() {
        let mut s = state();
        let start = Instant::now();
        record_failure_at(&mut s, "stale", start);

        record_failure_at(&mut s, "fresh", start + WINDOW + Duration::from_secs(1));

        assert!(!s.contains_key("stale"));
        assert!(s.contains_key("fresh"));
    }

    #[test]
    fn key_separates_account_from_source() {
        assert_eq!(key("Admin", "10.0.0.1"), key("  admin ", "10.0.0.1"));
        assert_ne!(key("admin", "10.0.0.1"), key("admin", "10.0.0.2"));
    }
}
