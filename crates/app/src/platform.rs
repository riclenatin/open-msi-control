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

/// `true` on AC power (or when Windows cannot tell).
pub fn on_ac_power() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: fills a caller-provided struct.
    let ok = unsafe { GetSystemPowerStatus(&mut status) }.is_ok();
    !ok || status.ACLineStatus != 0
}

/// Precision Touchpad state from `HKCU\…\PrecisionTouchPad\Status\Enabled`; `None` without one.
pub fn touchpad_enabled() -> Option<bool> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::w;
    let mut value = 0u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: `value`/`len` describe a DWORD-sized buffer; RRF_RT_REG_DWORD enforces the type.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Status"),
            w!("Enabled"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut len),
        )
    }
    .ok()
    .ok()?;
    Some(value != 0)
}

/// Toggles the touchpad with Ctrl+Win+F24, the Windows Precision Touchpad system hotkey (what
/// MSI Center sends). It is a toggle: callers verify the result with [`touchpad_enabled`].
pub fn toggle_touchpad() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_F24, VK_LCONTROL, VK_LWIN,
    };
    let key = |vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, dwFlags: flags, ..Default::default() } },
    };
    let up = KEYEVENTF_KEYUP;
    let none = KEYBD_EVENT_FLAGS(0);
    let inputs = [
        key(VK_LCONTROL, none),
        key(VK_LWIN, none),
        key(VK_F24, none),
        key(VK_F24, up),
        key(VK_LWIN, up),
        key(VK_LCONTROL, up),
    ];
    // SAFETY: `inputs` is a valid array of keyboard INPUT records.
    unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32);
    }
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

/// HTTPS GET through WinINet (system proxy and certificate store). Blocking.
/// Returns the HTTP status and, for 2xx, the body (empty otherwise).
pub fn http_get(url: &str) -> std::io::Result<(u32, Vec<u8>)> {
    use std::ffi::c_void;
    use windows::Win32::Networking::WinInet::{
        HTTP_QUERY_FLAG_NUMBER, HTTP_QUERY_STATUS_CODE, HttpQueryInfoW, INTERNET_FLAG_NO_CACHE_WRITE,
        INTERNET_FLAG_NO_UI, INTERNET_FLAG_RELOAD, INTERNET_OPEN_TYPE_PRECONFIG, InternetCloseHandle,
        InternetOpenUrlW, InternetOpenW, InternetReadFile,
    };
    use windows::core::{HSTRING, PCWSTR};

    /// Closes a WinINet handle on drop.
    struct Handle(*mut c_void);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: a non-null handle returned by WinINet, closed exactly once.
            unsafe {
                let _ = InternetCloseHandle(self.0);
            }
        }
    }
    let open = |h: *mut c_void| {
        if h.is_null() { Err(std::io::Error::last_os_error()) } else { Ok(Handle(h)) }
    };

    let agent = HSTRING::from(concat!("open-msi-control/", env!("CARGO_PKG_VERSION")));
    let url = HSTRING::from(url);
    // SAFETY: wide strings outlive the calls; handles are wrapped right away and closed on drop
    // (request before session, by declaration order in reverse).
    unsafe {
        let session =
            open(InternetOpenW(&agent, INTERNET_OPEN_TYPE_PRECONFIG.0, PCWSTR::null(), PCWSTR::null(), 0))?;
        let flags = INTERNET_FLAG_RELOAD | INTERNET_FLAG_NO_CACHE_WRITE | INTERNET_FLAG_NO_UI;
        let request = open(InternetOpenUrlW(session.0, &url, None, flags, None))?;

        let mut status = 0u32;
        let mut len = size_of::<u32>() as u32;
        HttpQueryInfoW(
            request.0,
            HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
            Some((&raw mut status).cast()),
            &mut len,
            None,
        )?;
        let mut body = Vec::new();
        if !(200..300).contains(&status) {
            return Ok((status, body));
        }

        let mut buf = [0u8; 16 * 1024];
        loop {
            let mut read = 0u32;
            InternetReadFile(request.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut read)?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&buf[..read as usize]);
        }
        Ok((status, body))
    }
}

/// Top-left corner (logical px) for a `width × height` popup next to the tray: above the
/// taskbar near the cursor, clamped to the work area. Assumes the tray is on the primary display.
pub fn tray_popup_origin(width: f32, height: f32) -> (f32, f32) {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    const MARGIN: f32 = 12.0;
    let mut work = RECT::default();
    let mut cursor = POINT::default();
    // SAFETY: both calls fill caller-provided structs; GetDpiForSystem has no preconditions.
    let scale = unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&raw mut work).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let _ = GetCursorPos(&mut cursor);
        GetDpiForSystem() as f32 / 96.0
    };
    let (left, top) = (work.left as f32 / scale, work.top as f32 / scale);
    let (right, bottom) = (work.right as f32 / scale, work.bottom as f32 / scale);
    let (cx, cy) = (cursor.x as f32 / scale, cursor.y as f32 / scale);
    let x = (cx - width / 2.0).clamp(left + MARGIN, right - width - MARGIN);
    // Taskbar at the top: open below it; otherwise above.
    let y = if cy < top + (bottom - top) / 2.0 { top + MARGIN } else { bottom - height - MARGIN };
    (x, y)
}

/// Keeps a popup above other windows and asks DWM (Windows 11) for rounded corners.
pub fn style_popup(window: &gpui::Window) {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    use windows::Win32::UI::WindowsAndMessaging::{HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos};
    let Some(h) = hwnd(window) else { return };
    let pref = DWMWCP_ROUND;
    // SAFETY: our own window; the attribute value is a DWORD-sized enum passed by pointer.
    unsafe {
        let _ = DwmSetWindowAttribute(
            h,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const pref).cast(),
            size_of_val(&pref) as u32,
        );
        let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    }
}
