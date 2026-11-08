//! Feature layer: mirrors what MSI Center does on WMI 2.x firmware.
//!
//! Every write is a read-modify-write of a single EC byte, so bits this crate does not own are
//! preserved exactly as MSI Center preserves them.

use crate::ec;
use crate::model::{CURVE_POINTS, Fan, FanCurve, FanStatus, Info, Scenario, Snapshot};
use crate::transport::{Method, Transport};
use crate::{Error, Result};

/// Tachometer constant: `rpm = 60_000_000 / (raw * 2 * 62.5)`.
const TACH_CONSTANT: u32 = 480_000;
/// Length of the firmware name inside the `Get_EC` string, e.g. `15K1IMS1.113`.
const EC_NAME_LEN: usize = 12;

#[derive(Debug)]
pub struct Laptop<T> {
    transport: T,
    info: Info,
}

#[cfg(windows)]
impl Laptop<crate::WmiTransport> {
    /// Connects to the real `MSI_ACPI` instance. Requires an elevated process.
    pub fn connect() -> Result<Self> {
        Self::new(crate::WmiTransport::connect()?)
    }
}

impl<T: Transport> Laptop<T> {
    /// Validates the interface version and reads firmware identification.
    pub fn new(transport: T) -> Result<Self> {
        let wmi = transport.call(Method::Get_WMI, 0, &[])?;
        let (major, minor) = (wmi[1], wmi[2]);
        if major != 2 {
            return Err(Error::UnsupportedInterface { major, minor });
        }
        // Get_EC payload: [EC RAM version] "15K1IMS1.113" MMDDYYYY "HH:MM:SS"
        let ec = transport.call(Method::Get_EC, 0, &[])?;
        let text: String = ec[2..].iter().take_while(|&&b| b != 0).map(|&b| char::from(b)).collect();
        let split = text.len().min(EC_NAME_LEN);
        let info = Info {
            wmi_version: (major, minor),
            ec_firmware: text[..split].to_owned(),
            ec_build: text[split..].to_owned(),
        };
        Ok(Self { transport, info })
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    /// Calls a `Get_*` method and returns its 31-byte payload.
    fn get(&self, method: Method, index: u8) -> Result<[u8; 31]> {
        let p = self.transport.call(method, index, &[])?;
        if p[0] == 0 {
            return Err(Error::Rejected { method, index });
        }
        let mut out = [0; 31];
        out.copy_from_slice(&p[1..]);
        Ok(out)
    }

    fn set(&self, method: Method, index: u8, payload: &[u8]) -> Result<()> {
        let p = self.transport.call(method, index, payload)?;
        if p[0] == 0 {
            return Err(Error::Rejected { method, index });
        }
        Ok(())
    }

    /// Reads one EC RAM byte.
    pub fn read_ec(&self, addr: u8) -> Result<u8> {
        Ok(self.get(Method::Get_Data, addr)?[0])
    }

    fn write_ec(&self, addr: u8, value: u8) -> Result<()> {
        self.set(Method::Set_Data, addr, &[value])
    }

    fn update_ec(&self, addr: u8, f: impl FnOnce(u8) -> u8) -> Result<()> {
        let cur = self.read_ec(addr)?;
        let new = f(cur);
        if new != cur {
            self.write_ec(addr, new)?;
        }
        Ok(())
    }

    fn set_bits(&self, addr: u8, mask: u8, on: bool) -> Result<()> {
        self.update_ec(addr, |v| if on { v | mask } else { v & !mask })
    }

    fn fan_curve(&self, fan: Fan) -> Result<(u8, FanCurve)> {
        let speeds = self.get(Method::Get_Fan, fan as u8)?;
        // Get_Temperature(1|2) payload = EC [0x69, 0x6F, 0x70, 0x6A..=0x6E] (CPU; GPU +0x18).
        let temps = self.get(Method::Get_Temperature, fan as u8)?;
        let mut curve_speeds = [0; CURVE_POINTS];
        curve_speeds.copy_from_slice(&speeds[1..=CURVE_POINTS]);
        let curve = FanCurve {
            temps: [temps[0], temps[3], temps[4], temps[5], temps[6], temps[7]],
            speeds: curve_speeds,
            top: (temps[1], speeds[CURVE_POINTS + 1]),
        };
        Ok((speeds[0], curve))
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        let temps = self.get(Method::Get_Temperature, 0)?;
        let tach = self.get(Method::Get_Fan, 0)?;
        let temp = |b: u8| (b != 0 && b != 0xFF).then_some(b);
        let rpm = |hi: u8, lo: u8| match u32::from(u16::from_be_bytes([hi, lo])) {
            0 => 0,
            raw => TACH_CONSTANT / raw,
        };
        let (cpu_duty, cpu_curve) = self.fan_curve(Fan::Cpu)?;
        let (gpu_duty, gpu_curve) = self.fan_curve(Fan::Gpu)?;

        let shift_raw = self.read_ec(ec::SHIFT)?;
        let fan_mode_raw = self.read_ec(ec::FAN_MODE)?;
        let battery = self.read_ec(ec::BATTERY)?;
        let caps = self.read_ec(ec::DEVICE_CAPS)?;
        let webcam = self.read_ec(ec::WEBCAM)?;
        let usb = self.read_ec(ec::USB_SHARE)?;
        let battery_pct = battery & ec::BATTERY_PERCENT_MASK;

        Ok(Snapshot {
            cpu: FanStatus {
                temp_c: temp(temps[0]),
                rpm: rpm(tach[0], tach[1]),
                duty: cpu_duty,
                curve: cpu_curve,
            },
            gpu: FanStatus {
                temp_c: temp(temps[1]),
                rpm: rpm(tach[2], tach[3]),
                duty: gpu_duty,
                curve: gpu_curve,
            },
            shift_raw,
            fan_mode_raw,
            scenario: Scenario::detect(shift_raw, fan_mode_raw),
            advanced_fan: fan_mode_raw & ec::FAN_ADVANCED != 0,
            cooler_boost: self.read_ec(ec::COOLER_BOOST)? & ec::COOLER_BOOST_ON != 0,
            // MSI writes 0x80 | 100 ("Best for Mobility"); 0x80 alone also means no limit.
            battery_limit: (battery & ec::BATTERY_ENABLED != 0 && (1..100).contains(&battery_pct))
                .then_some(battery_pct),
            webcam: (caps & ec::CAP_WEBCAM != 0).then_some(webcam & ec::WEBCAM_ON != 0),
            win_fn_swapped: self.read_ec(ec::WIN_FN)? & ec::WIN_FN_NORMAL == 0,
            fn_lock: self.read_ec(ec::STATUS)? & ec::STATUS_FN_UNLOCKED == 0,
            kbd_backlight: self.read_ec(ec::KBD_BACKLIGHT)? & ec::KBD_LEVEL_MASK,
            usb_share: (usb & ec::USB_SHARE_SUPPORTED != 0).then_some(usb & ec::USB_SHARE_ON != 0),
        })
    }

    pub fn set_scenario(&self, scenario: Scenario) -> Result<()> {
        self.write_ec(ec::SHIFT, scenario.shift())?;
        self.set_bits(ec::FAN_MODE, ec::FAN_SILENT, scenario == Scenario::Silent)
    }

    pub fn set_cooler_boost(&self, on: bool) -> Result<()> {
        self.set_bits(ec::COOLER_BOOST, ec::COOLER_BOOST_ON, on)
    }

    /// Returns to the firmware's built-in curve; the silent bit is kept.
    pub fn set_auto_fan(&self) -> Result<()> {
        self.set_bits(ec::FAN_MODE, ec::FAN_ADVANCED, false)
    }

    /// Writes user curves for both fans (normalised first) and switches the EC to them.
    pub fn set_fan_curves(&self, cpu: [u8; CURVE_POINTS], gpu: [u8; CURVE_POINTS]) -> Result<()> {
        for (fan, speeds) in [(Fan::Cpu, cpu), (Fan::Gpu, gpu)] {
            // Bytes 0 (live duty) and 7 (top point) are echoed back unchanged, as MSI does.
            let current = self.get(Method::Get_Fan, fan as u8)?;
            let mut table = [0; CURVE_POINTS + 2];
            table.copy_from_slice(&current[..CURVE_POINTS + 2]);
            let ceiling = table[CURVE_POINTS + 1];
            table[1..=CURVE_POINTS].copy_from_slice(&FanCurve::normalized(speeds, ceiling));
            self.set(Method::Set_Fan, fan as u8, &table)?;
        }
        self.set_bits(ec::FAN_MODE, ec::FAN_ADVANCED, true)
    }

    /// Stop charging at `percent` (`10..=100`); `None` charges to 100 %.
    pub fn set_battery_limit(&self, percent: Option<u8>) -> Result<()> {
        let percent = percent.unwrap_or(100);
        if !(10..=100).contains(&percent) {
            return Err(Error::InvalidArgument(format!("charge limit {percent}% is outside 10..=100")));
        }
        self.write_ec(ec::BATTERY, ec::BATTERY_ENABLED | percent)
    }

    pub fn set_webcam(&self, on: bool) -> Result<()> {
        self.set_bits(ec::WEBCAM, ec::WEBCAM_ON, on)
    }

    pub fn set_win_fn_swap(&self, swapped: bool) -> Result<()> {
        self.set_bits(ec::WIN_FN, ec::WIN_FN_NORMAL, !swapped)
    }

    pub fn set_kbd_backlight(&self, level: u8) -> Result<()> {
        if level > ec::KBD_MAX_LEVEL {
            return Err(Error::InvalidArgument(format!("backlight level {level} is above 3")));
        }
        self.update_ec(ec::KBD_BACKLIGHT, |v| (v & ec::KBD_KEEP) | level)
    }

    pub fn set_usb_share(&self, on: bool) -> Result<()> {
        self.set_bits(ec::USB_SHARE, ec::USB_SHARE_ON, on)
    }
}
