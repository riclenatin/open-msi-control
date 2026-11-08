use crate::ec;

/// Editable points per fan curve. MSI Center edits EC speeds 0..=5 and leaves the topmost
/// (seventh) point to the firmware; so does this crate.
pub const CURVE_POINTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fan {
    Cpu = 1,
    Gpu = 2,
}

/// The user scenarios MSI Center offers on models without TDP control (e.g. MS-15K1).
///
/// MSI Center keeps the scenario as software state; it is recovered here from the EC registers
/// the scenario writes (shift mode and the silent fan bit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum Scenario {
    ExtremePerformance,
    Balanced,
    Silent,
    SuperBattery,
}

impl Scenario {
    pub const ALL: [Self; 4] = [Self::ExtremePerformance, Self::Balanced, Self::Silent, Self::SuperBattery];

    pub(crate) const fn shift(self) -> u8 {
        match self {
            Self::ExtremePerformance => 0xC4,
            Self::Balanced | Self::Silent => 0xC1,
            Self::SuperBattery => 0xC2,
        }
    }

    pub(crate) fn detect(shift: u8, fan_mode: u8) -> Option<Self> {
        match (shift, fan_mode & ec::FAN_SILENT != 0) {
            (0xC4, _) => Some(Self::ExtremePerformance),
            (0xC1, true) => Some(Self::Silent),
            (0xC1, false) => Some(Self::Balanced),
            (0xC2, _) => Some(Self::SuperBattery),
            _ => None,
        }
    }
}

/// One fan's curve as the EC stores it.
///
/// EC layout for the CPU fan (GPU is `+0x18`): breakpoints `0x6A..=0x6F`, speeds `0x72..=0x78`.
/// Speed `i` applies from breakpoint `i - 1` upwards; speed 0 applies from 0 °C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FanCurve {
    /// Lower temperature bound (°C) of each editable point; `temps[0]` is always 0.
    pub temps: [u8; CURVE_POINTS],
    /// Fan duty (%) per point.
    pub speeds: [u8; CURVE_POINTS],
    /// Topmost point `(lower bound °C, duty %)`, owned by the firmware.
    pub top: (u8, u8),
}

impl FanCurve {
    /// Makes speeds non-decreasing with temperature and caps them at `ceiling` — the
    /// firmware's top point — so the fan never slows down when the laptop gets hottest.
    pub fn normalized(mut speeds: [u8; CURVE_POINTS], ceiling: u8) -> [u8; CURVE_POINTS] {
        let ceiling = ceiling.min(100);
        let mut floor = 0;
        for s in &mut speeds {
            *s = (*s).clamp(floor, ceiling);
            floor = *s;
        }
        speeds
    }

    /// Index of the point whose temperature band contains `temp_c`, or `CURVE_POINTS` for the
    /// firmware-owned top point.
    pub fn active_point(&self, temp_c: u8) -> usize {
        if temp_c >= self.top.0 {
            return CURVE_POINTS;
        }
        self.temps.iter().rposition(|&t| temp_c >= t).unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FanStatus {
    /// `None` when the sensor reports 0 or 255.
    pub temp_c: Option<u8>,
    pub rpm: u32,
    /// Current target duty (%) chosen by the EC.
    pub duty: u8,
    pub curve: FanCurve,
}

/// Everything the UI shows, read in one pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub cpu: FanStatus,
    pub gpu: FanStatus,
    pub shift_raw: u8,
    pub fan_mode_raw: u8,
    /// `None` when the registers hold a combination MSI Center never writes.
    pub scenario: Option<Scenario>,
    pub advanced_fan: bool,
    pub cooler_boost: bool,
    /// Stop-charging threshold in %, `None` = charge to 100 %.
    pub battery_limit: Option<u8>,
    /// `None` when the model has no webcam switch.
    pub webcam: Option<bool>,
    pub win_fn_swapped: bool,
    /// Fn Lock engaged (read-only; toggled on the keyboard with Fn+Esc).
    pub fn_lock: bool,
    pub kbd_backlight: u8,
    /// `None` when the model has no USB power share.
    pub usb_share: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub wmi_version: (u8, u8),
    /// e.g. `15K1IMS1.113`.
    pub ec_firmware: String,
    /// Build stamp following the firmware name, e.g. `0819202415:13:22`.
    pub ec_build: String,
}
