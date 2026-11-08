//! Behaviour of the feature layer against an emulated EC: the MS-15K1 dump, and other models by
//! their EC firmware string.

mod support;

use msi_acpi::{CURVE_POINTS, Error, FanCurve, Laptop, Method, Scenario, Scenarios, Snapshot};
use support::FakeEc;

/// Prestige 14 A10SC, msi-ec `CONF_G1_0`: shift/fan/charge/backlight at `0xF2..=0xF4`/`0xEF`,
/// Win/Fn at `0xBF` bit4 (not inverted).
const PRESTIGE_14: &str = "14C1EMS1.012";
/// GE63 Raider 8RE, msi-ec `CONF_G1_10`: no charge limit, no keyboard backlight, no turbo shift
/// mode and no silent fan mode.
const GE63_RAIDER: &str = "16P5EMS1.103";

fn laptop(ec: &FakeEc) -> Laptop<&FakeEc> {
    Laptop::new(ec).expect("laptop")
}

fn snapshot(l: &Laptop<&FakeEc>) -> Snapshot {
    l.snapshot().expect("snapshot")
}

#[test]
fn reads_firmware_identity() {
    let ec = FakeEc::cyborg15();
    let info = laptop(&ec).info().clone();
    assert_eq!(info.wmi_version, (2, 8));
    assert_eq!(info.ec_firmware, "15K1IMS1.113");
    assert_eq!(info.ec_build, "0819202415:13:22");
    assert_eq!(info.ec_config, Some("CONF_G2_1"));
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
    let s = snapshot(&laptop(&ec));

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

    assert_eq!(s.scenarios, Scenarios::ALL);
    assert_eq!(s.scenario, Some(Scenario::Balanced));
    assert!(!s.advanced_fan);
    assert_eq!(s.cooler_boost, Some(false));
    assert_eq!(s.charge_limit, Some(100), "0x80 = limit bit with 0 % means no limit");
    assert_eq!(s.webcam, Some(true));
    assert_eq!(s.win_fn_swapped, Some(true));
    assert_eq!(s.fn_lock, Some(true), "0xD9 = 0x05: bit1 clear means Fn Lock engaged");
    assert_eq!(s.kbd_backlight, Some(1));
    assert_eq!(s.usb_share, Some(true));
}

#[test]
fn scenarios_round_trip_and_only_touch_the_silent_bit() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.put(0xD4, 0x8D); // advanced curve active
    for scenario in Scenario::ALL {
        l.set_scenario(scenario).expect("scenario");
        assert_eq!(snapshot(&l).scenario, Some(scenario));
        assert_eq!(ec.get(0xD4) & 0x80, 0x80, "{scenario:?} must keep the advanced bit");
    }
    l.set_scenario(Scenario::Silent).expect("silent");
    assert_eq!((ec.get(0xD2), ec.get(0xD4)), (0xC1, 0x9D));
    l.set_scenario(Scenario::ExtremePerformance).expect("extreme");
    assert_eq!((ec.get(0xD2), ec.get(0xD4)), (0xC4, 0x8D));
}

#[test]
fn unknown_shift_value_has_no_scenario() {
    let ec = FakeEc::cyborg15();
    ec.put(0xD2, 0xC5);
    assert_eq!(snapshot(&laptop(&ec)).scenario, None);
}

#[test]
fn fan_curves_are_normalised_and_keep_firmware_bytes() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.put(0xD4, 0x1D); // silent + auto
    l.set_fan_curves([30, 20, 50, 120, 60, 70], [40, 45, 50, 55, 60, 65]).expect("curves");

    let s = snapshot(&l);
    assert_eq!(s.cpu.curve.speeds, [30, 30, 50, 78, 78, 78], "capped at the firmware top point");
    assert_eq!(s.gpu.curve.speeds, [40, 45, 50, 55, 60, 65]);
    assert_eq!(s.cpu.curve.top, (83, 78), "top point belongs to the firmware");
    assert_eq!(s.gpu.curve.top, (70, 78));
    assert_eq!(ec.get(0xD4), 0x9D, "advanced on, silent kept");

    l.set_auto_fan().expect("auto fan");
    assert_eq!(ec.get(0xD4), 0x1D);
}

