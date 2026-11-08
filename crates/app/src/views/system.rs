use gpui::{Context, SharedString, Window, div, prelude::*, px};

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{Width, card, icon, inset, segmented, switch_row};

impl MsiApp {
    pub(crate) fn render_system(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let Some(s) = self.snap.clone() else { return div() };
        let ring = self.keyboard_nav;

        let level = usize::from(s.kbd_backlight);
        let kbd_at = self.transitions.animate(("kbd", 0), level as f32, Spec::movement(motion::MOVE));
        let kbd = segmented(
            "kbd-backlight",
            &["Выкл", "1", "2", "3"].map(SharedString::from),
            Some(level),
            kbd_at,
            Width::Content,
            ring,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.set_kbd_backlight(i as u8, kb, cx)
            }),
        );

        let mut toggle = |key: &'static str, on: bool| -> f32 {
            self.transitions.animate((key, 0), if on { 1. } else { 0. }, Spec::movement(motion::TOGGLE))
        };
        let webcam = s.webcam.map(|on| {
            let at = toggle("webcam", on);
            switch_row(
                "webcam",
                Icon::Webcam,
                "Веб-камера",
                None,
                on,
                at,
                ring,
                MsiApp::handler(cx, |this, (v, kb): (bool, bool), _, cx| this.set_webcam(v, kb, cx)),
            )
        });
        let win_fn_at = toggle("winfn", s.win_fn_swapped);
        let win_fn = switch_row(
            "win-fn",
            Icon::Swap,
            "Поменять Win и Fn",
            None,
            s.win_fn_swapped,
            win_fn_at,
            ring,
            MsiApp::handler(cx, |this, (v, kb): (bool, bool), _, cx| this.set_win_fn(v, kb, cx)),
        );
        let usb = s.usb_share.map(|on| {
            let at = toggle("usb", on);
            switch_row(
                "usb-share",
                Icon::Usb,
                "USB-зарядка при выключении",
                None,
                on,
                at,
                ring,
                MsiApp::handler(cx, |this, (v, kb): (bool, bool), _, cx| this.set_usb_share(v, kb, cx)),
            )
        });

        card(Icon::Sliders, "Система", None)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pl(px(12.))
                    .child(icon(Icon::Keyboard, 15., if level > 0 { t.accent } else { t.text_muted }))
                    .child(div().flex_1().text_size(px(13.)).child("Подсветка"))
                    .child(kbd),
            )
            .child(inset().p(px(2.)).children(webcam).child(win_fn).children(usb))
    }
}
