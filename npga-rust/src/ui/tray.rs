//! System tray icon and the native window icon.

use eframe::egui;

#[cfg(windows)]
pub(super) fn tray() -> anyhow::Result<(
    tray_icon::TrayIcon,
    (
        tray_icon::menu::MenuId,
        tray_icon::menu::MenuId,
        tray_icon::menu::MenuId,
    ),
)> {
    use tray_icon::{
        Icon, TrayIconBuilder,
        menu::{Menu, MenuItem},
    };
    let menu = Menu::new();
    let show = MenuItem::new("显示主界面", true, None);
    let stop = MenuItem::new("停止加速", true, None);
    let quit = MenuItem::new("退出 NPGA", true, None);
    menu.append_items(&[&show, &stop, &quit])?;
    let rgba = native_icon().rgba.clone();
    let tray = TrayIconBuilder::new()
        .with_tooltip("NPGA Rust · 游戏加速")
        .with_icon(Icon::from_rgba(rgba, 32, 32)?)
        .with_menu(Box::new(menu))
        .build()?;
    Ok((
        tray,
        (show.id().clone(), stop.id().clone(), quit.id().clone()),
    ))
}

pub fn native_icon() -> std::sync::Arc<egui::IconData> {
    let polygon = [
        (-0.22, 0.04),
        (0.08, -0.27),
        (0.01, -0.02),
        (0.22, -0.04),
        (-0.08, 0.27),
        (-0.01, 0.02),
    ];
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let px = (x as f32 + 0.5) / 32.0 - 0.5;
            let py = (y as f32 + 0.5) / 32.0 - 0.5;
            let mut white = false;
            for i in 0..polygon.len() {
                let (ax, ay) = polygon[i];
                let (bx, by) = polygon[(i + 1) % polygon.len()];
                if (ay > py) != (by > py) && px < (bx - ax) * (py - ay) / (by - ay) + ax {
                    white = !white;
                }
            }
            let i = (y * 32 + x) * 4;
            let alpha = if (px.abs() - 0.25).max(0.0).hypot((py.abs() - 0.25).max(0.0)) > 0.25 {
                0
            } else {
                255
            };
            let color = if white {
                [255, 255, 255, alpha]
            } else {
                [45, 109, 246, alpha]
            };
            rgba[i..i + 4].copy_from_slice(&color);
        }
    }
    std::sync::Arc::new(egui::IconData {
        rgba,
        width: 32,
        height: 32,
    })
}
