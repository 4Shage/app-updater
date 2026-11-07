use egui::{vec2, Color32, Context, FontFamily, FontId, Frame, Margin, RichText, Rounding, TextStyle, Ui};

pub const GREEN: Color32 = Color32::from_rgb(80, 200, 120);
pub const ORANGE: Color32 = Color32::from_rgb(240, 170, 60);
pub const RED: Color32 = Color32::from_rgb(235, 90, 90);
pub const GRAY: Color32 = Color32::from_rgb(150, 150, 160);

pub fn apply_style(ctx: &Context) {
    ctx.style_mut(|style| {
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.interact_size.y = 28.0;
        style.spacing.window_margin = Margin::same(14.0);

        let rounding = Rounding::same(6.0);
        let widgets = &mut style.visuals.widgets;
        for visuals in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            visuals.rounding = rounding;
        }

        let sizes = [
            (TextStyle::Heading, 22.0),
            (TextStyle::Body, 15.0),
            (TextStyle::Button, 15.0),
            (TextStyle::Small, 12.5),
            (TextStyle::Monospace, 14.0),
        ];
        for (text_style, size) in sizes {
            let family = if text_style == TextStyle::Monospace {
                FontFamily::Monospace
            } else {
                FontFamily::Proportional
            };
            style.text_styles.insert(text_style, FontId::new(size, family));
        }
    });
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::group(ui.style())
        .inner_margin(12.0)
        .rounding(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn badge(ui: &mut Ui, text: &str, color: Color32) {
    Frame::none()
        .fill(color.gamma_multiply(0.2))
        .rounding(10.0)
        .inner_margin(Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).small().strong());
        });
}
