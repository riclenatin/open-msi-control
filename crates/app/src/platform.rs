//! Small OS queries and window/process plumbing (Windows only, like the whole app).

/// Windows "Show animations in Windows" setting (Settings → Accessibility → Visual effects).
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

/// Marketing model name from SMBIOS, e.g. `Cyborg 15 A12VF`.
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

/// `LANGID` of the Windows display language.
pub fn ui_language() -> u16 {
    // SAFETY: no arguments, returns a value.
    unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() }
}

const SETTINGS_KEY: windows::core::PCWSTR = windows::core::w!(r"Software\open-msi-control");

/// Reads a per-user string setting from `HKCU\Software\open-msi-control`.
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

/// Holds the single-instance mutex for the life of the process.
#[derive(Debug)]
pub struct InstanceGuard(windows::Win32::Foundation::HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: closing the mutex handle we own.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Claims the single-instance mutex. `None` means another instance is already running; it is
/// then asked to show its window.
pub fn claim_single_instance() -> Option<InstanceGuard> {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LPARAM, WPARAM};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};
    use windows::core::{PCWSTR, w};
    // SAFETY: named mutex creation; GetLastError is read immediately after.
    let handle = unsafe { CreateMutexW(None, true, w!(r"Local\open-msi-control")) }.ok()?;
    // SAFETY: as above.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        // SAFETY: the existing instance's hidden tray window; posting is fire-and-forget.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            if let Ok(hwnd) = FindWindowW(crate::tray::CLASS, PCWSTR::null()) {
                let _ = PostMessageW(Some(hwnd), crate::tray::WM_SHOW_EXISTING, WPARAM(0), LPARAM(0));
            }
        }
        return None;
    }
    Some(InstanceGuard(handle))
}

fn hwnd(window: &gpui::Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(windows::Win32::Foundation::HWND(h.hwnd.get() as _)),
        _ => None,
    }
}

/// Hides the window (it stays alive; the tray brings it back).
pub fn hide_window(window: &gpui::Window) {
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    if let Some(h) = hwnd(window) {
        // SAFETY: our own top-level window.
        unsafe {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
}

/// Shows, restores and focuses the window.
pub fn show_window(window: &gpui::Window) {
    use windows::Win32::UI::WindowsAndMessaging::{SW_RESTORE, SW_SHOW, ShowWindow};
    if let Some(h) = hwnd(window) {
        // SAFETY: our own top-level window.
        unsafe {
            let _ = ShowWindow(h, SW_SHOW);
            let _ = ShowWindow(h, SW_RESTORE);
        }
    }
    window.activate_window();
}

pub fn is_window_visible(window: &gpui::Window) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
    // SAFETY: querying our own window.
    hwnd(window).is_some_and(|h| unsafe { IsWindowVisible(h).as_bool() && !IsIconic(h).as_bool() })
}
