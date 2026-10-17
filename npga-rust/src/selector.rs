//! Node selection and automatic switching (the deterministic version of what
//! commercial accelerators sell as "intelligent path selection").
//!
//! Two independent decisions live here, both pure functions so they are unit
//! testable without touching the network:
//! - [`select_best`]: which candidate to use when connecting.
//! - [`decide_switch`]: whether the running connection should move to another
//!   candidate, given latency samples, cooldown, and the user's risk tolerance.

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SelectorConfig {
    /// Automatic selection is on.
    pub enabled: bool,
    /// Allow switching a working connection on latency degradation (not only
    /// on failure). Games treat an IP change as a reconnect, so this is opt-in.
    pub switch_on_degradation: bool,
    /// Latency above this is considered degraded when degradation switching is on.
    pub degrade_ms: u128,
    /// Minimum time between switches; prevents flapping.
    pub cooldown: Duration,
    /// How often a running connection is re-probed.
    pub probe_interval: Duration,
}

impl Default for SelectorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            switch_on_degradation: false,
            degrade_ms: 250,
            cooldown: Duration::from_secs(30),
            probe_interval: Duration::from_secs(20),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Keep the current node.
    Keep,
    /// Move to the given node index.
    Switch {
        to: usize,
        millis: u128,
        reason: String,
    },
    /// A switch is due but blocked; carries the explanation for the log.
    Blocked { reason: String },
}

/// Picks the lowest-latency node from probe results. Failed probes (`None`) are
/// ignored; ties keep the earlier candidate. Returns `None` when every probe
/// failed or the list is empty.
pub fn select_best(results: &[(usize, Option<u128>)]) -> Option<(usize, u128)> {
    results
        .iter()
        .filter_map(|(index, millis)| millis.map(|m| (*index, m)))
        .min_by_key(|(_, millis)| *millis)
}