#[test]
fn cooler_boost_preserves_other_bits() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    l.set_cooler_boost(true).expect("on");
    assert_eq!(ec.get(0x98), 0x86);
    assert_eq!(snapshot(&l).cooler_boost, Some(true));
    l.set_cooler_boost(false).expect("off");
    assert_eq!(ec.get(0x98), 0x06);
}

#[test]
fn charge_limit_encoding_and_bounds() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    for (limit, raw) in [(80, 0xD0), (60, 0xBC), (10, 0x8A), (100, 0xE4)] {
        l.set_charge_limit(limit).expect("charge limit");
        assert_eq!(ec.get(0xD7), raw);
        assert_eq!(snapshot(&l).charge_limit, Some(limit));
    }
    for bad in [0, 9, 101] {
        assert!(matches!(l.set_charge_limit(bad), Err(Error::InvalidArgument(_))));
    }
    assert_eq!(ec.get(0xD7), 0xE4, "rejected values must not be written");
}

#[test]
fn keyboard_backlight_matches_msi_encoding() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);
    ec.put(0xD3, 0x85);
    l.set_kbd_backlight(3).expect("level 3");
    assert_eq!(ec.get(0xD3), 0x83);
    l.set_kbd_backlight(0).expect("off");
    assert_eq!(ec.get(0xD3), 0x80);
    assert!(matches!(l.set_kbd_backlight(4), Err(Error::InvalidArgument(_))));
}

#[test]
fn toggles_flip_only_their_bit() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);

    l.set_usb_share(false).expect("usb share");
    assert_eq!(ec.get(0xBF), 0x08);
    assert_eq!(snapshot(&l).usb_share, Some(false));

    l.set_webcam(false).expect("webcam");
    assert_eq!(ec.get(0x2E), 0x09);
    assert_eq!(snapshot(&l).webcam, Some(false));
}

#[test]
fn win_fn_bit_cleared_means_swapped_on_ms_15k1() {
    let ec = FakeEc::cyborg15();
    let l = laptop(&ec);

    l.set_win_fn_swap(false).expect("normal");
    assert_eq!(ec.get(0xE8), 0x11, "bit4 set, other bits kept");
    assert_eq!(snapshot(&l).win_fn_swapped, Some(false));

    l.set_win_fn_swap(true).expect("swapped");
    assert_eq!(ec.get(0xE8), 0x01);
    assert_eq!(snapshot(&l).win_fn_swapped, Some(true));
}

