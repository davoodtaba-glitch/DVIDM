//! Windows tray icon and native window handle for the GPUI Kit build.
//!
//! GPUI itself has no tray API, so the notification-area icon comes from the
//! `tray-icon` crate (the same crate the egui build uses). Its events arrive on
//! Windows' message loop; two blocking threads forward them into an async
//! channel, which keeps the hidden application completely idle between events.
use crate::core::sys;
use crate::i18n::{t, tf, Key};
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Clone, Copy, Debug)]
pub(crate) enum TrayMsg {
    Open,
    Exit,
}

pub(crate) struct Tray {
    icon: TrayIcon,
    open: MenuItem,
    exit: MenuItem,
    rx: Option<smol::channel::Receiver<TrayMsg>>,
}

impl Tray {
    /// Build the icon and start the two forwarder threads.
    pub(crate) fn install(quitting: Arc<AtomicBool>) -> Option<Self> {
        let open_id = MenuId::new("open");
        let exit_id = MenuId::new("exit");
        let open = MenuItem::with_id(open_id.clone(), t(Key::TrayOpen), true, None);
        let exit = MenuItem::with_id(exit_id.clone(), t(Key::TrayExit), true, None);
        let menu = Menu::new();
        let _ = menu.append(&open);
        let _ = menu.append(&exit);
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tf(Key::TrayTooltip, &[env!("CARGO_PKG_VERSION")]))
            .with_icon(make_icon()?)
            .build()
            .ok()?;

        let (tx, rx) = smol::channel::unbounded();
        let click_tx = tx.clone();
        std::thread::Builder::new()
            .name("tray-events".into())
            .spawn(move || loop {
                let Ok(ev) = TrayIconEvent::receiver().recv() else {
                    return;
                };
                let restore = match ev {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => true,
                    _ => false,
                };
                if restore && click_tx.send_blocking(TrayMsg::Open).is_err() {
                    return;
                }
            })
            .ok()?;

        let menu_tx = tx;
        let quit_handle = quitting;
        std::thread::Builder::new()
            .name("tray-menu".into())
            .spawn(move || loop {
                let Ok(ev) = MenuEvent::receiver().recv() else {
                    return;
                };
                if ev.id == open_id {
                    if menu_tx.send_blocking(TrayMsg::Open).is_err() {
                        return;
                    }
                } else if ev.id == exit_id {
                    // Set before the message arrives so the close handler lets
                    // the window go instead of hiding it again.
                    quit_handle.store(true, Ordering::SeqCst);
                    let _ = menu_tx.send_blocking(TrayMsg::Exit);
                    return;
                }
            })
            .ok()?;

        Some(Self {
            icon,
            open,
            exit,
            rx: Some(rx),
        })
    }

    pub(crate) fn relabel(&self) {
        self.open.set_text(t(Key::TrayOpen));
        self.exit.set_text(t(Key::TrayExit));
        let _ = self
            .icon
            .set_tooltip(Some(tf(Key::TrayTooltip, &[env!("CARGO_PKG_VERSION")])));
    }

    /// Hand the event stream to the view task; the icon keeps living here.
    pub(crate) fn take_rx(&mut self) -> Option<smol::channel::Receiver<TrayMsg>> {
        self.rx.take()
    }
}

fn make_icon() -> Option<tray_icon::Icon> {
    let source = image::load_from_memory(include_bytes!("../../resource/icon.png"))
        .ok()?
        .to_rgba8();
    let small = image::imageops::resize(&source, 32, 32, image::imageops::FilterType::Lanczos3);
    tray_icon::Icon::from_rgba(small.into_raw(), 32, 32).ok()
}

/// Native handle of the GPUI window, captured once after it is created.
pub(crate) fn hwnd(window: &Window) -> isize {
    // `Window` has an inherent `window_handle` returning a GPUI handle, so the
    // raw-window-handle trait must be named explicitly here.
    match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(w)) => w.hwnd.get(),
        _ => 0,
    }
}

/// Remember the handle as soon as the window exists so tray events can reach
/// it even while the window is hidden.
pub(crate) fn remember_hwnd(window: &Window, slot: &sys::HwndSlot) {
    let h = hwnd(window);
    if h != 0 {
        slot.set(h);
    }
}

/// Wait for the Win32 show state to settle before reporting it.
pub(crate) fn visible_now(slot: &sys::HwndSlot) -> bool {
    let hwnd = slot.get();
    hwnd != 0 && sys::is_visible(hwnd) && !sys::is_minimized(hwnd)
}
