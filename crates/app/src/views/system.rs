use gpui::{Context, SharedString, Window, div, prelude::*, px};

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{Handler, SwitchRow, Width, card, control_row, inset, mono, segmented};

type Setter = fn(&mut MsiApp, bool, bool, &mut Context<MsiApp>);

impl MsiApp {
    pub(crate) fn render_system(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let tr = self.tr();
        let Some(s) = self.snap.clone() else { return div() };
        let ring = self.keyboard_nav;

        let level = usize::from(s.kbd_backlight);
        let kbd_at = self.transitions.animate(("kbd", 0), level as f32, Spec::movement(motion::MOVE));
        let kbd = segmented(
            "kbd-backlight",
            &[tr.backlight_off, "1", "2", "3"].map(SharedString::from),
            Some(level),
            kbd_at,
            Width::Content,
            ring,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.set_kbd_backlight(i as u8, kb, cx)
            }),
        );

        let mut row = |key: &'static str, icon, label, explain, on: bool, set: Setter| {
            let thumb_at =
                self.transitions.animate((key, 0), f32::from(u8::from(on)), Spec::movement(motion::TOGGLE));
            let on_toggle: Handler<(bool, bool)> =
                MsiApp::handler(cx, move |this, (v, kb): (bool, bool), _, cx| set(this, v, kb, cx));
            SwitchRow {
                id: key.into(),
                icon,
                label,
                description: None,
                explain: Some(explain),
                on,
                thumb_at,
                ring,
                on_toggle,
            }
            .render()
        };
        let touchpad = self.touchpad.map(|on| {
            row("touchpad", Icon::Touchpad, tr.touchpad, tr.touchpad_info, on, MsiApp::set_touchpad)
        });
        let webcam =
            s.webcam.map(|on| row("webcam", Icon::Webcam, tr.webcam, tr.webcam_info, on, MsiApp::set_webcam));
        let win_fn =
            row("win-fn", Icon::Swap, tr.win_fn, tr.win_fn_info, s.win_fn_swapped, MsiApp::set_win_fn);
        let usb = s.usb_share.map(|on| {
            row("usb-share", Icon::Usb, tr.usb_share, tr.usb_share_info, on, MsiApp::set_usb_share)
        });

        // Fn Lock is owned by the EC (Fn+Esc); show its state only.
        let fn_lock = mono(if s.fn_lock { tr.on } else { tr.off })
            .px(px(10.))
            .text_size(px(12.))
            .text_color(if s.fn_lock { t.accent } else { t.text_muted });

        card(Icon::Sliders, tr.system, None, None)
            .child(control_row(Icon::Keyboard, level > 0, tr.backlight, Some(tr.backlight_info), kbd))
            .child(inset().p(px(2.)).children(touchpad).children(webcam).child(win_fn).children(usb))
            .child(control_row(Icon::Lock, s.fn_lock, tr.fn_lock, Some(tr.fn_lock_info), fn_lock))
    }
}
