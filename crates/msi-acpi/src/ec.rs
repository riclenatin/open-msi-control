//! Embedded-controller register layouts (`Get_Data`/`Set_Data` space of MSI WMI 2.x firmware).
//!
//! This is the only place that maps features to EC addresses. A [`Layout`] is resolved once per
//! laptop from its EC firmware string: firmware listed in the msi-ec table (`crate::models`) gets
//! its config's registers; anything else gets [`Layout::BUILT_IN`], the layout verified on a
//! Cyborg 15 A12VF (MS-15K1, EC `15K1IMS1.113`, msi-ec `CONF_G2_1`). See `docs/PROTOCOL.md`.

use std::fmt;

use crate::model::{Scenario, Scenarios};
use crate::models::{self, ModelConf, Modes};

/// msi-ec config of the verified laptop. Registers msi-ec does not describe (USB power share,
/// Fn Lock) are known only for this family, so other configs do not get them.
const VERIFIED_CONF: &str = "CONF_G2_1";

/// bit3 = USB power share supported, bit5 = powering USB while the laptop is off.
const USB_SHARE: Gated = Gated { switch: Bit::new(0xBF, 5), supported: Some(Bit::new(0xBF, 3)) };

/// Status flags; bit1 cleared = Fn Lock engaged. The EC toggles it on Fn+Esc; MSI Center only
/// reads it (other bits of this byte are owned by MSI's service), so this crate never writes it.
const FN_UNLOCKED: Bit = Bit::new(0xD9, 1);

/// Battery stop-charge register: bit7 = enabled, low 7 bits = stop-charging percentage.
pub(crate) const BATTERY_ENABLED: u8 = 1 << 7;
const BATTERY_PERCENT_MASK: u8 = 0x7F;

/// Keyboard backlight level lives in the low two bits.
pub(crate) const KBD_LEVEL_MASK: u8 = 0x03;
pub(crate) const KBD_MAX_LEVEL: u8 = 3;

/// Stop-charge percentage encoded in the battery register; 100 = no limit.
pub(crate) fn charge_limit(raw: u8) -> u8 {
    let percent = raw & BATTERY_PERCENT_MASK;
    // MSI writes 0x80 | 100 ("Best for Mobility"); 0x80 alone also means no limit.
    if raw & BATTERY_ENABLED != 0 && (1..100).contains(&percent) { percent } else { 100 }
}

/// One bit of one EC register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bit {
    pub address: u8,
    pub mask: u8,
}

impl Bit {
    const fn new(address: u8, bit: u8) -> Self {
        Self { address, mask: 1 << bit }
    }

    pub(crate) fn is_set(self, value: u8) -> bool {
        value & self.mask != 0
    }
}

impl From<models::Bit> for Bit {
    fn from(b: models::Bit) -> Self {
        Self::new(b.address, b.bit)
    }
}

/// A flag of the fan-mode register (silent, advanced).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flag {
    /// One bit, set or cleared by read-modify-write; the byte's other bits are kept.
    Bit(u8),
    /// msi-ec's whole-byte mode values: `active` while the flag is on, `auto` to leave it.
    Value { active: u8, auto: u8 },
}

impl Flag {
    /// From msi-ec's `auto` and mode values: a single bit when the mode is `auto` plus one bit
    /// (as on MS-15K1), whole-byte values otherwise.
    fn derive(auto: u8, active: u8) -> Self {
        let diff = auto ^ active;
        if diff.is_power_of_two() && active & diff != 0 {
            Self::Bit(diff)
        } else {
            Self::Value { active, auto }
        }
    }

    pub(crate) fn is_on(self, value: u8) -> bool {
        match self {
            Self::Bit(mask) => value & mask != 0,
            Self::Value { active, .. } => value == active,
        }
    }

    /// The register value after switching the flag on or off.
    pub(crate) fn apply(self, value: u8, on: bool) -> u8 {
        match self {
            Self::Bit(mask) if on => value | mask,
            Self::Bit(mask) => value & !mask,
            Self::Value { active, .. } if on => active,
            Self::Value { active, auto } if value == active => auto,
            Self::Value { .. } => value,
        }
    }
}

/// Shift (performance) mode register and msi-ec's whole-byte values the scenarios use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shift {
    pub address: u8,
    pub turbo: Option<u8>,
    pub comfort: Option<u8>,
    pub eco: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FanMode {
    pub address: u8,
    pub silent: Option<Flag>,
    /// User fan curve (`Set_Fan`) instead of the firmware's.
    pub advanced: Option<Flag>,
}

/// A switch that exists only while its capability bit is set (when there is one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Gated {
    pub switch: Bit,
    pub supported: Option<Bit>,
}

