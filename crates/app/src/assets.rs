//! Embedded fonts and icons. Everything ships inside the executable.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

/// Geist / Geist Mono (SIL OFL 1.1), see `assets/fonts/OFL.txt`.
pub const FONTS: [&[u8]; 5] = [
    include_bytes!("../../../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/GeistMono-Medium.ttf"),
];

macro_rules! icons {
    ($($variant:ident => $file:literal),* $(,)?) => {
        /// Phosphor icons (MIT), see `assets/icons/LICENSE`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon { $($variant),* }

        impl Icon {
            pub const fn path(self) -> &'static str {
                match self { $(Self::$variant => concat!("icons/", $file)),* }
            }
        }

        fn icon_bytes(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $file) => Some(include_bytes!(concat!("../../../assets/icons/", $file))),)*
                _ => None,
            }
        }
    };
}

icons! {
    Fan => "fan.svg",
    Gauge => "gauge.svg",
    Battery => "battery-charging.svg",
    Rocket => "rocket-launch.svg",
    Scales => "scales.svg",
    Moon => "moon-stars.svg",
    Leaf => "leaf.svg",
    Wind => "wind.svg",
    Keyboard => "keyboard.svg",
    Webcam => "webcam.svg",
    Usb => "usb.svg",
    Swap => "arrows-left-right.svg",
    Chart => "chart-line-up.svg",
    Sliders => "sliders-horizontal.svg",
    Warning => "warning-circle.svg",
    Reset => "arrow-counter-clockwise.svg",
    Info => "info.svg",
    Minimize => "minus.svg",
    Close => "x.svg",
}

#[derive(Debug)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(icon_bytes(path).map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
