//! Feature layer: mirrors what MSI Center does on WMI 2.x firmware.
//!
//! Every write is a read-modify-write of a single EC byte, so bits this crate does not own are
//! preserved exactly as MSI Center preserves them. Where each feature lives comes from the
//! laptop's [`Layout`]; a feature the layout lacks is reported as `None` and never written.

use std::fmt;

use crate::ec::{self, Bit, Flag, Gated, Layout};
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
    layout: Layout,
}

#[cfg(windows)]
impl Laptop<crate::WmiTransport> {
    /// Connects to the real `MSI_ACPI` instance. Requires an elevated process.
    pub fn connect() -> Result<Self> {
        Self::new(crate::WmiTransport::connect()?)
    }
}

impl<T: Transport> Laptop<T> {
    /// Validates the interface version, reads firmware identification and picks the EC layout
    /// for that firmware.
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
        let (ec_config, layout) = Layout::resolve(&text[..split]);
        let info = Info {
            wmi_version: (major, minor),
            ec_firmware: text[..split].to_owned(),
            ec_build: text[split..].to_owned(),
            ec_config,
        };
        Ok(Self { transport, info, layout })
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    /// The EC registers in use, one feature per line (for diagnostics).
    pub fn layout(&self) -> impl fmt::Display + '_ {
        &self.layout
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

    fn read_bit(&self, bit: Option<Bit>) -> Result<Option<bool>> {
        bit.map(|b| self.read_ec(b.address).map(|v| b.is_set(v))).transpose()
    }

    /// `None` when the layout has no such switch or its capability bit is clear.
    fn read_gated(&self, gated: Option<Gated>) -> Result<Option<bool>> {
        let Some(Gated { switch, supported }) = gated else { return Ok(None) };
        let cap = supported.map(|cap| self.read_ec(cap.address).map(|v| (cap, v))).transpose()?;
        let value = match cap {
            Some((cap, v)) if !cap.is_set(v) => return Ok(None),
            Some((cap, v)) if cap.address == switch.address => v,
            _ => self.read_ec(switch.address)?,
        };
        Ok(Some(switch.is_set(value)))
    }

    /// Fan-mode register and its advanced (user curve) flag.
    fn advanced_fan(&self) -> Result<(u8, Flag)> {
        self.layout
            .fan_mode
            .and_then(|m| Some((m.address, m.advanced?)))
            .ok_or(Error::Unsupported("custom fan curves"))
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

        let layout = &self.layout;
        let shift_raw = layout.shift.map(|s| self.read_ec(s.address)).transpose()?;
        let fan_mode_raw = layout.fan_mode.map(|m| self.read_ec(m.address)).transpose()?;
        let advanced = layout.fan_mode.and_then(|m| m.advanced);
        let read = |addr: Option<u8>| addr.map(|a| self.read_ec(a)).transpose();

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
            scenario: shift_raw.and_then(|shift| layout.scenario(shift, fan_mode_raw)),
            scenarios: layout.scenarios(),
            advanced_fan: advanced.zip(fan_mode_raw).is_some_and(|(flag, v)| flag.is_on(v)),
            cooler_boost: self.read_bit(layout.cooler_boost)?,
            charge_limit: read(layout.battery)?.map(ec::charge_limit),
            webcam: self.read_gated(layout.webcam)?,
            win_fn_swapped: layout
                .win_fn
                .map(|w| self.read_ec(w.bit.address).map(|v| w.swapped(v)))
                .transpose()?,
            fn_lock: self.read_bit(layout.fn_unlocked)?.map(|unlocked| !unlocked),
            kbd_backlight: read(layout.kbd_backlight.map(|k| k.address))?.map(|v| v & ec::KBD_LEVEL_MASK),
            usb_share: self.read_gated(layout.usb_share)?,
        })
    }

    /// Writes the scenario's shift mode and sets or clears the silent fan flag.
    pub fn set_scenario(&self, scenario: Scenario) -> Result<()> {
        let (address, value) = self.layout.shift_for(scenario).ok_or(Error::Unsupported("this scenario"))?;
        self.write_ec(address, value)?;
        if let Some(m) = self.layout.fan_mode
            && let Some(silent) = m.silent
        {
            self.update_ec(m.address, |v| silent.apply(v, scenario == Scenario::Silent))?;
        }
        Ok(())
    }

    pub fn set_cooler_boost(&self, on: bool) -> Result<()> {
        let bit = self.layout.cooler_boost.ok_or(Error::Unsupported("Cooler Boost"))?;
        self.set_bits(bit.address, bit.mask, on)
    }

    /// Returns to the firmware's built-in curve; the silent flag is kept.
    pub fn set_auto_fan(&self) -> Result<()> {
        let (address, advanced) = self.advanced_fan()?;
        self.update_ec(address, |v| advanced.apply(v, false))
    }

    /// Writes user curves for both fans (normalised first) and switches the EC to them.
    pub fn set_fan_curves(&self, cpu: [u8; CURVE_POINTS], gpu: [u8; CURVE_POINTS]) -> Result<()> {
        let (address, advanced) = self.advanced_fan()?;
        for (fan, speeds) in [(Fan::Cpu, cpu), (Fan::Gpu, gpu)] {
            // Bytes 0 (live duty) and 7 (top point) are echoed back unchanged, as MSI does.
            let current = self.get(Method::Get_Fan, fan as u8)?;
            let mut table = [0; CURVE_POINTS + 2];
            table.copy_from_slice(&current[..CURVE_POINTS + 2]);
            let ceiling = table[CURVE_POINTS + 1];
            table[1..=CURVE_POINTS].copy_from_slice(&FanCurve::normalized(speeds, ceiling));
            self.set(Method::Set_Fan, fan as u8, &table)?;
        }
        self.update_ec(address, |v| advanced.apply(v, true))
    }

    /// Stop charging at `percent` (`10..=100`); 100 charges fully.
    pub fn set_charge_limit(&self, percent: u8) -> Result<()> {
        let address = self.layout.battery.ok_or(Error::Unsupported("battery charge limit"))?;
        if !(10..=100).contains(&percent) {
            return Err(Error::InvalidArgument(format!("charge limit {percent}% is outside 10..=100")));
        }
        self.write_ec(address, ec::BATTERY_ENABLED | percent)
    }

    pub fn set_webcam(&self, on: bool) -> Result<()> {
        let webcam = self.layout.webcam.ok_or(Error::Unsupported("webcam switch"))?;
        self.set_bits(webcam.switch.address, webcam.switch.mask, on)
    }

    pub fn set_win_fn_swap(&self, swapped: bool) -> Result<()> {
        let win_fn = self.layout.win_fn.ok_or(Error::Unsupported("Win/Fn swap"))?;
        self.set_bits(win_fn.bit.address, win_fn.bit.mask, win_fn.bit_for(swapped))
    }

    pub fn set_kbd_backlight(&self, level: u8) -> Result<()> {
        let kbd = self.layout.kbd_backlight.ok_or(Error::Unsupported("keyboard backlight"))?;
        if level > ec::KBD_MAX_LEVEL {
            return Err(Error::InvalidArgument(format!("backlight level {level} is above 3")));
        }
        self.update_ec(kbd.address, |v| (v & kbd.keep) | level)
    }

    pub fn set_usb_share(&self, on: bool) -> Result<()> {
        let usb = self.layout.usb_share.ok_or(Error::Unsupported("USB power share"))?;
        self.set_bits(usb.switch.address, usb.switch.mask, on)
    }
}
