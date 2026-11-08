use gpui::{Context, SharedString, Window, div, prelude::*, px};
use msi_acpi::Scenario;

use crate::app::MsiApp;
use crate::assets::Icon;
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{Tone, Width, card, icon, mono, pill, segmented};

fn describe(s: Scenario) -> (Icon, &'static str, &'static str) {
    match s {
        Scenario::ExtremePerformance => (Icon::Rocket, "Экстрим", "Максимум мощности CPU и GPU"),
        Scenario::Balanced => (Icon::Scales, "Баланс", "Производительность и шум поровну"),
        Scenario::Silent => (Icon::Moon, "Тихий", "Вентиляторы ограничены прошивкой"),
        Scenario::SuperBattery => (Icon::Leaf, "Батарея", "Минимум энергопотребления"),
    }
}

impl MsiApp {
    pub(crate) fn render_mode(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let Some(s) = self.snap.clone() else { return div() };
        let current = s.scenario.and_then(|c| Scenario::ALL.iter().position(|&x| x == c));
        let at = current.map_or(0.0, |i| {
            self.transitions.animate(("scenario", 0), i as f32, Spec::movement(motion::MOVE))
        });
        let labels = Scenario::ALL.map(|sc| SharedString::from(describe(sc).1));

        let picker = segmented(
            "scenario",
            &labels,
            current,
            at,
            Width::Fill,
            self.keyboard_nav,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.set_scenario(Scenario::ALL[i], kb, cx);
            }),
        );

        let hint = match s.scenario {
            Some(sc) => {
                let (glyph, _, text) = describe(sc);
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(4.))
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child(icon(glyph, 13., t.accent))
                    .child(text)
            }
            None => div().flex().child(pill(Tone::Warning, "Нестандартный режим — выберите сценарий")),
        };

        card(
            Icon::Gauge,
            "Режим",
            Some(
                mono(format!("0x{:02X} · 0x{:02X}", s.shift_raw, s.fan_mode_raw))
                    .text_size(px(11.))
                    .text_color(t.text_faint)
                    .into_any_element(),
            ),
        )
        .child(picker)
        .child(hint)
    }
}
