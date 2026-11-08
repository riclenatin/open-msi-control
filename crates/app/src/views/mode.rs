use gpui::{Context, SharedString, Window, div, prelude::*, px};
use msi_acpi::Scenario;

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{Tone, Width, card, icon, mono, pill, segmented};

const ICONS: [Icon; 4] = [Icon::Rocket, Icon::Scales, Icon::Moon, Icon::Leaf];

impl MsiApp {
    pub(crate) fn render_mode(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let tr = self.tr();
        let Some(s) = self.snap.clone() else { return div() };
        let current = s.scenario.and_then(|c| Scenario::ALL.iter().position(|&x| x == c));
        let at = current.map_or(0.0, |i| {
            self.transitions.animate(("scenario", 0), i as f32, Spec::movement(motion::MOVE))
        });

        let picker = segmented(
            "scenario",
            &tr.scenarios.map(SharedString::from),
            current,
            at,
            Width::Fill,
            self.keyboard_nav,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.set_scenario(Scenario::ALL[i], kb, cx);
            }),
        );

        let hint = match current {
            Some(i) => div()
                .flex()
                .items_start()
                .gap(px(6.))
                .px(px(4.))
                .child(div().pt(px(2.)).child(icon(ICONS[i], 13., t.accent)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(t.text_muted)
                        .child(tr.scenario_hints[i]),
                ),
            None => div().flex().child(pill(Tone::Warning, tr.mode_unknown)),
        };

        // With separate profiles, say which one edits go to.
        let source = self.settings.per_power_source.then(|| {
            mono(if self.on_ac { tr.profile_ac } else { tr.profile_battery })
                .text_size(px(11.))
                .text_color(t.text_faint)
                .into_any_element()
        });
        card(Icon::Gauge, tr.mode, Some(tr.mode_info), source).child(picker).child(hint)
    }
}
