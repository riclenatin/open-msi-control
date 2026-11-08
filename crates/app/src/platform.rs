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