#[test]
fn missing_capabilities_hide_features() {
    let ec = FakeEc::cyborg15();
    ec.put(0x2F, 0x00);
    ec.put(0xBF, 0x20);
    let s = snapshot(&laptop(&ec));
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
fn listed_model_uses_its_own_registers() {
    let ec = FakeEc::with_firmware(PRESTIGE_14);
    let l = laptop(&ec);
    assert_eq!(l.info().ec_config, Some("CONF_G1_0"));
    ec.put(0xF2, 0xC1);
    ec.put(0xF4, 0x8D); // advanced curve active
    ec.put(0xF3, 0x85);
    ec.put(0xEF, 0x80);
    let ms15k1 = [0xD2, 0xD3, 0xD4, 0xD7, 0xE8].map(|addr| (addr, ec.get(addr)));

    assert_eq!(snapshot(&l).scenarios, Scenarios::ALL);
    l.set_scenario(Scenario::Silent).expect("silent");
    assert_eq!((ec.get(0xF2), ec.get(0xF4)), (0xC1, 0x9D), "silent bit set, advanced kept");
    assert_eq!(snapshot(&l).scenario, Some(Scenario::Silent));
    l.set_scenario(Scenario::ExtremePerformance).expect("extreme");
    assert_eq!((ec.get(0xF2), ec.get(0xF4)), (0xC4, 0x8D));

    l.set_auto_fan().expect("auto fan");
    assert_eq!(ec.get(0xF4), 0x0D);

    l.set_charge_limit(80).expect("charge limit");
    assert_eq!(ec.get(0xEF), 0xD0);
    assert_eq!(snapshot(&l).charge_limit, Some(80));

    l.set_kbd_backlight(2).expect("backlight");
    assert_eq!(ec.get(0xF3), 0x82);
    assert_eq!(snapshot(&l).kbd_backlight, Some(2));

    // Win/Fn shares 0xBF with bits this model does not use; they must survive.
    l.set_win_fn_swap(true).expect("swap");
    assert_eq!(ec.get(0xBF), 0x38, "bit4 set (not inverted), bits 3 and 5 kept");
    assert_eq!(snapshot(&l).win_fn_swapped, Some(true));

    for (addr, value) in ms15k1 {
        assert_eq!(ec.get(addr), value, "MS-15K1 register 0x{addr:02X} must not be touched");
    }
}

#[test]
fn features_a_model_lacks_are_hidden_and_never_written() {
    let ec = FakeEc::with_firmware(GE63_RAIDER);
    let l = laptop(&ec);
    ec.put(0xF2, 0xC1);
    ec.put(0xF4, 0x0C);

    let s = snapshot(&l);
    assert_eq!(s.scenarios.iter().collect::<Vec<_>>(), [Scenario::Balanced, Scenario::SuperBattery]);
    assert_eq!(s.scenario, Some(Scenario::Balanced));
    assert_eq!(s.charge_limit, None);
    assert_eq!(s.kbd_backlight, None);
    assert_eq!(s.usb_share, None, "0xBF bit5 is known only on the MS-15K1 family");
    assert_eq!(s.fn_lock, None);

    let before = *ec.ram.borrow();
    for result in [
        l.set_charge_limit(80),
        l.set_kbd_backlight(2),
        l.set_usb_share(false),
        l.set_scenario(Scenario::ExtremePerformance),
        l.set_scenario(Scenario::Silent),
    ] {
        assert!(matches!(result, Err(Error::Unsupported(_))), "{result:?}");
    }
    assert_eq!(*ec.ram.borrow(), before, "nothing was written");

    l.set_scenario(Scenario::SuperBattery).expect("eco");
    assert_eq!((ec.get(0xF2), ec.get(0xF4)), (0xC2, 0x0C), "no silent flag to touch");
    ec.put(0xF2, 0xC0);
    assert_eq!(snapshot(&l).scenario, None, "sport matches no scenario");
}

#[test]
fn unknown_firmware_falls_back_to_the_ms_15k1_layout() {
    let known = FakeEc::cyborg15();
    let unknown = FakeEc::with_firmware("1234EMS1.000");
    let l = laptop(&unknown);
    assert_eq!(l.info().ec_config, None, "reported as unrecognised");
    assert_eq!(snapshot(&l), snapshot(&laptop(&known)));

    l.set_cooler_boost(true).expect("cooler boost");
    l.set_charge_limit(80).expect("charge limit");
    l.set_scenario(Scenario::Silent).expect("silent");
    assert_eq!([0x98, 0xD7, 0xD2, 0xD4].map(|addr| unknown.get(addr)), [0x86, 0xD0, 0xC1, 0x1D]);
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
    let curve = snapshot(&laptop(&ec)).cpu.curve;
    assert_eq!(curve.active_point(20), 0);
    assert_eq!(curve.active_point(51), 1);
    assert_eq!(curve.active_point(77), 4);
    assert_eq!(curve.active_point(78), 5);
    assert_eq!(curve.active_point(83), CURVE_POINTS);
}