/// Win/Fn swap: swapped while the bit XOR `invert` is 1 (msi-ec: "Win key on the left"; MSI's
/// default puts Win right of Fn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WinFn {
    pub bit: Bit,
    pub invert: bool,
}

impl WinFn {
    pub(crate) fn swapped(self, value: u8) -> bool {
        self.bit.is_set(value) != self.invert
    }

    /// Whether the bit must be set for `swapped`.
    pub(crate) fn bit_for(self, swapped: bool) -> bool {
        swapped != self.invert
    }
}

/// Keyboard backlight: level `0..=3` in the low two bits; a write keeps only the `keep` bits
/// (MSI writes `(v & 0x80) | level`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Backlight {
    pub address: u8,
    pub keep: u8,
}

/// Where one laptop keeps each feature; `None` = the model does not have it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub shift: Option<Shift>,
    pub fan_mode: Option<FanMode>,
    /// Set = fans at maximum.
    pub cooler_boost: Option<Bit>,
    /// Stop-charge register, see [`charge_limit`].
    pub battery: Option<u8>,
    pub webcam: Option<Gated>,
    pub usb_share: Option<Gated>,
    pub win_fn: Option<WinFn>,
    /// Set = Fn Lock off. Read-only.
    pub fn_unlocked: Option<Bit>,
    pub kbd_backlight: Option<Backlight>,
}

impl Layout {
    /// The MS-15K1 layout, verified against MSI Center; used for firmware msi-ec does not list.
    pub(crate) const BUILT_IN: Self = Self {
        shift: Some(Shift { address: 0xD2, turbo: Some(0xC4), comfort: Some(0xC1), eco: Some(0xC2) }),
        fan_mode: Some(FanMode {
            address: 0xD4,
            silent: Some(Flag::Bit(1 << 4)),
            advanced: Some(Flag::Bit(1 << 7)),
        }),
        cooler_boost: Some(Bit::new(0x98, 7)),
        battery: Some(0xD7),
        webcam: Some(Gated { switch: Bit::new(0x2E, 1), supported: Some(Bit::new(0x2F, 1)) }),
        usb_share: Some(USB_SHARE),
        win_fn: Some(WinFn { bit: Bit::new(0xE8, 4), invert: true }),
        fn_unlocked: Some(FN_UNLOCKED),
        kbd_backlight: Some(Backlight { address: 0xD3, keep: 1 << 7 }),
    };

    /// The layout for `firmware` and the msi-ec config it comes from (`None` = built-in).
    pub(crate) fn resolve(firmware: &str) -> (Option<&'static str>, Self) {
        match models::lookup(firmware) {
            Some(conf) => (Some(conf.name), Self::from_conf(conf)),
            None => (None, Self::BUILT_IN),
        }
    }

    fn from_conf(conf: &ModelConf) -> Self {
        let verified = conf.name == VERIFIED_CONF;
        let mode = |modes: Modes, name: &str| modes.modes.iter().find(|m| m.0 == name).map(|m| m.1);
        Self {
            shift: conf.shift_mode.map(|m| Shift {
                address: m.address,
                turbo: mode(m, "turbo"),
                comfort: mode(m, "comfort"),
                eco: mode(m, "eco"),
            }),
            fan_mode: conf.fan_mode.map(|m| {
                let flag = |name| Some(Flag::derive(mode(m, "auto")?, mode(m, name)?));
                FanMode { address: m.address, silent: flag("silent"), advanced: flag("advanced") }
            }),
            cooler_boost: conf.cooler_boost.map(Bit::from),
            battery: conf.charge_control,
            webcam: conf
                .webcam
                .map(|b| Gated { switch: b.into(), supported: conf.webcam_block.map(Bit::from) }),
            usb_share: verified.then_some(USB_SHARE),
            win_fn: conf.fn_win_swap.map(|w| WinFn { bit: Bit::new(w.address, w.bit), invert: w.invert }),
            fn_unlocked: verified.then_some(FN_UNLOCKED),
            // Only the encoding implemented here: four levels, read from the low two bits.
            kbd_backlight: conf.kbd_backlight.and_then(|k| {
                let address = k.state_address?;
                (k.max_state == KBD_MAX_LEVEL).then_some(Backlight { address, keep: k.state_base })
            }),
        }
    }

    /// `(address, value)` the shift register takes for `scenario`; `None` if this layout cannot
    /// express it (msi-ec lists no such mode, or Silent without a silent fan mode).
    pub(crate) fn shift_for(&self, scenario: Scenario) -> Option<(u8, u8)> {
        let shift = self.shift?;
        let value = match scenario {
            Scenario::ExtremePerformance => shift.turbo,
            Scenario::Balanced => shift.comfort,
            Scenario::Silent => self.silent().and(shift.comfort),
            Scenario::SuperBattery => shift.eco,
        }?;
        Some((shift.address, value))
    }

