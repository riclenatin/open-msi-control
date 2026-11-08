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
        // Scenarios the model cannot express are not offered; `i` indexes `Scenario::ALL`.
        let shown: Vec<(usize, Scenario)> =
            Scenario::ALL.into_iter().enumerate().filter(|&(_, sc)| s.scenarios.contains(sc)).collect();
        if shown.is_empty() {
            return div();
        }
        let current = s.scenario.and_then(|c| shown.iter().position(|&(_, x)| x == c));
        let at = current.map_or(0.0, |i| {
            self.transitions.animate(("scenario", 0), i as f32, Spec::movement(motion::MOVE))
        });

        let labels: Vec<SharedString> = shown.iter().map(|&(i, _)| tr.scenarios[i].into()).collect();
        let hint = match current.map(|i| shown[i].0) {
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
        let picker = segmented(
            "scenario",
            &labels,
            current,
            at,
            Width::Fill,
            self.keyboard_nav,
            window,
            MsiApp::handler(cx, move |this, (i, kb): (usize, bool), _, cx| {
                this.set_scenario(shown[i].1, kb, cx);
            }),
        );

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
