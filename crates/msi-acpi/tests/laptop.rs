//! Behaviour of the feature layer against an emulated MS-15K1 EC.

#![allow(clippy::unwrap_used, reason = "tests")]

mod support;

use msi_acpi::{CURVE_POINTS, Error, FanCurve, Laptop, Method, Scenario};
use support::FakeEc;

fn laptop(ec: &FakeEc) -> Laptop<&FakeEc> {
    Laptop::new(ec).unwrap()
}

#[test]
fn reads_firmware_identity() {
    let ec = FakeEc::cyborg15();
    let info = laptop(&ec).info().clone();
    assert_eq!(info.wmi_version, (2, 8));
    assert_eq!(info.ec_firmware, "15K1IMS1.113");
    assert_eq!(info.ec_build, "0819202415:13:22");
}

#[test]
fn rejects_non_v2_interface() {
    let mut ec = FakeEc::cyborg15();
    ec.wmi_version = (3, 0);
    assert!(matches!(Laptop::new(&ec), Err(Error::UnsupportedInterface { major: 3, minor: 0 })));
}

#[test]
fn decodes_snapshot_from_real_dump() {
    let ec = FakeEc::cyborg15();
    let s = laptop(&ec).snapshot().unwrap();

    assert_eq!(s.cpu.temp_c, Some(0x42));
    assert_eq!(s.gpu.temp_c, None, "0 means the dGPU sensor is asleep");
    assert_eq!(s.cpu.rpm, 480_000 / 0x94);
    assert_eq!(s.gpu.rpm, 0);
    assert_eq!(s.cpu.duty, 0x36);

    assert_eq!(s.cpu.curve.temps, [0, 51, 58, 65, 73, 78]);
    assert_eq!(s.cpu.curve.speeds, [38, 40, 45, 54, 60, 68]);
    assert_eq!(s.cpu.curve.top, (83, 78));
    assert_eq!(s.gpu.curve.temps, [0, 43, 48, 53, 58, 65]);
    assert_eq!(s.gpu.curve.top, (70, 78));

    assert_eq!(s.scenario, Some(Scenario::Balanced));
    assert!(!s.advanced_fan);
    assert!(!s.cooler_boost);
    assert_eq!(s.battery_limit, None, "0x80 = limit bit with 0 % means no limit");
    assert_eq!(s.webcam, Some(true));
    assert!(s.win_fn_swapped);
    assert!(s.fn_lock, "0xD9 = 0x05: bit1 clear means Fn Lock engaged");
    assert_eq!(s.kbd_backlight, 1);
    assert_eq!(s.usb_share, Some(true));
}

#[test]
fn scenarios_round_trip_and_only_touch_the_silent_bit() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.put(0xD4, 0x8D); // advanced curve active
    for scenario in Scenario::ALL {
        l.set_scenario(scenario).unwrap();
        assert_eq!(l.snapshot().unwrap().scenario, Some(scenario));
        assert_eq!(ec.get(0xD4) & 0x80, 0x80, "{scenario:?} must keep the advanced bit");
    }
    l.set_scenario(Scenario::Silent).unwrap();
    assert_eq!((ec.get(0xD2), ec.get(0xD4)), (0xC1, 0x9D));
    l.set_scenario(Scenario::ExtremePerformance).unwrap();
    assert_eq!((ec.get(0xD2), ec.get(0xD4)), (0xC4, 0x8D));
}

#[test]
fn unknown_shift_value_has_no_scenario() {
    let ec = FakeEc::cyborg15();
    ec.put(0xD2, 0xC5);
    assert_eq!(laptop(&ec).snapshot().unwrap().scenario, None);
}

#[test]
fn fan_curves_are_normalised_and_keep_firmware_bytes() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.put(0xD4, 0x1D); // silent + auto
    l.set_fan_curves([30, 20, 50, 120, 60, 70], [40, 45, 50, 55, 60, 65]).unwrap();

    let s = l.snapshot().unwrap();
    assert_eq!(s.cpu.curve.speeds, [30, 30, 50, 78, 78, 78], "capped at the firmware top point");
    assert_eq!(s.gpu.curve.speeds, [40, 45, 50, 55, 60, 65]);
    assert_eq!(s.cpu.curve.top, (83, 78), "top point belongs to the firmware");
    assert_eq!(s.gpu.curve.top, (70, 78));
    assert_eq!(ec.get(0xD4), 0x9D, "advanced on, silent kept");

    l.set_auto_fan().unwrap();
    assert_eq!(ec.get(0xD4), 0x1D);
}