    pub(crate) fn scenarios(&self) -> Scenarios {
        Scenario::ALL.into_iter().filter(|&s| self.shift_for(s).is_some()).collect()
    }

    /// The scenario that writes these register values; `None` for any other combination.
    pub(crate) fn scenario(&self, shift: u8, fan_mode: Option<u8>) -> Option<Scenario> {
        let modes = self.shift?;
        let silent = self.silent().zip(fan_mode).is_some_and(|(flag, v)| flag.is_on(v));
        let shift = Some(shift);
        if shift == modes.turbo {
            Some(Scenario::ExtremePerformance)
        } else if shift == modes.comfort {
            Some(if silent { Scenario::Silent } else { Scenario::Balanced })
        } else if shift == modes.eco {
            Some(Scenario::SuperBattery)
        } else {
            None
        }
    }

    fn silent(&self) -> Option<Flag> {
        self.fan_mode?.silent
    }
}

impl fmt::Display for Bit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:02X} bit {}", self.address, self.mask.trailing_zeros())
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Bit(mask) => write!(f, "bit {}", mask.trailing_zeros()),
            Self::Value { active, auto } => write!(f, "0x{active:02X} (auto 0x{auto:02X})"),
        }
    }
}

/// One line per feature, for `msi-probe`.
impl fmt::Display for Layout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn line(f: &mut fmt::Formatter<'_>, name: &str, value: Option<String>) -> fmt::Result {
            writeln!(f, "  {name:<14}{}", value.as_deref().unwrap_or("unsupported"))
        }
        let hex = |v: Option<u8>| v.map_or_else(|| "-".to_owned(), |v| format!("0x{v:02X}"));
        let flag = |v: Option<Flag>| v.map_or_else(|| "-".to_owned(), |v| v.to_string());
        let gated = |g: Gated| match g.supported {
            Some(cap) => format!("{}, supported while {cap} is set", g.switch),
            None => g.switch.to_string(),
        };

        line(
            f,
            "shift mode",
            self.shift.map(|s| {
                let (turbo, comfort, eco) = (hex(s.turbo), hex(s.comfort), hex(s.eco));
                format!("0x{:02X}: turbo {turbo}, comfort {comfort}, eco {eco}", s.address)
            }),
        )?;
        line(
            f,
            "fan mode",
            self.fan_mode.map(|m| {
                let (silent, advanced) = (flag(m.silent), flag(m.advanced));
                format!("0x{:02X}: silent {silent}, advanced {advanced}", m.address)
            }),
        )?;
        line(f, "cooler boost", self.cooler_boost.map(|b| b.to_string()))?;
        line(f, "charge limit", self.battery.map(|a| format!("0x{a:02X} = 0x80 | percent")))?;
        line(f, "webcam", self.webcam.map(gated))?;
        line(f, "usb share", self.usb_share.map(gated))?;
        line(
            f,
            "win/fn swap",
            self.win_fn
                .map(|w| format!("{}, swapped while {}", w.bit, if w.invert { "clear" } else { "set" })),
        )?;
        line(f, "fn lock", self.fn_unlocked.map(|b| format!("{b}, engaged while clear (read-only)")))?;
        line(
            f,
            "kbd backlight",
            self.kbd_backlight.map(|k| format!("0x{:02X}: level 0..=3, keeps 0x{:02X}", k.address, k.keep)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_firmware_resolves_to_the_built_in_registers() {
        assert_eq!(Layout::resolve("15K1IMS1.113"), (Some("CONF_G2_1"), Layout::BUILT_IN));
    }

    #[test]
    fn fan_modes_one_bit_apart_are_switched_bitwise() {
        let silent = Flag::derive(0x0D, 0x1D);
        assert_eq!(silent, Flag::Bit(0x10));
        assert_eq!(silent.apply(0x8D, true), 0x9D, "other bits are kept");
        assert_eq!(silent.apply(0x9D, false), 0x8D);
    }

    #[test]
    fn other_fan_modes_are_written_as_msi_ec_values() {
        let silent = Flag::derive(0x0D, 0x2E);
        assert_eq!(silent, Flag::Value { active: 0x2E, auto: 0x0D });
        assert!(silent.is_on(0x2E));
        assert!(!silent.is_on(0x2F));
        assert_eq!(silent.apply(0x4D, true), 0x2E);
        assert_eq!(silent.apply(0x2E, false), 0x0D, "leaving the mode returns to auto");
        assert_eq!(silent.apply(0x8D, false), 0x8D, "another mode is left alone");
    }
}
