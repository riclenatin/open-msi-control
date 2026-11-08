use gpui::{Context, SharedString, Window, prelude::*, px};

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::i18n::Lang;
use crate::motion::{self, Spec};
use crate::ui::{Handler, SwitchRow, Width, card, control_row, inset, segmented};

type Setter = fn(&mut MsiApp, bool, bool, &mut Context<MsiApp>);

impl MsiApp {
    pub(crate) fn render_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let tr = self.tr();
        let ring = self.keyboard_nav;

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
        let s = self.settings.clone();
        // Autostart state is queried from Task Scheduler; show it off until the answer arrives.
        let rows = [
            row(
                "autostart",
                Icon::Power,
                tr.autostart,
                tr.autostart_info,
                self.autostart.unwrap_or(false),
                MsiApp::set_autostart,
            ),
            row(
                "close-to-tray",
                Icon::Tray,
                tr.close_to_tray,
                tr.close_to_tray_info,
                s.close_to_tray,
                MsiApp::set_close_to_tray,
            ),
            row("restore", Icon::Reset, tr.restore, tr.restore_info, s.restore, MsiApp::set_restore),
            row(
                "per-source",
                Icon::Battery,
                tr.per_source,
                tr.per_source_info,
                s.per_power_source,
                MsiApp::set_per_source,
            ),
            row(
                "updates",
                Icon::Download,
                tr.check_updates,
                tr.check_updates_info,
                s.check_updates,
                MsiApp::set_check_updates,
            ),
        ];

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

        card(Icon::Gear, tr.app, None, None).child(inset().p(px(2.)).children(rows)).child(control_row(
            Icon::Globe,
            false,
            tr.language,
            None,
            lang,
        ))
    }
}
