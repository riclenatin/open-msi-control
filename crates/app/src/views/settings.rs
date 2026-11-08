use gpui::{Context, SharedString, Window, prelude::*, px};

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::i18n::Lang;
use crate::motion::{self, Spec};
use crate::ui::{SwitchRow, Width, card, control_row, inset, segmented};

impl MsiApp {
    pub(crate) fn render_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let tr = self.tr();
        let ring = self.keyboard_nav;

        // Autostart state is queried from Task Scheduler; show it off until the answer arrives.
        let autostart = self.autostart.unwrap_or(false);
        let autostart_at = self.transitions.animate(
            ("autostart", 0),
            f32::from(u8::from(autostart)),
            Spec::movement(motion::TOGGLE),
        );
        let tray_at = self.transitions.animate(
            ("close-to-tray", 0),
            f32::from(u8::from(self.close_to_tray)),
            Spec::movement(motion::TOGGLE),
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

        card(Icon::Gear, tr.app, None, None)
            .child(
                inset()
                    .p(px(2.))
                    .child(
                        SwitchRow {
                            id: "autostart".into(),
                            icon: Icon::Power,
                            label: tr.autostart,
                            description: None,
                            explain: Some(tr.autostart_info),
                            on: autostart,
                            thumb_at: autostart_at,
                            ring,
                            on_toggle: MsiApp::handler(cx, |this, (on, kb): (bool, bool), _, cx| {
                                this.set_autostart(on, kb, cx)
                            }),
                        }
                        .render(),
                    )
                    .child(
                        SwitchRow {
                            id: "close-to-tray".into(),
                            icon: Icon::Tray,
                            label: tr.close_to_tray,
                            description: None,
                            explain: Some(tr.close_to_tray_info),
                            on: self.close_to_tray,
                            thumb_at: tray_at,
                            ring,
                            on_toggle: MsiApp::handler(cx, |this, (on, kb): (bool, bool), _, cx| {
                                this.set_close_to_tray(on, kb, cx)
                            }),
                        }
                        .render(),
                    ),
            )
            .child(control_row(Icon::Globe, false, tr.language, None, lang))
    }
}
