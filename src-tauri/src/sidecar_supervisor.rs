//! Process-independent recovery policy. The host owns child handles and health I/O.
//!
//! The initial launch and explicit manual launches do not consume the eight
//! automatic attempts. A spawned process is not considered stable until health
//! observations have stayed successful for 30 seconds; simply spawning or briefly
//! answering health never resets the budget. All times come from the caller.

use std::time::{Duration, Instant};

pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(1);
pub(crate) const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const STABLE_HEALTH: Duration = Duration::from_secs(30);
const RETRY_DELAYS: [u64; 8] = [1, 2, 4, 8, 16, 30, 30, 30];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HealthObservation {
    Inactive,
    Waiting,
    TimedOut,
    Healthy {
        became_healthy: bool,
        budget_reset: bool,
    },
}

#[derive(Debug, Default)]
enum Phase {
    #[default]
    Dormant,
    Launching,
    Monitoring {
        unhealthy_since: Option<Instant>,
        healthy_since: Option<Instant>,
        stable: bool,
    },
    Backoff {
        retry_at: Instant,
    },
    Exhausted,
}

#[derive(Debug, Default)]
pub(crate) struct RetryPolicy {
    retries_started: usize,
    phase: Phase,
}

impl RetryPolicy {
    /// Record only a successful spawn. Readiness still requires a health probe.
    pub(crate) fn record_spawn(&mut self, now: Instant) {
        self.phase = Phase::Monitoring {
            unhealthy_since: Some(now),
            healthy_since: None,
            stable: false,
        };
    }

    /// Account for one failed launch or process generation. Repeated failure
    /// reports while already waiting do not extend the deadline or burn budget.
    pub(crate) fn record_failure(&mut self, now: Instant) {
        if matches!(self.phase, Phase::Backoff { .. } | Phase::Exhausted) {
            return;
        }
        self.phase = match RETRY_DELAYS.get(self.retries_started) {
            Some(seconds) => Phase::Backoff {
                retry_at: now + Duration::from_secs(*seconds),
            },
            None => Phase::Exhausted,
        };
    }

    /// Claim one due automatic attempt before calling spawn. This changes phase
    /// immediately, so a repeated tick cannot launch twice for the same deadline.
    pub(crate) fn take_retry(&mut self, now: Instant) -> bool {
        if !matches!(self.phase, Phase::Backoff { retry_at } if now >= retry_at) {
            return false;
        }
        self.retries_started += 1;
        self.phase = Phase::Launching;
        true
    }

    /// A deliberate user action starts a fresh budget and bypasses backoff. The
    /// caller launches immediately, then records spawn success or failure.
    pub(crate) fn manual_retry(&mut self) {
        self.retries_started = 0;
        self.phase = Phase::Launching;
    }

    /// Monitor a known owned child. On timeout the caller must stop that owned
    /// child and record failure; this module never authorizes killing by PID.
    /// The same grace period covers startup and a later continuous health outage.
    pub(crate) fn observe_health(&mut self, now: Instant, healthy: bool) -> HealthObservation {
        let Phase::Monitoring {
            unhealthy_since,
            healthy_since,
            stable,
        } = &mut self.phase
        else {
            return HealthObservation::Inactive;
        };
        if healthy {
            let became_healthy = healthy_since.is_none();
            let since = *healthy_since.get_or_insert(now);
            *unhealthy_since = None;
            let budget_reset = !*stable && now.saturating_duration_since(since) >= STABLE_HEALTH;
            if budget_reset {
                self.retries_started = 0;
                *stable = true;
            }
            HealthObservation::Healthy {
                became_healthy,
                budget_reset,
            }
        } else {
            *healthy_since = None;
            *stable = false;
            let since = *unhealthy_since.get_or_insert(now);
            if now.saturating_duration_since(since) >= STARTUP_TIMEOUT {
                HealthObservation::TimedOut
            } else {
                HealthObservation::Waiting
            }
        }
    }

