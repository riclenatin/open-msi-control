use gpui::{
    Bounds, Context, Hitbox, HitboxBehavior, KeyDownEvent, SharedString, Window, canvas, div, fill, point,
    prelude::*, px, relative, size, transparent_black,
};

use crate::app::{BATTERY_MIN, Drag, MsiApp};
use crate::assets::Icon;
use crate::motion::{self, Spec};
use crate::theme::theme;
use crate::ui::{DragEvent, Width, card, mono, register_drag, segmented};

const PRESETS: [u8; 3] = [100, 80, 60];
const MARKS: [u8; 4] = [40, 60, 80, 100];

fn frac(v: u8) -> f32 {
    f32::from(v.saturating_sub(BATTERY_MIN)) / f32::from(100 - BATTERY_MIN)
}

impl MsiApp {
    pub(crate) fn render_battery(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = theme();
        let value = self.battery_value();
        let ring = self.keyboard_nav;
        let dragging = self.drag == Some(Drag::Battery);
        let pos = self.transitions.animate(
            ("battery", 0),
            frac(value),
            if dragging { Spec::movement(std::time::Duration::ZERO) } else { Spec::value(motion::VALUE) },
        );

        let on_drag = MsiApp::handler(cx, |this, e: DragEvent, _, cx| this.on_battery_drag(e, cx));
        let track = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |bounds, hitbox: Hitbox, window, _| {
                register_drag(window, bounds, &hitbox, dragging, on_drag);
                let mid = bounds.center().y;
                let rail = Bounds::new(point(bounds.left(), mid - px(3.)), size(bounds.size.width, px(6.)));
                window.paint_quad(fill(rail, t.raised).corner_radii(px(3.)));
                let filled = Bounds::new(rail.origin, size(rail.size.width * pos, rail.size.height));
                window.paint_quad(fill(filled, t.accent).corner_radii(px(3.)));
                let knob_c = point(bounds.left() + bounds.size.width * pos, mid);
                let knob = Bounds::centered_at(knob_c, size(px(20.), px(20.)));
                window.paint_quad(fill(knob, t.text).corner_radii(px(10.)));
                let core = Bounds::centered_at(knob_c, size(px(8.), px(8.)));
                window.paint_quad(fill(core, t.accent).corner_radii(px(4.)));
            },
        )
        .size_full();

        let slider = div()
            .id("battery-slider")
            .tab_index(0)
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(10.))
            .py(px(4.))
            .rounded(px(12.))
            .border_2()
            .border_color(transparent_black())
            .when(ring, |d| d.focus(|st| st.border_color(t.focus)))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let step: i16 = if e.keystroke.modifiers.shift { 5 } else { 1 };
                let cur = i16::from(this.battery_value());
                let next = match e.keystroke.key.as_str() {
                    "left" | "down" => cur - step,
                    "right" | "up" => cur + step,
                    "home" => i16::from(BATTERY_MIN),
                    "end" => 100,
                    _ => return,
                };
                cx.stop_propagation();
                this.keyboard_nav = true;
                this.snap_frame = true;
                this.set_battery(next.clamp(0, 100) as u8, true, cx);
            }))
            .child(div().h(px(24.)).child(track))
            .child(div().relative().h(px(14.)).children(MARKS.map(|m| {
                div()
                    .absolute()
                    .left(relative(frac(m)))
                    .ml(px(-16.))
                    .w(px(32.))
                    .flex()
                    .justify_center()
                    .child(mono(format!("{m}")).text_size(px(11.)).text_color(t.text_faint))
            })));

        let preset = PRESETS.iter().position(|&p| p == value);
        let preset_at = preset.map_or(0.0, |i| {
            self.transitions.animate(("battery-preset", 0), i as f32, Spec::movement(motion::MOVE))
        });
        let presets = segmented(
            "battery-presets",
            &[SharedString::from("100 %"), SharedString::from("80 %"), SharedString::from("60 %")],
            preset,
            preset_at,
            Width::Content,
            ring,
            window,
            MsiApp::handler(cx, |this, (i, kb): (usize, bool), _, cx| {
                this.snap_frame |= kb;
                this.set_battery(PRESETS[i], false, cx);
            }),
        );

        let readout =
            if value >= 100 { "до полного".to_owned() } else { format!("стоп на {value}%") };
        card(
            Icon::Battery,
            "Батарея",
            Some(mono(readout).text_size(px(12.)).text_color(t.accent).into_any_element()),
        )
        .child(slider)
        .child(
            div().flex().items_center().gap(px(10.)).child(presets).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("60–80 % бережнее к батарее"),
            ),
        )
    }
}
