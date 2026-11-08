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
/// the scenario writes (shift mode and the silent fan flag).
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

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A set of scenarios, e.g. those a model's EC layout can express.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scenarios(u8);

impl Scenarios {
    pub const ALL: Self = Self(0b1111);

    pub fn contains(self, scenario: Scenario) -> bool {
        self.0 & scenario.bit() != 0
    }

    /// Members in [`Scenario::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = Scenario> {
        Scenario::ALL.into_iter().filter(move |&s| self.contains(s))
    }
}

impl FromIterator<Scenario> for Scenarios {
    fn from_iter<I: IntoIterator<Item = Scenario>>(iter: I) -> Self {
        Self(iter.into_iter().fold(0, |set, s| set | s.bit()))
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

/// Everything the UI shows, read in one pass. Features the model's EC layout lacks are `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub cpu: FanStatus,
    pub gpu: FanStatus,
    pub shift_raw: Option<u8>,
    pub fan_mode_raw: Option<u8>,
    /// `None` when the registers hold a combination MSI Center never writes.
    pub scenario: Option<Scenario>,
    /// Scenarios this model can be switched to.
    pub scenarios: Scenarios,
    /// The user fan curve is active (always `false` without an advanced fan mode).
    pub advanced_fan: bool,
    pub cooler_boost: Option<bool>,
    /// Stop-charging threshold in %, 100 = charges fully.
    pub charge_limit: Option<u8>,
    /// Also `None` while the firmware reports no webcam switch.
    pub webcam: Option<bool>,
    pub win_fn_swapped: Option<bool>,
    /// Fn Lock engaged (read-only; toggled on the keyboard with Fn+Esc).
    pub fn_lock: Option<bool>,
    /// Level `0..=3`.
    pub kbd_backlight: Option<u8>,
    /// Also `None` while the firmware reports no USB power share.
    pub usb_share: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub wmi_version: (u8, u8),
    /// e.g. `15K1IMS1.113`.
    pub ec_firmware: String,
    /// Build stamp following the firmware name, e.g. `0819202415:13:22`.
    pub ec_build: String,
    /// msi-ec config whose register layout is in use, e.g. `CONF_G2_1`. `None` when msi-ec does
    /// not list the firmware; the built-in MS-15K1 layout (same registers as `CONF_G2_1`) is used.
    pub ec_config: Option<&'static str>,
}