/// Decides whether a live connection should switch nodes.
///
/// `results` carries every candidate's latest latency (current node included).
/// `since_last_switch` gates flapping; `None` means no switch happened yet.
pub fn decide_switch(
    current: usize,
    current_ms: Option<u128>,
    results: &[(usize, Option<u128>)],
    config: &SelectorConfig,
    since_last_switch: Option<Duration>,
) -> Outcome {
    if !config.enabled {
        return Outcome::Keep;
    }
    let Some((best, best_ms)) = select_best(results) else {
        if current_ms.is_none() {
            return Outcome::Blocked {
                reason: "当前节点与所有备选均探测失败，保持当前连接等待恢复".into(),
            };
        }
        return Outcome::Blocked {
            reason: "所有候选节点探测失败，保持当前节点".into(),
        };
    };
    let current_failed = current_ms.is_none();
    // The current node is unusable: switch regardless of the cooldown, a dead
    // connection is worse than an IP change.
    if current_failed {
        if let Some(elapsed) = since_last_switch
            && elapsed < config.cooldown
        {
            return Outcome::Blocked {
                reason: format!(
                    "当前节点探测失败，但距上次切换仅 {} 秒（冷却中），暂不切换",
                    elapsed.as_secs()
                ),
            };
        }
        if best == current {
            return Outcome::Blocked {
                reason: "当前节点探测失败且没有可用备选".into(),
            };
        }
        return Outcome::Switch {
            to: best,
            millis: best_ms,
            reason: "当前节点不可用".into(),
        };
    }
    let current_ms = current_ms.unwrap();
    let degraded = current_ms > config.degrade_ms;
    if best == current {
        return Outcome::Keep;
    }
    if !config.switch_on_degradation && !degraded && current_ms <= config.degrade_ms {
        // Healthy and not allowed to switch for latency alone.
        return Outcome::Keep;
    }
    if degraded && !config.switch_on_degradation {
        return Outcome::Blocked {
            reason: format!(
                "当前延迟 {current_ms}ms 已超过阈值 {}ms，但未开启「延迟劣化时切换」",
                config.degrade_ms
            ),
        };
    }
    if let Some(elapsed) = since_last_switch
        && elapsed < config.cooldown
    {
        return Outcome::Blocked {
            reason: format!(
                "存在更优节点（{best_ms}ms），但距上次切换仅 {} 秒（冷却中）",
                elapsed.as_secs()
            ),
        };
    }
    Outcome::Switch {
        to: best,
        millis: best_ms,
        reason: if degraded {
            format!("当前延迟 {current_ms}ms 劣化，切换到更优节点")
        } else {
            format!("发现更优节点（{best_ms}ms < {current_ms}ms）")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> SelectorConfig {
        SelectorConfig {
            enabled: true,
            switch_on_degradation: true,
            degrade_ms: 200,
            cooldown: Duration::from_secs(30),
            probe_interval: Duration::from_secs(20),
        }
    }

    #[test]
    fn best_pick_ignores_failures_and_keeps_earliest_tie() {
        let results = vec![
            (0, None),
            (1, Some(120)),
            (2, Some(80)),
            (3, Some(80)),
            (4, Some(400)),
        ];
        assert_eq!(select_best(&results), Some((2, 80)));
        assert_eq!(select_best(&[(0, None), (1, None)]), None);
        assert_eq!(select_best(&[]), None);
    }

    #[test]
    fn disabled_selector_never_switches() {
        let mut cfg = config();
        cfg.enabled = false;
        let results = vec![(0, Some(500)), (1, Some(50))];
        assert_eq!(
            decide_switch(0, Some(500), &results, &cfg, None),
            Outcome::Keep
        );
    }

    #[test]
    fn healthy_connection_is_left_alone_without_degradation_switching() {
        let mut cfg = config();
        cfg.switch_on_degradation = false;
        let results = vec![(0, Some(90)), (1, Some(50))];
        assert_eq!(
            decide_switch(0, Some(90), &results, &cfg, Some(Duration::from_secs(120))),
            Outcome::Keep
        );
    }

    #[test]
    fn degraded_connection_switches_only_when_allowed() {
        let mut cfg = config();
        cfg.switch_on_degradation = true;
        let results = vec![(0, Some(600)), (1, Some(60))];
        assert_eq!(
            decide_switch(0, Some(600), &results, &cfg, Some(Duration::from_secs(60))),
            Outcome::Switch {
                to: 1,
                millis: 60,
                reason: "当前延迟 600ms 劣化，切换到更优节点".into()
            }
        );

        cfg.switch_on_degradation = false;
        match decide_switch(0, Some(600), &results, &cfg, Some(Duration::from_secs(60))) {
            Outcome::Blocked { reason } => assert!(reason.contains("延迟劣化时切换")),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn failed_current_node_switches_even_during_cooldown_gap() {
        let cfg = config();
        let results = vec![(0, None), (1, Some(60))];
        // No switch happened yet: recover immediately.
        assert_eq!(
            decide_switch(0, None, &results, &cfg, None),
            Outcome::Switch {
                to: 1,
                millis: 60,
                reason: "当前节点不可用".into()
            }
        );
        // A switch 5 seconds ago: still cooling down, stay put.
        match decide_switch(0, None, &results, &cfg, Some(Duration::from_secs(5))) {
            Outcome::Blocked { reason } => assert!(reason.contains("冷却中")),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn failed_current_node_without_alternative_is_reported() {
        let cfg = config();
        let results = vec![(0, None), (1, None)];
        match decide_switch(0, None, &results, &cfg, Some(Duration::from_secs(600))) {
            Outcome::Blocked { reason } => {
                assert!(reason.contains("探测失败"), "原因应说明探测失败: {reason}");
                assert!(reason.contains("备选"), "应说明备选也不可用: {reason}");
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
        // Current healthy but every probe failed: still blocked, different wording.
        match decide_switch(0, Some(50), &results, &cfg, Some(Duration::from_secs(600))) {
            Outcome::Blocked { reason } => assert!(reason.contains("所有候选节点探测失败")),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn better_node_during_cooldown_is_blocked_not_switched() {
        let cfg = config();
        let results = vec![(0, Some(300)), (1, Some(60))];
        match decide_switch(0, Some(300), &results, &cfg, Some(Duration::from_secs(10))) {
            Outcome::Blocked { reason } => assert!(reason.contains("冷却中")),
            other => panic!("expected Blocked, got {other:?}"),
        }
        // After the cooldown the same situation switches.
        assert!(matches!(
            decide_switch(0, Some(300), &results, &cfg, Some(Duration::from_secs(45))),
            Outcome::Switch { to: 1, .. }
        ));
    }
}