    pub(crate) fn needs_health_probe(&self) -> bool {
        matches!(self.phase, Phase::Monitoring { .. })
    }

    pub(crate) fn health_probe_interval(&self) -> Duration {
        if matches!(self.phase, Phase::Monitoring { stable: true, .. }) {
            Duration::from_secs(30)
        } else {
            POLL_INTERVAL
        }
    }

    pub(crate) fn exhausted(&self) -> bool {
        matches!(self.phase, Phase::Exhausted)
    }

    pub(crate) fn retries_started(&self) -> usize {
        self.retries_started
    }

    pub(crate) fn retry_after(&self, now: Instant) -> Option<Duration> {
        match self.phase {
            Phase::Backoff { retry_at } => Some(retry_at.saturating_duration_since(now)),
            _ => None,
        }
    }
}

/// A healthy response is usable only if the same owned generation survived it.
pub(crate) fn probe_current_generation(
    expected: u64,
    probe: impl FnOnce() -> bool,
    mut current: impl FnMut() -> Result<u64, String>,
) -> Result<bool, String> {
    if current()? != expected { return Ok(false); }
    Ok(probe() && current()? == expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    fn healthy(became_healthy: bool, budget_reset: bool) -> HealthObservation {
        HealthObservation::Healthy {
            became_healthy,
            budget_reset,
        }
    }

    fn recovering(now: Instant) -> RetryPolicy {
        let mut policy = RetryPolicy::default();
        policy.record_failure(now);
        assert!(policy.take_retry(now + seconds(1)));
        policy.record_spawn(now + seconds(1));
        policy
    }

    #[test]
    fn initial_spawn_failure_waits_before_first_automatic_attempt() {
        let now = Instant::now();
        let mut policy = RetryPolicy::default();
        assert!(!policy.take_retry(now));
        assert!(!policy.needs_health_probe());
        policy.record_failure(now);
        assert_eq!(policy.retry_after(now), Some(seconds(1)));
        assert_eq!(policy.retries_started(), 0);
        assert!(!policy.take_retry(now + Duration::from_millis(999)));
        assert!(policy.take_retry(now + seconds(1)));
        assert_eq!(policy.retries_started(), 1);
        assert!(!policy.take_retry(now + seconds(1)));
    }

    #[test]
    fn repeated_spawn_failures_back_off_then_exhaust_exactly_eight_attempts() {
        let mut now = Instant::now();
        let mut policy = RetryPolicy::default();
        policy.record_failure(now);
        for (attempt, delay) in [1, 2, 4, 8, 16, 30, 30, 30].into_iter().enumerate() {
            assert_eq!(policy.retry_after(now), Some(seconds(delay)));
            assert!(!policy.exhausted());
            now += seconds(delay);
            assert!(policy.take_retry(now));
            assert_eq!(policy.retries_started(), attempt + 1);
            policy.record_failure(now);
        }
        assert!(policy.exhausted());
        assert!(!policy.needs_health_probe());
        assert_eq!(policy.retry_after(now), None);
        assert!(!policy.take_retry(now + seconds(86_400)));
        policy.record_failure(now + seconds(86_400));
        assert!(policy.exhausted());
        assert_eq!(policy.retries_started(), 8);
    }

    #[test]
    fn repeated_failure_notifications_do_not_postpone_the_same_retry() {
        let now = Instant::now();
        let mut policy = RetryPolicy::default();
        policy.record_failure(now);
        policy.record_failure(now + Duration::from_millis(900));
        assert!(policy.take_retry(now + seconds(1)));
        policy.record_failure(now + seconds(1));
        policy.record_failure(now + seconds(2));
        assert_eq!(policy.retry_after(now + seconds(2)), Some(seconds(1)));
        assert!(policy.take_retry(now + seconds(3)));
    }

    #[test]
    fn immediate_exits_keep_the_retry_budget_across_successful_spawns() {
        let mut now = Instant::now();
        let mut policy = RetryPolicy::default();
        policy.record_spawn(now);
        policy.record_failure(now);
        for delay in RETRY_DELAYS {
            now += seconds(delay);
            assert!(policy.take_retry(now));
            policy.record_spawn(now);
            assert!(policy.needs_health_probe());
            policy.record_failure(now);
        }
        assert!(policy.exhausted());
        assert_eq!(policy.retries_started(), 8);
    }

    #[test]
    fn startup_without_health_times_out_at_fifteen_seconds() {
        let now = Instant::now();
        let mut policy = RetryPolicy::default();
        policy.record_spawn(now);
        assert_eq!(
            policy.observe_health(now, false),
            HealthObservation::Waiting
        );
        assert_eq!(
            policy.observe_health(now + seconds(14), false),
            HealthObservation::Waiting
        );
        assert_eq!(
            policy.observe_health(now + seconds(15), false),
            HealthObservation::TimedOut
        );
        policy.record_failure(now + seconds(15));
        assert_eq!(policy.retry_after(now + seconds(15)), Some(seconds(1)));
        assert_eq!(
            policy.observe_health(now + seconds(16), true),
            HealthObservation::Inactive
        );
    }

    #[test]
    fn slow_start_can_become_ready_without_resetting_budget() {
        let now = Instant::now();
        let mut policy = recovering(now);
        assert_eq!(
            policy.observe_health(now + seconds(15), false),
            HealthObservation::Waiting
        );
        assert_eq!(
            policy.observe_health(now + seconds(16), true),
            healthy(true, false)
        );
        assert_eq!(policy.retries_started(), 1);
        assert_eq!(
            policy.observe_health(now + seconds(17), true),
            healthy(false, false)
        );
    }

    #[test]
    fn short_healthy_period_does_not_reset_budget() {
        let now = Instant::now();
        let mut policy = recovering(now);
        assert_eq!(
            policy.observe_health(now + seconds(1), true),
            healthy(true, false)
        );
        assert_eq!(
            policy.observe_health(now + seconds(30), true),
            healthy(false, false)
        );
        policy.record_failure(now + seconds(30));
        assert_eq!(policy.retries_started(), 1);
        assert_eq!(policy.retry_after(now + seconds(30)), Some(seconds(2)));
    }

    #[test]
    fn thirty_seconds_of_health_resets_budget_once_and_restarts_backoff() {
        let now = Instant::now();
        let mut policy = recovering(now);
        assert_eq!(
            policy.observe_health(now + seconds(1), true),
            healthy(true, false)
        );
        assert_eq!(
            policy.observe_health(now + seconds(31), true),
            healthy(false, true)
        );
        assert_eq!(policy.retries_started(), 0);
        assert_eq!(
            policy.observe_health(now + seconds(32), true),
            healthy(false, false)
        );
        policy.record_failure(now + seconds(33));
        assert_eq!(policy.retry_after(now + seconds(33)), Some(seconds(1)));
    }

    #[test]
    fn intermittent_health_restarts_stability_window_and_outage_gets_its_own_deadline() {
        let now = Instant::now();
        let mut policy = recovering(now);
        assert_eq!(
            policy.observe_health(now + seconds(1), true),
            healthy(true, false)
        );
        assert_eq!(
            policy.observe_health(now + seconds(30), false),
            HealthObservation::Waiting
        );
        assert_eq!(
            policy.observe_health(now + seconds(31), true),
            healthy(true, false)
        );
        assert_eq!(
            policy.observe_health(now + seconds(60), true),
            healthy(false, false)
        );
        assert_eq!(policy.retries_started(), 1);
        assert_eq!(
            policy.observe_health(now + seconds(61), true),
            healthy(false, true)
        );
        assert_eq!(
            policy.observe_health(now + seconds(62), false),
            HealthObservation::Waiting
        );
        assert_eq!(
            policy.observe_health(now + seconds(76), false),
            HealthObservation::Waiting
        );
        assert_eq!(
            policy.observe_health(now + seconds(77), false),
            HealthObservation::TimedOut
        );
    }

    #[test]
    fn manual_retry_recovers_exhausted_budget_and_bypasses_a_pending_deadline() {
        let now = Instant::now();
        let mut policy = RetryPolicy {
            retries_started: 8,
            phase: Phase::Exhausted,
        };
        policy.manual_retry();
        assert!(!policy.exhausted());
        assert_eq!(policy.retries_started(), 0);
        assert_eq!(policy.retry_after(now), None);
        assert!(!policy.take_retry(now));
        policy.record_spawn(now);
        assert!(policy.needs_health_probe());
        policy.record_failure(now);
        assert_eq!(policy.retry_after(now), Some(seconds(1)));
        policy.manual_retry();
        assert_eq!(policy.retry_after(now), None);
        policy.record_failure(now);
        assert_eq!(policy.retry_after(now), Some(seconds(1)));
    }

    #[test]
    fn inactive_phases_cannot_reset_budget_from_an_unrelated_healthy_server() {
        let now = Instant::now();
        let mut policy = RetryPolicy::default();
        assert_eq!(
            policy.observe_health(now, true),
            HealthObservation::Inactive
        );
        policy.record_failure(now);
        assert_eq!(
            policy.observe_health(now + seconds(60), true),
            HealthObservation::Inactive
        );
        assert!(policy.take_retry(now + seconds(60)));
        assert_eq!(
            policy.observe_health(now + seconds(90), true),
            HealthObservation::Inactive
        );
        assert_eq!(policy.retries_started(), 1);
        assert_eq!(policy.retry_after(now), None);
    }
}


#[cfg(test)]
mod readiness_tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn an_in_flight_health_response_cannot_certify_a_replacement_generation() {
        let generation = Cell::new(4);
        let result = probe_current_generation(4, || { generation.set(5); true }, || Ok(generation.get())).unwrap();
        assert!(!result);
        assert!(probe_current_generation(5, || true, || Ok(generation.get())).unwrap());
    }
    #[test]
    fn stopping_during_a_health_probe_is_not_ready() {
        let stopped = Cell::new(false);
        let result = probe_current_generation(4, || { stopped.set(true); true }, || {
            if stopped.get() { Err("stopping".into()) } else { Ok(4) }
        });
        assert!(result.is_err());
    }
    #[test]
    fn repeated_task_isolation_joins_backoff_without_resetting_automatic_budget() {
        let mut now = Instant::now();
        let mut policy = RetryPolicy::default();
        policy.record_spawn(now);
        for delay in [1, 2, 4, 8, 16, 30, 30, 30] {
            policy.record_failure(now);
            for _ in 0..100 { policy.record_failure(now); }
            now += Duration::from_secs(delay);
            assert!(policy.take_retry(now));
            policy.record_spawn(now);
        }
        policy.record_failure(now);
        for _ in 0..100 { policy.record_failure(now); }
        assert!(policy.exhausted());
        assert_eq!(policy.retries_started(), 8);
        assert!(!policy.take_retry(now + Duration::from_secs(3600)));
    }
}


#[cfg(test)]
mod idle_probe_tests {
    use super::*;
    #[test]
    fn stable_health_reduces_http_probes_and_a_failure_restores_fast_observation() {
        let now = Instant::now();
        let mut policy = RetryPolicy::default();
        policy.record_spawn(now);
        assert_eq!(policy.health_probe_interval(), Duration::from_secs(1));
        policy.observe_health(now, true);
        policy.observe_health(now + STABLE_HEALTH, true);
        assert_eq!(policy.health_probe_interval(), Duration::from_secs(30));
        policy.observe_health(now + STABLE_HEALTH + Duration::from_secs(30), false);
        assert_eq!(policy.health_probe_interval(), Duration::from_secs(1));
    }
}
