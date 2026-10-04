//! The front end's look: Soldat's interface font (`play-regular.ttf`), dark translucent panels
//! with an orange accent, larger text, all scaled with the window's height.

use egui::epaint::CornerRadius;
use egui::{
    Button, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Stroke, TextStyle,
};
use std::sync::Arc;

/// Behind the menus.
pub fn background() -> gfx2d::Color {
    gfx2d::rgb(22, 26, 33)
}

pub const ACCENT: Color32 = Color32::from_rgb(214, 132, 48);
pub const TEXT: Color32 = Color32::from_rgb(226, 228, 236);
pub const DIM: Color32 = Color32::from_rgb(150, 156, 170);
pub const WARNING: Color32 = Color32::from_rgb(240, 110, 100);

/// The menus are laid out for a window this many points high.
const DESIGN_HEIGHT: f32 = 720.0;

pub fn apply(ctx: &egui::Context, font: Option<Vec<u8>>) {
    let mut fonts = FontDefinitions::default();
    if let Some(font) = font {
        fonts
            .font_data
            .insert("play".to_string(), Arc::new(FontData::from_owned(font)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "play".to_string());
    }
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(30.0)),
            (TextStyle::Body, FontId::proportional(17.0)),
            (TextStyle::Button, FontId::proportional(18.0)),
            (TextStyle::Small, FontId::proportional(13.0)),
            (TextStyle::Monospace, FontId::monospace(15.0)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 6.0);
        style.spacing.interact_size.y = 28.0;
        style.spacing.slider_width = 220.0;
        // a game's menus: text is read, not selected (text fields still are)
        style.interaction.selectable_labels = false;
        style.interaction.multi_widget_text_select = false;

        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = Color32::from_rgba_unmultiplied(28, 33, 42, 240);
        v.window_fill = Color32::from_rgba_unmultiplied(28, 33, 42, 245);
        v.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(70, 76, 90));
        v.window_corner_radius = CornerRadius::same(8);
        v.extreme_bg_color = Color32::from_rgb(16, 19, 24);
        v.faint_bg_color = Color32::from_rgb(34, 40, 50);
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = ACCENT.linear_multiply(0.6);
        v.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(255, 220, 170));
        let radius = CornerRadius::same(4);
        let w = &mut v.widgets;
        w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.noninteractive.corner_radius = radius;
        w.inactive.bg_fill = Color32::from_rgb(44, 50, 62);
        w.inactive.weak_bg_fill = Color32::from_rgb(44, 50, 62);
        w.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.inactive.corner_radius = radius;
        w.hovered.bg_fill = Color32::from_rgb(78, 64, 46);
        w.hovered.weak_bg_fill = Color32::from_rgb(78, 64, 46);
        w.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
        w.hovered.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
        w.hovered.corner_radius = radius;
        w.active.bg_fill = ACCENT.linear_multiply(0.8);
        w.active.weak_bg_fill = ACCENT.linear_multiply(0.8);
        w.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
        w.active.corner_radius = radius;
        w.open.corner_radius = radius;
    });
}

/// The menus scale with the window: laid out for 720 points high.
pub fn scale(ctx: &egui::Context, screen_height: f32, dpi: f32) {
    let zoom = (screen_height / dpi / DESIGN_HEIGHT).clamp(0.75, 3.0);
    if (ctx.zoom_factor() - zoom).abs() > 0.01 {
        ctx.set_zoom_factor(zoom);
    }
}

/// The screen's main action: filled with the accent.
pub fn primary(text: &str, size: f32) -> Button<'static> {
    Button::new(
        RichText::new(text)
            .size(size)
            .strong()
            .color(Color32::WHITE),
    )
    .fill(ACCENT.linear_multiply(0.85))
    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 196, 130)))
}
