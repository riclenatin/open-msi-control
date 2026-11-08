use gpui::{Context, Div, SharedString, Window, div, prelude::*, px};

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::i18n::Lang;
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{SwitchRow, Width, card, icon, info, inset, segmented};

/// Label row with a trailing control (backlight level, language).
fn control_row(
    glyph: Icon,
    active: bool,
    label: &'static str,
    explain: Option<&'static str>,
    control: impl IntoElement,
) -> Div {
    let t = theme();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .pl(px(12.))
        .child(icon(glyph, 15., if active { t.accent } else { t.text_muted }))
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .gap(px(4.))
                .child(div().text_size(px(13.)).child(label))
                .children(explain.map(|text| info(label, text))),
        )
        .child(control)
}

impl MsiApp {
    pub(crate) fn render_system(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
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

        let lang_index = Lang::ALL.iter().position(|&l| l == self.lang).unwrap_or(0);
        let lang_at = self.transitions.animate(("lang", 0), lang_index as f32, Spec::movement(motion::MOVE));
        let lang = segmented(
            "language",
            &[SharedString::from("RU"), SharedString::from("EN")],
            Some(lang_index),
            lang_at,
            Width::Content,
            ring,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.snap_frame |= kb;
                this.set_lang(Lang::ALL[i], cx);
            }),
        );

        let mut toggle = |key: &'static str, on: bool| -> f32 {
            self.transitions.animate((key, 0), f32::from(u8::from(on)), Spec::movement(motion::TOGGLE))
        };
        let row = |id: &'static str, icon, label, explain, on, thumb_at, on_toggle| {
            SwitchRow {
                id: id.into(),
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
        let webcam = s.webcam.map(|on| {
            let at = toggle("webcam", on);
            row(
                "webcam",
                Icon::Webcam,
                tr.webcam,
                tr.webcam_info,
                on,
                at,
                MsiApp::handler(cx, |this, (v, kb): (bool, bool), _, cx| this.set_webcam(v, kb, cx)),
            )
        });
        let win_fn_at = toggle("winfn", s.win_fn_swapped);
        let win_fn = row(
            "win-fn",
            Icon::Swap,
            tr.win_fn,
            tr.win_fn_info,
            s.win_fn_swapped,
            win_fn_at,
            MsiApp::handler(cx, |this, (v, kb): (bool, bool), _, cx| this.set_win_fn(v, kb, cx)),
        );
        let usb = s.usb_share.map(|on| {
            let at = toggle("usb", on);
            row(
                "usb-share",
                Icon::Usb,
                tr.usb_share,
                tr.usb_share_info,
                on,
                at,
                MsiApp::handler(cx, |this, (v, kb): (bool, bool), _, cx| this.set_usb_share(v, kb, cx)),
            )
        });

        card(Icon::Sliders, tr.system, None, None)
            .child(control_row(Icon::Keyboard, level > 0, tr.backlight, Some(tr.backlight_info), kbd))
            .child(inset().p(px(2.)).children(webcam).child(win_fn).children(usb))
            .child(control_row(Icon::Globe, false, tr.language, None, lang))
    }
}
