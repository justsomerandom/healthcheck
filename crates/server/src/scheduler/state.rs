use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use healthcheck_core::Check;
use uuid::Uuid;

#[derive(Debug)]
pub(super) struct SchedulerState {
    checks: HashMap<Uuid, ScheduledCheck>,
}

impl SchedulerState {
    pub(super) fn new() -> Self {
        Self {
            checks: HashMap::new(),
        }
    }

    pub(super) fn register(&mut self, check: Check, now: Instant) {
        self.checks
            .insert(check.id, ScheduledCheck::new(check, now));
    }

    pub(super) fn claim_due_checks(&mut self, now: Instant) -> Vec<Check> {
        let mut due = Vec::new();

        for scheduled in self.checks.values_mut() {
            if !scheduled.check.enabled || scheduled.running || scheduled.next_due > now {
                continue;
            }

            scheduled.running = true;
            scheduled.last_started = Some(now);
            due.push(scheduled.check.clone());
        }

        due
    }

    pub(super) fn complete(
        &mut self,
        check_id: Uuid,
        completed_at: Instant,
        interval_unit: Duration,
    ) {
        let Some(scheduled) = self.checks.get_mut(&check_id) else {
            return;
        };

        scheduled.running = false;
        scheduled.last_completed = Some(completed_at);
        scheduled.next_due =
            completed_at + interval_duration(scheduled.check.interval_seconds, interval_unit);
    }
}

#[derive(Debug)]
struct ScheduledCheck {
    check: Check,
    next_due: Instant,
    running: bool,
    last_started: Option<Instant>,
    last_completed: Option<Instant>,
}

impl ScheduledCheck {
    fn new(check: Check, next_due: Instant) -> Self {
        Self {
            check,
            next_due,
            running: false,
            last_started: None,
            last_completed: None,
        }
    }
}

fn interval_duration(interval_seconds: u64, interval_unit: Duration) -> Duration {
    let units = u32::try_from(interval_seconds).unwrap_or(u32::MAX);
    interval_unit.checked_mul(units).unwrap_or(Duration::MAX)
}