#[test]
fn cooler_boost_preserves_other_bits() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    l.set_cooler_boost(true).unwrap();
    assert_eq!(ec.get(0x98), 0x86);
    assert!(l.snapshot().unwrap().cooler_boost);
    l.set_cooler_boost(false).unwrap();
    assert_eq!(ec.get(0x98), 0x06);
}

#[test]
fn battery_limit_encoding_and_bounds() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    for (limit, raw) in [(Some(80), 0xD0), (Some(60), 0xBC), (Some(10), 0x8A), (None, 0xE4)] {
        l.set_battery_limit(limit).unwrap();
        assert_eq!(ec.get(0xD7), raw);
        assert_eq!(l.snapshot().unwrap().battery_limit, limit);
    }
    l.set_battery_limit(Some(100)).unwrap();
    assert_eq!(l.snapshot().unwrap().battery_limit, None, "100 % is no limit");
    for bad in [0, 9, 101] {
        assert!(matches!(l.set_battery_limit(Some(bad)), Err(Error::InvalidArgument(_))));
    }
    assert_eq!(ec.get(0xD7), 0xE4, "rejected values must not be written");
}

#[test]
fn keyboard_backlight_matches_msi_encoding() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.put(0xD3, 0x85);
    l.set_kbd_backlight(3).unwrap();
    assert_eq!(ec.get(0xD3), 0x83);
    l.set_kbd_backlight(0).unwrap();
    assert_eq!(ec.get(0xD3), 0x80);
    assert!(matches!(l.set_kbd_backlight(4), Err(Error::InvalidArgument(_))));
}

#[test]
fn toggles_flip_only_their_bit() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);

    l.set_win_fn_swap(false).unwrap();
    assert_eq!(ec.get(0xE8), 0x11);
    assert!(!l.snapshot().unwrap().win_fn_swapped);

    l.set_usb_share(false).unwrap();
    assert_eq!(ec.get(0xBF), 0x08);
    assert_eq!(l.snapshot().unwrap().usb_share, Some(false));

    l.set_webcam(false).unwrap();
    assert_eq!(ec.get(0x2E), 0x09);
    assert_eq!(l.snapshot().unwrap().webcam, Some(false));
}

#[test]
fn missing_capabilities_hide_features() {
    let ec = FakeEc::cyborg15();
    ec.put(0x2F, 0x00);
    ec.put(0xBF, 0x20);
    let s = laptop(&ec).snapshot().unwrap();
    assert_eq!(s.webcam, None);
    assert_eq!(s.usb_share, None);
}

#[test]
fn firmware_rejection_is_reported() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.reject.set(Some(Method::Set_Data));
    assert!(matches!(
        l.set_cooler_boost(true),
        Err(Error::Rejected { method: Method::Set_Data, index: 0x98 })
    ));
}

#[test]
fn normalisation_clamps_and_sorts() {
    assert_eq!(FanCurve::normalized([0, 101, 50, 255, 0, 99], 100), [0, 100, 100, 100, 100, 100]);
    assert_eq!(FanCurve::normalized([10, 90, 20, 30, 40, 50], 78), [10, 78, 78, 78, 78, 78]);
    let sorted = [10, 20, 30, 40, 50, 60];
    assert_eq!(FanCurve::normalized(sorted, 78), sorted);
}

#[test]
fn active_point_follows_band_bounds() {
    let ec = FakeEc::cyborg15();
    let curve = laptop(&ec).snapshot().unwrap().cpu.curve;
    assert_eq!(curve.active_point(20), 0);
    assert_eq!(curve.active_point(51), 1);
    assert_eq!(curve.active_point(77), 4);
    assert_eq!(curve.active_point(78), 5);
    assert_eq!(curve.active_point(83), CURVE_POINTS);
}
