//! Discrete observations, not equal time buckets or an uptime percentage.
use serde::{Deserialize, Serialize};

pub const HISTORY_LIMIT: usize = 48;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub checked_at: String,
    /// Server-formatted UTC+09:00, identical for SSR and hydration.
    pub checked_at_kst: String,
    pub alive: bool,
    pub response_ms: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Responding,
    Slow,
    Unreachable,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Responding => "응답 확인",
            Self::Slow => "느린 응답",
            Self::Unreachable => "응답 없음",
        }
    }
    pub fn class(self) -> &'static str {
        match self {
            Self::Responding => "responding",
            Self::Slow => "slow",
            Self::Unreachable => "unreachable",
        }
    }
}
impl Check {
    pub fn status(&self) -> Status {
        if !self.alive {
            Status::Unreachable
        } else if self.response_ms.is_some_and(|ms| ms >= 2000) {
            Status::Slow
        } else {
            Status::Responding
        }
    }
    pub fn response(&self) -> String {
        self.response_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "측정값 없음".into())
    }
    pub fn description(&self) -> String {
        format!(
            "{} · {} · {}",
            self.checked_at_kst,
            self.status().label(),
            self.response()
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub preview: bool,
    /// Oldest to newest, with a deterministic tie order; at most HISTORY_LIMIT.
    pub checks: Vec<Check>,
}
impl History {
    pub fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status() == status).count()
    }
    pub fn summary(&self) -> String {
        if self.checks.is_empty() {
            return "아직 확인 기록이 없어요.".into();
        }
        let missing = self.count(Status::Unreachable);
        if missing == 0 {
            format!("최근 {}회 모두 응답을 확인했어요.", self.checks.len())
        } else {
            format!(
                "최근 {}회 중 {}회 응답이 없었어요.",
                self.checks.len(),
                missing
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(alive: bool, ms: Option<i32>) -> Check {
        Check {
            checked_at: "2026-09-14T00:00:00+00:00".into(),
            checked_at_kst: "2026-09-14 09:00:00".into(),
            alive,
            response_ms: ms,
        }
    }
    #[test]
    fn health_history_keeps_phoenix_two_second_threshold_and_unknown_latency() {
        assert_eq!(check(true, Some(1999)).status(), Status::Responding);
        assert_eq!(check(true, Some(2000)).status(), Status::Slow);
        assert_eq!(check(false, Some(1)).status(), Status::Unreachable);
        assert_eq!(check(true, None).status(), Status::Responding);
        assert_eq!(check(true, None).response(), "측정값 없음");
        assert_eq!(check(true, Some(0)).response(), "0 ms");
    }
    #[test]
    fn health_history_empty_is_not_an_outage_or_perfect_uptime() {
        assert_eq!(History::default().summary(), "아직 확인 기록이 없어요.");
        let history = History {
            preview: false,
            checks: vec![
                check(true, None),
                check(false, None),
                check(true, Some(2000)),
            ],
        };
        assert_eq!(history.summary(), "최근 3회 중 1회 응답이 없었어요.");
        assert_eq!(history.count(Status::Responding), 1);
        assert_eq!(history.count(Status::Slow), 1);
        assert!(!history.summary().contains('%'));
    }
}
