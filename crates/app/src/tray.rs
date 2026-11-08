//! System tray icon with a native context menu (Win32 `Shell_NotifyIconW`).
//!
//! A hidden top-level window receives the icon's callbacks on the UI thread (gpui's message
//! loop dispatches it) and forwards them over a channel to the app.
//!
//! The context menu runs a modal loop that keeps pumping gpui's messages, so it must not run
//! inside a gpui callback (re-entering the app panics). The app publishes the menu items with
//! [`Tray::set_menu`]; the window procedure shows them and sends back the chosen index.

use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, GetCursorPos,
    GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    PostMessageW, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SM_CYSMICON, SetForegroundWindow,
    TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU,
    WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{PCSTR, PCWSTR, w};

/// Window class of the hidden tray window; a second instance finds it by this name.
pub const CLASS: PCWSTR = w!("open-msi-control.tray");
/// Sent by a second instance: show the existing window.
pub const WM_SHOW_EXISTING: u32 = WM_APP + 2;
const WM_TRAY: u32 = WM_APP + 1;
const ICON_ID: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// Left click: open or close the flyout.
    Toggle,
    /// An item of the context menu was chosen (index into the last [`Tray::set_menu`] list).
    MenuPick(usize),
    /// Another instance was started.
    ShowRequested,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Action { label: String, checked: bool },
    Separator,
}

/// Items of the context menu, kept current by the app.
static MENU: Mutex<Vec<MenuItem>> = Mutex::new(Vec::new());

static EVENTS: OnceLock<flume::Sender<TrayEvent>> = OnceLock::new();
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
/// `HICON` and `HWND` as integers, for re-adding the icon after Explorer restarts.
static ICON: OnceLock<(isize, isize)> = OnceLock::new();

#[derive(Debug)]
pub struct Tray {
    hwnd: HWND,
}

fn send(e: TrayEvent) {
    if let Some(tx) = EVENTS.get() {
        let _ = tx.send(e);
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            match lparam.0 as u32 {
                WM_LBUTTONUP => send(TrayEvent::Toggle),
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    let items = MENU.lock().map(|m| m.clone()).unwrap_or_default();
                    if let Some(i) = show_menu(hwnd, &items) {
                        send(TrayEvent::MenuPick(i));
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_SHOW_EXISTING => {
            send(TrayEvent::ShowRequested);
            LRESULT(0)
        }
        m if TASKBAR_CREATED.get() == Some(&m) => {
            if let Some(&(icon, _)) = ICON.get() {
                add_icon(hwnd, HICON(icon as _), "Open MSI Control");
            }
            LRESULT(0)
        }
        // SAFETY: default processing for every other message of our own window.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn icon_data(hwnd: HWND, tip: &str) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        uFlags: NIF_TIP,
        ..Default::default()
    };
    for (dst, src) in nid.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
        *dst = src;
    }
    nid
}

fn add_icon(hwnd: HWND, icon: HICON, tip: &str) -> bool {
    let mut nid = icon_data(hwnd, tip);
    nid.uFlags |= NIF_ICON | NIF_MESSAGE;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    // SAFETY: `nid` is fully initialised and outlives the call.
    unsafe { Shell_NotifyIconW(NIM_ADD, &nid) }.as_bool()
}

/// Opt the process into dark native menus (uxtheme `SetPreferredAppMode`, ordinal 135).
fn force_dark_menus() {
    // SAFETY: uxtheme ships with every supported Windows; the ordinals are stable since 1903.
    // If they are missing we simply keep light menus.
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        if let Some(set_mode) = GetProcAddress(uxtheme, PCSTR(std::ptr::without_provenance(135))) {
            let set_mode: extern "system" fn(i32) -> i32 = std::mem::transmute(set_mode);
            const FORCE_DARK: i32 = 2;
            set_mode(FORCE_DARK);
        }
        if let Some(flush) = GetProcAddress(uxtheme, PCSTR(std::ptr::without_provenance(136))) {
            let flush: extern "system" fn() = std::mem::transmute(flush);
            flush();
        }
    }
}

impl Tray {
    /// Creates the hidden window and the icon. Must run on the UI thread.
    pub fn create(tooltip: &str) -> windows::core::Result<(Self, flume::Receiver<TrayEvent>)> {
        let (tx, rx) = flume::unbounded();
        let _ = EVENTS.set(tx);
        force_dark_menus();
        // SAFETY: standard window-class registration and creation for a never-shown window.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: CLASS,
                ..Default::default()
            };
            RegisterClassW(&class);
            let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS,
                w!("open-msi-control tray"),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            // Icon resource 1 of our own executable (the app icon).
            let icon = LoadImageW(
                Some(instance.into()),
                PCWSTR(std::ptr::without_provenance(1)), // MAKEINTRESOURCE(1)
                IMAGE_ICON,
                GetSystemMetrics(SM_CXSMICON),
                GetSystemMetrics(SM_CYSMICON),
                LR_DEFAULTCOLOR,
            )?;
            let icon = HICON(icon.0);
            let _ = ICON.set((icon.0 as isize, hwnd.0 as isize));
            add_icon(hwnd, icon, tooltip);
            Ok((Self { hwnd }, rx))
        }
    }

    pub fn set_tooltip(&self, text: &str) {
        let nid = icon_data(self.hwnd, text);
        // SAFETY: `nid` is fully initialised; NIM_MODIFY with NIF_TIP only updates the text.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    /// Replaces the context menu items (cheap; call whenever they may have changed).
    pub fn set_menu(&self, items: Vec<MenuItem>) {
        if let Ok(mut m) = MENU.lock() {
            *m = items;
        }
    }
}

/// Shows the menu at the cursor and returns the index of the chosen action, if any.
/// Blocks (pumping messages) until the menu closes.
fn show_menu(hwnd: HWND, items: &[MenuItem]) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    // SAFETY: menu handle is created, used and destroyed within this call on the UI thread.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for (i, item) in items.iter().enumerate() {
            let _ = match item {
                MenuItem::Action { label, checked } => {
                    let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                    let flags = if *checked { MF_STRING | MF_CHECKED } else { MF_STRING };
                    AppendMenuW(menu, flags, i + 1, PCWSTR(wide.as_ptr()))
                }
                MenuItem::Separator => AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()),
            };
        }
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Required so the menu closes when the user clicks elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        usize::try_from(chosen.0).ok().filter(|&c| c > 0).map(|c| c - 1)
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let nid = icon_data(self.hwnd, "");
        // SAFETY: removing our own icon and destroying our own window.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
