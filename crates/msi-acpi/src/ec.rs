//! Embedded-controller register map for MSI WMI 2.x firmware (`Get_Data`/`Set_Data` space).
//!
//! Derived from MSI Center's own calls and cross-checked with the Linux `msi-ec` driver
//! (`CONF_G2_1`). See `docs/PROTOCOL.md`.

/// bit1 = webcam on.
pub(crate) const WEBCAM: u8 = 0x2E;
pub(crate) const WEBCAM_ON: u8 = 1 << 1;

/// Capability bits; bit1 = webcam switch supported.
pub(crate) const DEVICE_CAPS: u8 = 0x2F;
pub(crate) const CAP_WEBCAM: u8 = 1 << 1;

/// bit7 = Cooler Boost (fans at maximum).
pub(crate) const COOLER_BOOST: u8 = 0x98;
pub(crate) const COOLER_BOOST_ON: u8 = 1 << 7;

/// bit3 = supported, bit5 = powering USB while the laptop is off.
pub(crate) const USB_SHARE: u8 = 0xBF;
pub(crate) const USB_SHARE_SUPPORTED: u8 = 1 << 3;
pub(crate) const USB_SHARE_ON: u8 = 1 << 5;

/// Shift (performance) mode: `0xC0 | mode`.
pub(crate) const SHIFT: u8 = 0xD2;

/// Keyboard backlight: `0x80 | level` (0..=3). MSI keeps only bit7 when writing.
pub(crate) const KBD_BACKLIGHT: u8 = 0xD3;
pub(crate) const KBD_KEEP: u8 = 1 << 7;
pub(crate) const KBD_LEVEL_MASK: u8 = 0x03;
pub(crate) const KBD_MAX_LEVEL: u8 = 3;

/// Fan mode: bit4 = silent, bit7 = advanced (user curve).
pub(crate) const FAN_MODE: u8 = 0xD4;
pub(crate) const FAN_SILENT: u8 = 1 << 4;
pub(crate) const FAN_ADVANCED: u8 = 1 << 7;

/// Battery charge limit: bit7 = enabled, low 7 bits = stop-charging percentage.
pub(crate) const BATTERY: u8 = 0xD7;
pub(crate) const BATTERY_ENABLED: u8 = 1 << 7;
pub(crate) const BATTERY_PERCENT_MASK: u8 = 0x7F;

/// bit4 cleared = Win and Fn keys swapped.
pub(crate) const WIN_FN: u8 = 0xE8;
pub(crate) const WIN_FN_NORMAL: u8 = 1 << 4;

/// Status flags; bit1 cleared = Fn Lock engaged. The EC toggles it on Fn+Esc; MSI Center only
/// reads it (other bits of this byte are owned by MSI's service), so this crate never writes it.
pub(crate) const STATUS: u8 = 0xD9;
pub(crate) const STATUS_FN_UNLOCKED: u8 = 1 << 1;
