//! Profiles: the state the user wants, and the EC writes needed to get there.

use msi_acpi::{CURVE_POINTS, FanCurve, Scenario, Snapshot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum FanProfile {
    #[default]
    Auto,
    /// Speeds as the EC stores them (already normalised and capped).
    Custom { cpu: [u8; CURVE_POINTS], gpu: [u8; CURVE_POINTS] },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// `None` when the firmware was in a mode no scenario matches; left alone then.
    pub scenario: Option<Scenario>,
    pub fan: FanProfile,
    /// Ignored on models without Cooler Boost.
    pub cooler_boost: bool,
}

impl Profile {
    /// The profile that describes what the hardware is doing right now.
    pub fn capture(s: &Snapshot) -> Self {
        Self {
            scenario: s.scenario,
            fan: if s.advanced_fan {
                FanProfile::Custom { cpu: s.cpu.curve.speeds, gpu: s.gpu.curve.speeds }
            } else {
                FanProfile::Auto
            },
            cooler_boost: s.cooler_boost.unwrap_or(false),
        }
    }
}

/// One EC change needed to reach a profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Scenario(Scenario),
    AutoFan,
    Curves {
        cpu: [u8; CURVE_POINTS],
        gpu: [u8; CURVE_POINTS],
    },
    CoolerBoost(bool),
    /// 100 = no limit.
    ChargeLimit(u8),
}

/// Writes that bring `s` to `profile` (+ `charge_limit`), in a safe order; empty if in sync.
/// Features the model lacks are never planned.
pub fn plan(s: &Snapshot, profile: &Profile, charge_limit: Option<u8>) -> Vec<Op> {
    let mut ops = Vec::new();
    if let Some(sc) = profile.scenario
        && s.scenarios.contains(sc)
        && s.scenario != Some(sc)
    {
        ops.push(Op::Scenario(sc));
    }
    match profile.fan {
        FanProfile::Auto if s.advanced_fan => ops.push(Op::AutoFan),
        FanProfile::Auto => {}
        FanProfile::Custom { cpu, gpu } => {
            // Compare against what the EC would store, so an unreachable request (above the
            // firmware's top point) does not cause endless rewrites.
            let cpu = FanCurve::normalized(cpu, s.cpu.curve.top.1);
            let gpu = FanCurve::normalized(gpu, s.gpu.curve.top.1);
            if !s.advanced_fan || s.cpu.curve.speeds != cpu || s.gpu.curve.speeds != gpu {
                ops.push(Op::Curves { cpu, gpu });
            }
        }
    }
    if let Some(on) = s.cooler_boost
        && on != profile.cooler_boost
    {
        ops.push(Op::CoolerBoost(profile.cooler_boost));
    }
    if let (Some(limit), Some(current)) = (charge_limit, s.charge_limit)
        && current != limit
    {
        ops.push(Op::ChargeLimit(limit));
    }
    ops
}

#[cfg(test)]
mod tests {
    use msi_acpi::{FanStatus, Scenarios, Snapshot};

    use super::*;

    fn snapshot() -> Snapshot {
        let fan = |speeds| FanStatus {
            temp_c: Some(60),
            rpm: 3000,
            duty: 45,
            curve: FanCurve { temps: [0, 51, 58, 65, 73, 78], speeds, top: (83, 78) },
        };
        Snapshot {
            cpu: fan([38, 40, 45, 54, 60, 68]),
            gpu: fan([38, 40, 45, 54, 60, 68]),
            shift_raw: Some(0xC1),
            fan_mode_raw: Some(0x0D),
            scenario: Some(Scenario::Balanced),
            scenarios: Scenarios::ALL,
            advanced_fan: false,
            cooler_boost: Some(false),
            charge_limit: Some(100),
            webcam: Some(true),
            win_fn_swapped: Some(false),
            kbd_backlight: Some(1),
            fn_lock: Some(false),
            usb_share: Some(true),
        }
    }

    #[test]
    fn captured_profile_is_in_sync() {
        let s = snapshot();
        assert!(plan(&s, &Profile::capture(&s), Some(100)).is_empty());
    }

    #[test]
    fn restores_what_msi_center_overwrote() {
        let mut s = snapshot();
        let wanted = Profile {
            scenario: Some(Scenario::Silent),
            fan: FanProfile::Custom { cpu: [30, 35, 40, 50, 60, 70], gpu: [30, 35, 40, 50, 60, 70] },
            cooler_boost: false,
        };
        s.cooler_boost = Some(true);
        s.charge_limit = Some(100);
        assert_eq!(
            plan(&s, &wanted, Some(80)),
            vec![
                Op::Scenario(Scenario::Silent),
                Op::Curves { cpu: [30, 35, 40, 50, 60, 70], gpu: [30, 35, 40, 50, 60, 70] },
                Op::CoolerBoost(false),
                Op::ChargeLimit(80),
            ]
        );
    }

    #[test]
    fn unreachable_curve_is_compared_after_capping() {
        let mut s = snapshot();
        s.advanced_fan = true;
        s.cpu.curve.speeds = [40, 50, 60, 78, 78, 78];
        s.gpu.curve.speeds = s.cpu.curve.speeds;
        let wanted = Profile {
            scenario: Some(Scenario::Balanced),
            fan: FanProfile::Custom { cpu: [40, 50, 60, 90, 95, 100], gpu: [40, 50, 60, 90, 95, 100] },
            cooler_boost: false,
        };
        assert!(plan(&s, &wanted, None).is_empty(), "EC already holds the capped curve");
    }

    #[test]
    fn unknown_scenario_and_unset_limit_are_left_alone() {
        let mut s = snapshot();
        s.scenario = None;
        s.charge_limit = Some(60);
        let wanted = Profile { scenario: None, ..Profile::capture(&snapshot()) };
        assert!(plan(&s, &wanted, None).is_empty());
    }

    #[test]
    fn auto_mode_turns_the_user_curve_off() {
        let mut s = snapshot();
        s.advanced_fan = true;
        assert_eq!(plan(&s, &Profile::capture(&snapshot()), None), vec![Op::AutoFan]);
    }

    #[test]
    fn features_the_model_lacks_are_never_planned() {
        let mut s = snapshot();
        s.scenarios = [Scenario::Balanced, Scenario::SuperBattery].into_iter().collect();
        s.cooler_boost = None;
        s.charge_limit = None;
        let wanted = Profile { scenario: Some(Scenario::Silent), fan: FanProfile::Auto, cooler_boost: true };
        assert!(plan(&s, &wanted, Some(80)).is_empty());
    }

    #[test]
    fn profiles_round_trip_through_json() {
        let p = Profile {
            scenario: Some(Scenario::ExtremePerformance),
            fan: FanProfile::Custom { cpu: [1, 2, 3, 4, 5, 6], gpu: [6, 5, 4, 3, 2, 1] },
            cooler_boost: true,
        };
        let json = serde_json::to_string(&p).unwrap_or_default();
        assert!(json.contains(r#""scenario":"extreme_performance""#), "{json}");
        assert_eq!(serde_json::from_str::<Profile>(&json).ok(), Some(p));
    }
}
