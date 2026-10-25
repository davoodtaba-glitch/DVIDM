//! DV Internet Download Manager, GPUI Kit interface.
//!
//! Shares the download engine, preferences and queue scheduler with the egui
//! build (`src/main.rs`) through `src/core`. Only this file and the modules
//! beside it know about GPUI.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "../clipboard.rs"]
mod clipboard;
#[path = "../core/mod.rs"]
mod core;
#[path = "../engine/mod.rs"]
mod engine;
/// `crate::format` is the shared formatter module from `core`, not a copy.
pub(crate) use core::format;
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;

mod app;
mod downloads;
mod logs;
mod settings;
mod shell;
mod sidebar;
mod theme;
mod toast;

use gpui_kit::*;
use gpui_kit::component::{Theme, ThemeMode};

/// Switch the whole component tree between the light and dark themes.
pub(crate) fn apply_theme(dark: bool, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    // Lift the default hover shade so the toolbar controls react visibly.
    Theme::update(cx, |theme| {
        let accent = theme.colors.accent;
        theme.tokens.button_hover = crate::theme::with_alpha(accent, 0.24).into();
        theme.tokens.button_secondary_hover = crate::theme::with_alpha(accent, 0.32).into();
    });
}

/// The theme's own family, restored when the preference is cleared.
static DEFAULT_FONT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Install the saved interface font; an empty name resets the system default.
pub(crate) fn apply_font(name: &str, cx: &mut App) {
    let family = if name.trim().is_empty() {
        DEFAULT_FONT.get().cloned().unwrap_or_default()
    } else {
        name.trim().to_string()
    };
    if family.is_empty() {
        return;
    }
    let family: SharedString = family.into();
    Theme::update(cx, |theme| {
        theme.font_family = family;
    });
}

fn main() {
    let Some(_single_instance) = core::sys::acquire_single_instance() else {
        return;
    };

    core::log::init();
    core::log::write(format!("exe: {:?}", std::env::current_exe()));
    core::sys::refresh_autostart();
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        core::log::write(format!("PANIC: {info}"));
        default_hook(info);
    }));

    // Icon SVGs are loaded through the asset source; without it every
    // `IconName` fails to load and buttons paint no icon at all.
    gpui_kit::application()
        // The full catalogue is needed for the list and clock icons.
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
        gpui_kit::init(cx);
        // Remember the theme family before a preference can replace it.
        let initial = Theme::global(cx).font_family.to_string();
        let _ = DEFAULT_FONT.set(initial);

        let title = format!(
            "DV Internet Download Manager {}",
            env!("CARGO_PKG_VERSION")
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(60.), px(30.)),
                size: size(px(1500.), px(950.)),
            })),
            window_min_size: Some(size(px(940.), px(560.))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            show: true,
            focus: true,
            kind: WindowKind::Normal,
            is_resizable: true,
            is_minimizable: true,
            ..Default::default()
        };

        match gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| app::MainView::new(window, cx))
        }) {
            Ok((handle, entity)) => {
                entity.update(cx, |view, _cx| view.window_handle = Some(handle));
            }
            Err(err) => {
                eprintln!("could not open the main window: {err:#}");
            }
        }
    });
}
