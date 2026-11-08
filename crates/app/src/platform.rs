//! Small OS queries.

/// Windows "Show animations in Windows" setting (Settings → Accessibility → Visual effects).
#[cfg(windows)]
pub fn reduced_motion() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    use windows::core::BOOL;
    let mut enabled = BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL into the provided pointer.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut enabled).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && !enabled.as_bool()
}

#[cfg(not(windows))]
pub fn reduced_motion() -> bool {
    false
}

/// Marketing model name from SMBIOS, e.g. `Cyborg 15 A12VF`.
#[cfg(windows)]
pub fn model_name() -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::w;
    let mut buf = [0u16; 128];
    let mut len = u32::try_from(size_of_val(&buf)).ok()?;
    // SAFETY: `buf`/`len` describe a valid writable buffer; RRF_RT_REG_SZ guarantees a string.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"HARDWARE\DESCRIPTION\System\BIOS"),
            w!("SystemProductName"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    }
    .ok()
    .ok()?;
    let chars = (len as usize / 2).saturating_sub(1);
    let name = String::from_utf16_lossy(&buf[..chars]).trim().to_owned();
    (!name.is_empty()).then_some(name)
}

#[cfg(not(windows))]
pub fn model_name() -> Option<String> {
    None
}

/// `LANGID` of the Windows display language.
#[cfg(windows)]
pub fn ui_language() -> u16 {
    // SAFETY: no arguments, returns a value.
    unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() }
}

#[cfg(not(windows))]
pub fn ui_language() -> u16 {
    0x0409
}

#[cfg(windows)]
const SETTINGS_KEY: windows::core::PCWSTR = windows::core::w!(r"Software\open-msi-control");

/// Reads a per-user string setting from `HKCU\Software\open-msi-control`.
#[cfg(windows)]
pub fn load_setting(name: &str) -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let name = windows::core::HSTRING::from(name);
    let mut buf = [0u16; 64];
    let mut len = u32::try_from(size_of_val(&buf)).ok()?;
    // SAFETY: `buf`/`len` describe a valid writable buffer; RRF_RT_REG_SZ guarantees a string.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            SETTINGS_KEY,
            &name,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    }
    .ok()
    .ok()?;
    let chars = (len as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buf[..chars]))
}

/// Writes a per-user string setting; failures are ignored (the setting is a convenience).
#[cfg(windows)]
pub fn save_setting(name: &str, value: &str) {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
    let name = windows::core::HSTRING::from(name);
    let data: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    let Ok(bytes) = u32::try_from(data.len() * 2) else { return };
    // SAFETY: `data` is a NUL-terminated UTF-16 buffer of `bytes` bytes; the key is created if missing.
    let _ = unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, SETTINGS_KEY, &name, REG_SZ.0, Some(data.as_ptr().cast()), bytes)
    };
}

#[cfg(not(windows))]
pub fn load_setting(_name: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn save_setting(_name: &str, _value: &str) {}
