//! Colour palette and the shared drawing helpers used by every page.

use super::Page;
use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, RichText, Stroke, Vec2};
use std::time::Duration;

pub const BLUE: Color32 = Color32::from_rgb(45, 109, 246);
pub const INK: Color32 = Color32::from_rgb(29, 41, 61);
pub const MUTED: Color32 = Color32::from_rgb(133, 145, 164);
pub const LINE: Color32 = Color32::from_rgb(232, 237, 245);
pub const PALE: Color32 = Color32::from_rgb(245, 248, 254);
pub const GREEN: Color32 = Color32::from_rgb(39, 173, 123);

pub(super) fn theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in ["C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/simhei.ttf"] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "Chinese".into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Chinese".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("Chinese".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = Color32::WHITE;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.selection.bg_fill = BLUE;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.inactive.bg_fill = PALE;
    style.visuals.widgets.inactive.weak_bg_fill = PALE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(7);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(232, 239, 255);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(7);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(7);
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 9.0);
    style.spacing.interact_size.y = 34.0;
    ctx.set_style(style);
}
pub(super) fn primary(ui: &mut egui::Ui, label: &str, size: [f32; 2], enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .size(14.0)
                .strong()
                .color(Color32::WHITE),
        )
        .fill(BLUE)
        .corner_radius(8)
        .min_size(Vec2::from(size)),
    )
    .clicked()
}
pub(super) fn secondary(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(12.0).color(INK))
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, LINE))
            .corner_radius(7)
            .min_size(Vec2::new(ui.available_width(), 34.0)),
    )
    .clicked()
}
pub(super) fn card(ui: &mut egui::Ui, width: f32, content: impl FnOnce(&mut egui::Ui)) {
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(12)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width((width - 40.0).max(40.0));
            ui.with_layout(Layout::top_down(Align::Min), content);
        });
}
pub(super) fn nav_icon(ui: &egui::Ui, page: Page, c: egui::Pos2, color: Color32) {
    let p = ui.painter();
    let stroke = Stroke::new(1.6_f32, color);
    match page {
        Page::Connect => {
            for sign in [-1.0, 1.0] {
                let y = sign * 4.0;
                let x = sign * 7.0;
                p.line_segment([c + Vec2::new(-7.0, y), c + Vec2::new(7.0, y)], stroke);
                p.line_segment(
                    [c + Vec2::new(x, y), c + Vec2::new(x - sign * 4.0, y - 4.0)],
                    stroke,
                );
            }
        }
        Page::Nodes => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    p.rect_stroke(
                        egui::Rect::from_center_size(
                            c + Vec2::new(x * 4.5, y * 4.5),
                            Vec2::splat(6.0),
                        ),
                        1,
                        stroke,
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        Page::Subscriptions => {
            p.circle_stroke(c, 7.0, stroke);
            p.line_segment([c + Vec2::new(7.0, -2.0), c + Vec2::new(4.0, -6.0)], stroke);
            p.line_segment(
                [c + Vec2::new(7.0, -2.0), c + Vec2::new(10.0, -6.0)],
                stroke,
            );
        }
        Page::Logs => {
            for y in [-5.0, 0.0, 5.0] {
                p.line_segment(
                    [
                        c + Vec2::new(-7.0, y),
                        c + Vec2::new(if y == 5.0 { 3.0 } else { 7.0 }, y),
                    ],
                    stroke,
                );
            }
        }
        Page::Settings => {
            p.circle_stroke(c, 5.0, stroke);
            p.circle_stroke(c, 1.5, stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let v = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + v * 6.0, c + v * 8.5], stroke);
            }
        }
        Page::About => {
            p.circle_stroke(c, 8.0, stroke);
            p.line_segment([c + Vec2::new(0.0, -1.0), c + Vec2::new(0.0, 4.0)], stroke);
            p.circle_filled(c + Vec2::new(0.0, -4.0), 1.0, color);
        }
    }
}
pub(super) fn pill(ui: &mut egui::Ui, label: &str, bg: Color32, color: Color32) {
    let font = egui::FontId::proportional(10.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), color);
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(galley.size().x + 18.0, 22.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 5, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
}
pub(super) fn metric(ui: &mut egui::Ui, label: &str, value: String) {
    let previous = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = 20.0;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(12.0).color(MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(value).size(13.0).strong().color(INK));
        });
    });
    ui.spacing_mut().interact_size.y = previous;
}
pub(super) fn empty(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.add_space(90.0);
    ui.vertical_centered(|ui| {
        logo(ui, 60.0);
        ui.add_space(20.0);
        ui.label(RichText::new(title).size(21.0).strong().color(INK));
        ui.add_space(8.0);
        ui.label(RichText::new(detail).size(13.0).color(MUTED));
    });
}
pub(super) fn logo(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same((size / 4.0) as u8), BLUE);
    let c = rect.center();
    let points = vec![
        c + Vec2::new(-size * 0.22, size * 0.04),
        c + Vec2::new(size * 0.08, -size * 0.27),
        c + Vec2::new(size * 0.01, -size * 0.02),
        c + Vec2::new(size * 0.22, -size * 0.04),
        c + Vec2::new(-size * 0.08, size * 0.27),
        c + Vec2::new(-size * 0.01, size * 0.02),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        Color32::WHITE,
        Stroke::NONE,
    ));
}
pub(super) fn network_art(ui: &mut egui::Ui, active: bool) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 88.0), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    p.circle_filled(c, 51.0, Color32::from_rgb(247, 250, 255));
    p.circle_stroke(
        c,
        41.0,
        Stroke::new(1.0_f32, Color32::from_rgb(212, 227, 255)),
    );
    p.circle_stroke(
        c,
        28.0,
        Stroke::new(1.0_f32, Color32::from_rgb(212, 227, 255)),
    );
    p.line_segment(
        [c + Vec2::new(-41.0, 0.0), c + Vec2::new(41.0, 0.0)],
        Stroke::new(1.0_f32, Color32::from_rgb(212, 227, 255)),
    );
    for x in [-1.0, 1.0] {
        let node = c + Vec2::new(x * 105.0, 0.0);
        p.line_segment(
            [c + Vec2::new(x * 50.0, 0.0), node],
            Stroke::new(1.5_f32, Color32::from_rgb(212, 227, 255)),
        );
        p.circle_filled(node, 18.0, Color32::from_rgb(237, 244, 255));
        p.circle_filled(node, 5.0, BLUE);
    }
    p.circle_filled(c, 18.0, BLUE);
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        if active { "✓" } else { "↗" },
        egui::FontId::proportional(24.0),
        Color32::WHITE,
    );
    p.circle_filled(
        c + Vec2::new(31.0, -27.0),
        5.0,
        if active {
            GREEN
        } else {
            Color32::from_rgb(137, 175, 250)
        },
    );
}
pub(super) fn bytes(n: f64) -> String {
    if n >= 1048576.0 {
        format!("{:.1} MB", n / 1048576.0)
    } else if n >= 1024.0 {
        format!("{:.1} KB", n / 1024.0)
    } else {
        format!("{n:.0} B")
    }
}
pub(super) fn duration(d: Duration) -> String {
    let n = d.as_secs();
    format!("{:02}:{:02}:{:02}", n / 3600, (n / 60) % 60, n % 60)
}
