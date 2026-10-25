//! The GPUI Kit window: layout, lifecycle, timers and command dispatch.
//!
//! Rendering only reads [`DownloadService`]; every user action becomes a
//! [`Command`]. The view owns the application's only polling loop: 250 ms while
//! the window is visible, 1 s while it is minimized or hidden, and no repaint
//! request at all when there is nothing on screen. That is what keeps the
//! tray-resident process idle.
use crate::clipboard::ClipImport;
use crate::core::{Command, DownloadService, Event};
use crate::engine::{Category, Snapshot, Status};
use crate::shell::{self, Tray, TrayMsg};
use crate::theme::{with_alpha, Palette};
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable as _, Icon, IconName};
use gpui_kit::component::WindowExt as _;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Downloads,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortCol {
    Num,
    Status,
    Name,
    Size,
    Progress,
    Speed,
    Eta,
    LastDate,
}

impl SortCol {
    pub(crate) fn key(self) -> &'static str {
        match self {
            SortCol::Num => "num",
            SortCol::Status => "status",
            SortCol::Name => "name",
            SortCol::Size => "size",
            SortCol::Progress => "progress",
            SortCol::Speed => "speed",
            SortCol::Eta => "eta",
            SortCol::LastDate => "last",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        match key {
            "num" => Some(SortCol::Num),
            "status" => Some(SortCol::Status),
            "name" => Some(SortCol::Name),
            "size" => Some(SortCol::Size),
            "progress" => Some(SortCol::Progress),
            "speed" => Some(SortCol::Speed),
            "eta" => Some(SortCol::Eta),
            "last" => Some(SortCol::LastDate),
            _ => None,
        }
    }
}

pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) error: bool,
}

/// Rows a confirmed delete will remove, and whether their files go too.
pub(crate) struct ConfirmDelete {
    pub(crate) ids: Vec<String>,
    pub(crate) delete_files: bool,
}

/// A checkbox drag: every row the pointer crosses gets `add`.
pub(crate) struct DragSelect {
    pub(crate) add: bool,
    pub(crate) start_id: String,
    pub(crate) dragging: bool,
}

pub(crate) struct MainView {
    pub(crate) svc: DownloadService,
    pub(crate) pal: Palette,
    pub(crate) page: Page,
    pub(crate) filter: Category,
    pub(crate) list_filter: Option<String>,
    pub(crate) selected: HashSet<String>,
    /// Active checkbox drag, if any.
    pub(crate) drag_select: Option<DragSelect>,
    /// "Keep the old file" choice in the redownload warning.
    pub(crate) redownload_keep: bool,
    /// Last debug heartbeat written from the tick loop.
    pub(crate) heartbeat_at: Instant,
    /// Separate debug-log window, when open.
    pub(crate) log_window: Option<AnyWindowHandle>,
    /// Navigation sidebar width in logical pixels.
    pub(crate) sidebar_width: f32,
    /// True while the splitter between the sidebar and the content is dragged.
    pub(crate) resizing_sidebar: bool,
    pub(crate) sort: Option<(SortCol, bool)>,
    pub(crate) url: Entity<InputState>,
    pub(crate) search: Entity<InputState>,
    /// Free-form speed limit entry on the settings page.
    pub(crate) limit_input: Entity<InputState>,
    /// The typed limit is in charge; no preset is selected.
    pub(crate) limit_custom: bool,
    /// Download whose properties dialog is open.
    pub(crate) props_id: Option<String>,
    /// Which property row was copied last, for the button's "Copied" state.
    pub(crate) props_copied: Option<String>,
    /// Editable source address in the properties dialog.
    pub(crate) props_url: Entity<InputState>,
    /// Why the last address change was rejected, shown under the field.
    pub(crate) props_url_error: Option<String>,
    /// Scroll position of the download table, shared with its scrollbar.
    pub(crate) row_scroll: UniformListScrollHandle,
    /// Which sidebar groups are expanded: library, categories, lists.
    pub(crate) nav_fold: [bool; 3],
    /// Mirrors the search field so filtering never needs the input entity.
    pub(crate) search_query: String,
    pub(crate) hwnd: crate::core::sys::HwndSlot,
    pub(crate) window_handle: Option<AnyWindowHandle>,
    pub(crate) quitting: Arc<AtomicBool>,
    pub(crate) tray: Option<Tray>,
    pub(crate) notice: Option<Notice>,
    /// Current borderless completion toast window.
    pub(crate) toast_window: Option<AnyWindowHandle>,
    /// Logo menu (user guide, about, update, exit) is open.
    pub(crate) logo_menu: bool,
    /// Parallel-download cap being edited in the list options dialog.
    pub(crate) list_opts_cap: u32,
    /// Label colour being edited in the list options dialog.
    pub(crate) list_opts_color: u8,
    /// Shutdown-at-end switch being edited in the list options dialog.
    pub(crate) list_opts_shutdown: bool,
    /// Whether the time window is enabled in the list options dialog.
    pub(crate) list_opts_sched: bool,
    /// Keystroke watchers on the dialog's clock fields, dropped on reopen.
    pub(crate) clock_subs: Vec<Subscription>,
    /// Links waiting for review, with the list they will be filed into.
    pub(crate) clip: Option<ClipImport>,
    pub(crate) clip_list: Option<String>,
    /// Start the links from the review window paused instead of immediately.
    pub(crate) clip_queued: bool,
    /// Delete confirmation waiting for the user.
    pub(crate) confirm_delete: Option<ConfirmDelete>,
    /// Pending machine shutdown requested by a list schedule.
    pub(crate) shutdown_at: Option<u64>,
    /// Last observed native visibility; drives the tick rate.
    pub(crate) visible: bool,
    /// URL batches posted by the Chrome extension.
    pub(crate) remote_rx: std::sync::mpsc::Receiver<Vec<String>>,
}

impl MainView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let svc = DownloadService::new();
        let dark = svc.prefs().theme_dark();
        let sidebar_width = svc.prefs().sidebar_width as f32;
        let sort = SortCol::from_key(svc.prefs().sort_col.trim())
            .map(|col| (col, svc.prefs().sort_desc));
        crate::apply_theme(dark, cx);

        let url = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://example.com/file.zip")
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search downloads…"));
        let limit_input = cx.new(|cx| InputState::new(window, cx).placeholder("KB/s"));
        let props_url = cx.new(|cx| InputState::new(window, cx).placeholder("https://…"));

        let hwnd = crate::core::sys::HwndSlot::new();
        shell::remember_hwnd(window, &hwnd);
        let quitting = Arc::new(AtomicBool::new(false));
        let tray = Tray::install(quitting.clone());

        let (remote_tx, remote_rx) = std::sync::mpsc::channel();
        let wake_slot = hwnd.clone();
        crate::core::server::start(
            remote_tx,
            std::sync::Arc::new(move || {
                // Surface the window from the HTTP thread; the tick loop then
                // picks the URLs up like any other input.
                let h = wake_slot.get();
                if h != 0 && !crate::core::sys::is_visible(h) {
                    crate::core::sys::restore(h);
                }
            }),
        );

        // The interface font is a preference shared with the egui build.
        let font = svc.prefs().font.clone();
        if !font.trim().is_empty() {
            crate::apply_font(&font, cx);
        }

        let mut view = Self {
            svc,
            pal: Palette::new(dark),
            page: Page::Downloads,
            filter: Category::All,
            list_filter: None,
            selected: HashSet::new(),
            drag_select: None,
            redownload_keep: true,
            heartbeat_at: Instant::now(),
            log_window: None,
            sidebar_width,
            resizing_sidebar: false,
            sort,
            url,
            search,
            limit_input,
            limit_custom: false,
            props_id: None,
            props_copied: None,
            props_url,
            props_url_error: None,
            row_scroll: UniformListScrollHandle::new(),
            nav_fold: [true, true, true],
            search_query: String::new(),
            hwnd,
            window_handle: None,
            quitting,
            tray,
            notice: None,
            toast_window: None,
            logo_menu: false,
            list_opts_cap: 0,
            list_opts_color: 0,
            list_opts_shutdown: false,
            list_opts_sched: false,
            clock_subs: Vec::new(),
            clip: None,
            clip_list: None,
            clip_queued: false,
            confirm_delete: None,
            shutdown_at: None,
            visible: true,
            remote_rx,
        };

        view.watch_inputs(window, cx);
        view.start_tick_loop(cx);
        view.start_tray_loop(cx);
        view.install_close_handler(window, cx);
        view
    }

    /// Enter in the URL field adds the download, like clicking the button.
    fn watch_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url.clone();
        cx.subscribe_in(&url, window, |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit(window, cx);
            } else {
                // Typing changes the disabled state of the Download button.
                cx.notify();
            }
        })
        .detach();

        let search = self.search.clone();
        cx.subscribe_in(&search, window, |this, _, ev: &InputEvent, _window, cx| {
            if matches!(ev, InputEvent::Change) {
                this.search_query = this.search.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        let limit = self.limit_input.clone();
        cx.subscribe_in(&limit, window, |this, _, ev: &InputEvent, _window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                // Preset mode owns the number: the field is disabled there, and
                // a late Blur must not quietly turn the preset into a custom one.
                if !this.limit_custom {
                    return;
                }
                let raw = this.limit_input.read(cx).value().trim().to_string();
                let kbps = if raw.is_empty() {
                    0
                } else {
                    raw.parse::<u64>().unwrap_or(0)
                };
                this.svc.apply(Command::SetLimitKbps(kbps));
                // A typed value is the custom limit, even if it matches a
                // preset number.
                this.limit_custom = kbps != 0;
                cx.notify();
            }
        })
        .detach();

        let props_url = self.props_url.clone();
        cx.subscribe_in(&props_url, window, |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                if let Some(id) = this.props_id.clone() {
                    let raw = this.props_url.read(cx).value().trim().to_string();
                    this.apply_props_url(&id, raw, window, cx);
                }
            }
        })
        .detach();
    }

    fn start_tick_loop(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            let visible = this
                .update(cx, |this, _| shell::visible_now(&this.hwnd))
                .unwrap_or(false);
            cx.background_executor()
                .timer(Duration::from_millis(if visible { 250 } else { 1000 }))
                .await;
            let result = this.update(cx, |this, cx| {
                let was_visible = this.visible;
                this.visible = shell::visible_now(&this.hwnd);
                if was_visible != this.visible {
                    crate::core::log::write(format!("window visible={}", this.visible));
                }
                let events = this.svc.tick();
                this.handle_events(events, cx);
                this.take_remote(cx);
                if this.heartbeat_at.elapsed() >= Duration::from_secs(5) {
                    this.heartbeat_at = Instant::now();
                    let active = this
                        .svc
                        .view()
                        .snaps
                        .iter()
                        .filter(|s| {
                            matches!(s.status, Status::Connecting | Status::Downloading)
                        })
                        .count();
                    crate::core::log::write(format!(
                        "tick visible={} rows={} active={}",
                        this.visible,
                        this.svc.view().snaps.len(),
                        active
                    ));
                }
                if let Some(clip) = this.clip.as_mut() {
                    clip.poll();
                }
                if this.visible {
                    cx.notify();
                }
            });
            if result.is_err() {
                break;
            }
        })
        .detach();
    }

    fn start_tray_loop(&mut self, cx: &mut Context<Self>) {
        let Some(rx) = self.tray.as_mut().and_then(|t| t.take_rx()) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            while let Ok(msg) = rx.recv().await {
                if this
                    .update(cx, |this, cx| this.on_tray(msg, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn install_close_handler(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let slot = self.hwnd.clone();
        let quitting = self.quitting.clone();
        window.on_window_should_close(cx, move |_window, _cx| {
            if quitting.load(Ordering::SeqCst) {
                return true;
            }
            // Close means "hide to tray", not "quit": SW_HIDE removes the
            // window from the taskbar while the process keeps downloading.
            crate::core::sys::hide(slot.get());
            false
        });
    }

    pub(crate) fn handle_events(&mut self, events: Vec<Event>, cx: &mut Context<Self>) {
        for event in events {
            match event {
                Event::Error(message) => {
                    self.notice = Some(Notice {
                        text: message,
                        error: true,
                    });
                }
                Event::ShutdownMachine => {
                    crate::app::force_shutdown();
                    self.quit(cx);
                }
                Event::ShutdownAt(Some(at)) => {
                    self.shutdown_at = Some(at);
                    // Surface the countdown even when the window is in the tray.
                    let hwnd = self.hwnd.get();
                    if hwnd != 0 {
                        crate::core::sys::restore(hwnd);
                    }
                    if let Some(handle) = self.window_handle {
                        handle
                            .update(cx, |_view, window, _cx| window.activate_window())
                            .ok();
                    }
                    cx.activate(true);
                }
                Event::Added(id) => {
                    let name = self
                        .svc
                        .view()
                        .snaps
                        .iter()
                        .find(|s| s.id == id)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| "download".into());
                    self.notice = Some(Notice {
                        text: format!("Added {name}"),
                        error: false,
                    });
                    if self.page != Page::Downloads {
                        self.page = Page::Downloads;
                    }
                }
                Event::ShutdownAt(None) => {
                    self.shutdown_at = None;
                }
                Event::Finished(name) => {
                    if self.svc.prefs().notify_complete {
                        self.show_toast(name, cx);
                    }
                }
            }
        }
    }

    /// Add URL batches sent by the browser extension.
    fn take_remote(&mut self, cx: &mut Context<Self>) {
        loop {
            let Ok(urls) = self.remote_rx.try_recv() else {
                return;
            };
            if urls.is_empty() {
                continue;
            }
            // Same review step as the egui build: nothing is queued until the
            // user confirms the batch.
            self.open_clip(urls, "Chrome", cx);
            self.page = Page::Downloads;
            self.list_filter = None;
        }
    }

    fn on_tray(&mut self, msg: TrayMsg, cx: &mut Context<Self>) {
        match msg {
            TrayMsg::Open => {
                crate::core::sys::restore(self.hwnd.get());
                if let Some(handle) = self.window_handle {
                    handle
                        .update(cx, |_view, window, _cx| window.activate_window())
                        .ok();
                }
                cx.activate(true);
                self.visible = true;
                cx.notify();
            }
            TrayMsg::Exit => {
                self.quitting.store(true, Ordering::SeqCst);
                self.quit(cx);
            }
        }
    }

    /// Persist download state through the engine interface, then leave the
    /// event loop.
    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        self.quitting.store(true, Ordering::SeqCst);
        self.svc.apply(Command::Shutdown);
        cx.quit();
    }

    pub(crate) fn set_theme(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.pal = Palette::new(dark);
        self.svc.apply(Command::SetTheme(dark));
        crate::apply_theme(dark, cx);
        cx.notify();
    }

    /// Cycle a column's sort and persist it for the next run.
    pub(crate) fn set_sort(&mut self, col: SortCol, cx: &mut Context<Self>) {
        self.sort = match self.sort {
            Some((c, false)) if c == col => Some((col, true)),
            Some((c, true)) if c == col => None,
            _ => Some((col, false)),
        };
        let (key, desc) = self
            .sort
            .map(|(c, d)| (c.key().to_string(), d))
            .unwrap_or_else(|| (String::new(), false));
        self.svc.apply(Command::SetSort { col: key, desc });
        cx.notify();
    }

    /// The message that floats over the window. Clicking anywhere outside the
    /// card — on the dimmed backdrop — closes it.
    fn notice_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let Some(notice) = &self.notice else {
            return div().into_any_element();
        };
        let text = notice.text.clone();
        let error = notice.error;
        let tone = if error { pal.bad } else { pal.ok };
        let icon = if error {
            Icon::new(gpui_kit::assets::IconName::CircleAlert)
        } else {
            Icon::new(gpui_kit::assets::IconName::CircleCheck)
        };
        let scrim = crate::theme::alpha(0x000000, if pal.dark { 0.42 } else { 0.20 });
        let card = div()
            .id("notice-card")
            .w(px(420.))
            .max_w_full()
            .rounded(px(14.))
            .bg(pal.card)
            .border_1()
            .border_color(with_alpha(tone, 0.42))
            .shadow(vec![
                BoxShadow::new(px(0.), px(12.), crate::theme::alpha(0x000000, 0.38))
                    .blur_radius(px(24.))
                    .spread_radius(px(-8.)),
            ])
            .px(px(16.))
            .py(px(14.))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(12.))
            .cursor_pointer()
            // Clicking the message itself also dismisses it.
            .on_click(cx.listener(|this, _, _, cx| {
                this.notice = None;
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                div()
                    .size(px(32.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(with_alpha(tone, 0.16))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon.size(px(18.)).text_color(tone)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(pal.text)
                            .child(if error { "Something went wrong" } else { "Done" }),
                    )
                    .child(div().text_sm().text_color(pal.muted).child(text)),
            );
        div()
            .absolute()
            .size_full()
            .child(
                div()
                    .id("notice-scrim")
                    .absolute()
                    .size_full()
                    .bg(scrim)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.notice = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(card),
            )
            .into_any_element()
    }

    fn toolbar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let typed = !self.url.read(cx).value().trim().is_empty();
        let queued = self.svc.prefs().queued;
        let has_selection = !self.selected.is_empty();
        let can_move = has_selection && self.sort.is_none();
        // Bulk buttons act on the selection when there is one, otherwise on
        // everything in the current view.
        let move_color = if can_move {
            pal.accent
        } else {
            with_alpha(pal.muted, 0.4)
        };
        let list_color = if has_selection {
            pal.paused
        } else {
            with_alpha(pal.muted, 0.4)
        };
        // A clearly visible hover for the action row, beyond the kit default.
        // Matches the kit's previous secondary fill, kept behind the stronger
        // hover tint.
        let action_style = ButtonCustomVariant::new(cx)
            .color(crate::theme::rgb(0x262626))
            .foreground(pal.text)
            .hover(with_alpha(pal.accent, 0.30))
            .active(with_alpha(pal.accent, 0.42));

        let add_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .id("url-box")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .flex_1()
                    .min_w_0()
                    .px(px(10.))
                    .py(px(2.))
                    .rounded(px(9.))
                    .bg(pal.bg)
                    .border_1()
                    .border_color(pal.line)
                    .hover(|s| {
                        s.bg(pal.card)
                            .border_color(with_alpha(pal.accent, 0.5))
                    })
                    .child(Icon::new(IconName::Globe).size(px(16.)).text_color(pal.muted))
                    .child(Input::new(&self.url).flex_1().min_w_0().appearance(false)),
            )
            .child(
                Button::new("paste")
                    .outline()
                    .icon(Icon::new(IconName::Copy).text_color(pal.accent))
                    .label("Clipboard")
                    .tooltip("Get links from the clipboard")
                    .on_click(cx.listener(|this, _, window, cx| this.paste(window, cx))),
            )
            // A hand-drawn switch: the kit `Switch` swallowed these clicks.
            .child(
                div()
                    .id("queue-toggle")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(7.))
                    .h(px(30.))
                    .px(px(8.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(with_alpha(pal.accent, 0.10)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let next = !this.svc.prefs().queued;
                        this.svc.apply(Command::SetQueued(next));
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(30.))
                            .h(px(16.))
                            .flex_shrink_0()
                            .rounded_full()
                            .border_1()
                            .border_color(if queued {
                                with_alpha(pal.accent, 0.7)
                            } else {
                                pal.line
                            })
                            .bg(if queued { pal.accent } else { pal.track })
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .size(px(10.))
                                    .rounded_full()
                                    .bg(if queued { pal.bg } else { pal.muted })
                                    .ml(px(if queued { 16. } else { 2. })),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(if queued { pal.text } else { pal.muted })
                            .child("Don't start immediately"),
                    ),
            )
            .child(
                Button::new("add")
                    .primary()
                    .icon(IconName::ArrowDown)
                    .label("Download")
                    .disabled(!typed)
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
            );

        let actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                Button::new("start")
                    .custom(action_style)
                    .icon(Icon::new(IconName::Play).text_color(pal.ok))
                    .label(if has_selection {
                        "Start".to_string()
                    } else {
                        "Start list".to_string()
                    })
                    .tooltip(if has_selection {
                        "Start the selected downloads"
                    } else {
                        "Start all downloads in this list or view"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        let ids = this.action_ids();
                        if ids.is_empty() {
                            this.svc.apply(Command::ResumeAll);
                        } else {
                            this.svc.apply(Command::Resume(ids));
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("pause")
                    .custom(action_style)
                    .icon(Icon::new(IconName::Pause).text_color(pal.warn))
                    .label(if has_selection {
                        "Pause".to_string()
                    } else {
                        "Pause list".to_string()
                    })
                    .tooltip(if has_selection {
                        "Pause the selected downloads"
                    } else {
                        "Pause all downloads in this list or view"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        let ids = this.action_ids();
                        if ids.is_empty() {
                            this.svc.apply(Command::PauseAll);
                        } else {
                            this.svc.apply(Command::Pause(ids));
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("delete")
                    .custom(action_style)
                    .icon(Icon::new(IconName::Delete).text_color(pal.bad))
                    .label(if has_selection {
                        "Delete".to_string()
                    } else {
                        "Delete all from list".to_string()
                    })
                    .tooltip(if has_selection {
                        "Delete the selected downloads"
                    } else {
                        "Delete all downloads in this list or view"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        let ids = this.action_ids();
                        if ids.is_empty() {
                            this.notice = Some(Notice {
                                text: "Nothing to delete in this view".into(),
                                error: true,
                            });
                        } else {
                            this.confirm_delete = Some(ConfirmDelete {
                                ids,
                                delete_files: false,
                            });
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("up")
                    .custom(action_style)
                    .icon(Icon::new(IconName::ArrowUp).text_color(move_color))
                    .tooltip("Move the selected downloads up the queue")
                    .disabled(!can_move)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.move_selected(true);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("down")
                    .custom(action_style)
                    .icon(Icon::new(IconName::ArrowDown).text_color(move_color))
                    .tooltip("Move the selected downloads down the queue")
                    .disabled(!can_move)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.move_selected(false);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("add-to-list")
                    .custom(action_style)
                    .icon(Icon::new(IconName::Inbox).text_color(list_color))
                    .label("List")
                    .disabled(!has_selection)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_list_dialog(window, cx);
                    })),
            );

        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(pal.line)
            .bg(pal.panel)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .px(px(20.))
                    .pt(px(14.))
                    .pb(px(12.))
                    .child(add_row)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(actions)
                            .child(
                                div()
                                    .id("search-box")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .w(px(230.))
                                    .px(px(9.))
                                    .py(px(2.))
                                    .rounded(px(8.))
                                    .bg(pal.bg)
                                    .border_1()
                                    .border_color(pal.line)
                                    .hover(|s| {
                                        s.bg(pal.card)
                                            .border_color(with_alpha(pal.accent, 0.5))
                                    })
                                    .child(
                                        Icon::new(IconName::Search)
                                            .size(px(14.))
                                            .text_color(pal.muted),
                                    )
                                    .child(
                                        Input::new(&self.search)
                                            .flex_1()
                                            .min_w_0()
                                            .appearance(false),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Draggable divider between the navigation sidebar and the content.
    fn sidebar_splitter(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let weak = cx.entity().downgrade();
        let active = self.resizing_sidebar;
        div()
            .id("sidebar-splitter")
            .w(px(5.))
            .h_full()
            .flex_shrink_0()
            .bg(if active {
                with_alpha(pal.accent, 0.55)
            } else {
                pal.line
            })
            .hover(|s| s.bg(with_alpha(pal.accent, 0.45)))
            .cursor_col_resize()
            .on_mouse_down(MouseButton::Left, move |_, _, cx: &mut App| {
                weak.update(cx, |this, cx| {
                    this.resizing_sidebar = true;
                    cx.notify();
                })
                .ok();
            })
            .into_any_element()
    }

    /// Full-window capture while resizing, so the pointer can leave the splitter.
    fn sidebar_resize_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let weak_move = cx.entity().downgrade();
        let weak_up = cx.entity().downgrade();
        div()
            .id("sidebar-resize-capture")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .cursor_col_resize()
            .on_mouse_move(move |event, _, cx: &mut App| {
                let x = event.position.x.as_f32();
                let still_down = event.pressed_button == Some(MouseButton::Left);
                weak_move
                    .update(cx, |this, cx| {
                        if !still_down {
                            this.finish_sidebar_resize(cx);
                            return;
                        }
                        this.sidebar_width = x.clamp(
                            crate::core::prefs::MIN_SIDEBAR_WIDTH as f32,
                            crate::core::prefs::MAX_SIDEBAR_WIDTH as f32,
                        );
                        cx.notify();
                    })
                    .ok();
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx: &mut App| {
                weak_up
                    .update(cx, |this, cx| this.finish_sidebar_resize(cx))
                    .ok();
            })
            .into_any_element()
    }

    /// Stop the splitter drag and persist the width.
    fn finish_sidebar_resize(&mut self, cx: &mut Context<Self>) {
        if !self.resizing_sidebar {
            return;
        }
        self.resizing_sidebar = false;
        let width = self.sidebar_width.round() as u32;
        self.svc.apply(Command::SetSidebarWidth(width));
        cx.notify();
    }

    fn status_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let app_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();
        let snaps = &self.svc.view().snaps;
        let all = snaps.len();
        let active: Vec<&Snapshot> = snaps
            .iter()
            .filter(|s| s.status == Status::Downloading)
            .collect();
        let speed: u64 = active.iter().map(|s| s.speed).sum();
        let conns: u32 = active.iter().map(|s| s.connections).sum();
        let limit = self.svc.prefs().limit_kbps;

        let mut left = format!(
            "{all} files  ·  {} active  ·  {conns} connections",
            active.len()
        );
        if limit > 0 {
            left.push_str(&format!(
                "  ·  Limit {}",
                crate::format::fmt_speed(limit * 1024)
            ));
        }
        let right = if speed > 0 {
            format!("Total {}", crate::format::fmt_speed(speed))
        } else {
            "Idle".to_string()
        };
        // Free space on the volume that holds new downloads.
        let dir = self.svc.save_dir();
        let disk = crate::core::sys::free_space(&dir).map(|(free, total)| {
            let used = total.saturating_sub(free);
            let fraction = (used as f32 / total as f32).clamp(0.0, 1.0);
            let tone = if fraction > 0.9 { pal.bad } else { pal.accent };
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .py(px(3.))
                .rounded(px(7.))
                .bg(pal.card)
                .border_1()
                .border_color(pal.line)
                .child(
                    div()
                        .font_semibold()
                        .text_color(pal.text)
                        .child(drive_label(&dir)),
                )
                .child(
                    div()
                        .w(px(90.))
                        .h(px(6.))
                        .rounded_full()
                        .bg(pal.track)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .rounded_full()
                                .bg(tone),
                        ),
                )
                .child(
                    div().text_color(pal.muted).child(format!(
                        "{} free of {}",
                        crate::format::fmt_bytes(free),
                        crate::format::fmt_bytes(total)
                    )),
                )
        });
        div()
            .h(px(34.))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(20.))
            .border_t_1()
            .border_color(pal.line)
            .bg(pal.panel)
            .text_xs()
            .text_color(pal.muted)
            .child(left)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .children(disk)
                    .child(
                        Button::new("open-logs")
                            .ghost()
                            .compact()
                            .icon(IconName::FileText)
                            .tooltip("Show the debug log")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_logs_window(window, cx);
                            })),
                    )
                    .child(
                        Button::new("open-app-folder")
                            .ghost()
                            .compact()
                            .icon(IconName::FolderOpen)
                            .tooltip("Open the app folder")
                            .on_click({
                                let dir = app_dir.clone();
                                move |_, _, cx: &mut App| {
                                    open_in_background(dir.clone(), cx);
                                }
                            }),
                    )
                    // Fixed width keeps the free-space card from shifting when
                    // the speed text changes size.
                    .child(
                        div()
                            .w(px(110.))
                            .flex_shrink_0()
                            .text_right()
                            .text_color(if speed > 0 { pal.accent } else { pal.muted })
                            .child(right),
                    ),
            )
            .into_any_element()
    }

    // --- commands -------------------------------------------------------

    pub(crate) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url.read(cx).value().trim().to_string();
        if url.is_empty() {
            self.notice = Some(Notice {
                text: "Paste an http(s) URL first".into(),
                error: true,
            });
            cx.notify();
            return;
        }
        if let Some(existing) = self.svc.unfinished_duplicate(&url) {
            self.ask_duplicate(url, existing.name, window, cx);
            return;
        }
        self.start_download(url, None, window, cx);
    }

    pub(crate) fn start_download(
        &mut self,
        url: String,
        file_name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dir = self.svc.save_dir();
        let queued = self.svc.prefs().queued;
        let events = self.svc.apply(Command::Add {
            url,
            dir,
            file_name,
            queued,
        });
        self.handle_events(events, cx);
        self.url
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    fn ask_duplicate(
        &mut self,
        url: String,
        existing: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pal = self.pal;
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let weak_ok = weak.clone();
            let url_ok = url.clone();
            dialog
                .title("Already in the list")
                .child(div().text_sm().text_color(pal.muted).child(format!(
                    "{existing} is still downloading from this address. Add another copy?"
                )))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("dup-cancel")
                                .outline()
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("dup-add")
                                .primary()
                                .label("Add anyway")
                                .on_click(move |_, window, cx| {
                                    let url = url_ok.clone();
                                    weak_ok
                                        .update(cx, |this, cx| {
                                            this.start_download(url, None, window, cx);
                                        })
                                        .ok();
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    pub(crate) fn pick_list_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        let names: Vec<(String, Hsla)> = self
            .svc
            .lists()
            .iter()
            .map(|l| (l.name.clone(), pal.list_color(l.color)))
            .collect();
        let selection: Vec<String> = self.selected.iter().cloned().collect();
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let mut rows = div().flex().flex_col().gap(px(2.)).py(px(4.)).w_full();
            for (name, color) in &names {
                let weak = weak.clone();
                let ids = selection.clone();
                let target = name.clone();
                rows = rows.child(
                    div()
                        .id(ElementId::Name(format!("pick-{name}").into()))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .hover(move |s| s.bg(pal.card_hover))
                        .on_click(move |_, window, cx: &mut App| {
                            let ids = ids.clone();
                            let target = target.clone();
                            weak.update(cx, |this, cx| {
                                this.svc.apply(Command::AssignList {
                                    ids,
                                    name: target,
                                });
                                cx.notify();
                            })
                            .ok();
                            window.close_dialog(cx);
                        })
                        .child(
                            div()
                                .size(px(9.))
                                .rounded_full()
                                .bg(*color),
                        )
                        .child(div().text_sm().child(name.clone())),
                );
            }
            dialog
                .title("Add to list")
                .child(rows)
                .footer(
                    DialogFooter::new().child(
                        Button::new("pick-cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }

    fn move_selected(&mut self, up: bool) {
        if self.selected.is_empty() {
            return;
        }
        let ids: Vec<String> = self.selected.iter().cloned().collect();
        self.svc.apply(Command::MoveSelected { ids, up });
    }

    /// Ids the bulk buttons act on: the selected rows, or the whole view.
    pub(crate) fn action_ids(&self) -> Vec<String> {
        if self.selected.is_empty() {
            crate::downloads::visible(self)
                .into_iter()
                .map(|s| s.id)
                .collect()
        } else {
            self.selected.iter().cloned().collect()
        }
    }

    /// Right-click properties for one download, shaped like the classic
    /// "File Properties" window: type, state, size, location and address.
    pub(crate) fn props_dialog(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snap) = self.svc.view().snaps.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        let pal = self.pal;
        self.props_id = Some(id.clone());
        self.props_copied = None;
        self.props_url_error = None;
        self.props_url
            .update(cx, |state, cx| state.set_value(snap.url.clone(), window, cx));
        let lists: Vec<String> = self
            .svc
            .lists()
            .iter()
            .filter(|l| l.items.iter().any(|x| x == &id))
            .map(|l| l.name.clone())
            .collect();
        let editable_url = snap.status == Status::Paused;
        let type_label = file_type_label(&snap.name);
        let status_label = crate::format::status_text(&snap);
        let size = match snap.total {
            Some(t) => format!("{} ({} bytes)", crate::format::fmt_bytes(t), t),
            None => format!("{} so far", crate::format::fmt_bytes(snap.received)),
        };
        let progress = match snap.total.filter(|t| *t > 0) {
            Some(_) => format!("{:.1}%", snap.received as f64 / snap.total.unwrap() as f64 * 100.0),
            None => "—".to_string(),
        };
        let downloaded = match snap.total {
            Some(t) => format!(
                "{} of {}",
                crate::format::fmt_bytes(snap.received),
                crate::format::fmt_bytes(t)
            ),
            None => crate::format::fmt_bytes(snap.received),
        };
        let save_to = snap.dest.to_string_lossy().into_owned();
        let name = snap.name.clone();
        let open_target = if snap.status == Status::Done {
            snap.dest.clone()
        } else {
            snap.dest
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| snap.dest.clone())
        };
        let mut rows: Vec<(&'static str, String, bool, Hsla)> = vec![
            ("Type", type_label.clone(), false, pal.text),
            ("Status", status_label, false, pal.status(snap.status)),
            ("Size", size, false, pal.text),
            ("Save To", save_to, true, pal.text),
            ("Downloaded", downloaded, false, pal.text),
            ("Progress", progress, false, pal.text),
            (
                "Lists",
                if lists.is_empty() {
                    "—".to_string()
                } else {
                    lists.join(", ")
                },
                false,
                pal.text,
            ),
        ];
        if let Some(err) = snap.error.clone() {
            rows.push(("Error", err, false, pal.bad));
        }

        let weak = cx.entity().downgrade();
        let url_input = self.props_url.clone();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let copied = weak
                .read_with(cx, |this, _| this.props_copied.clone())
                .unwrap_or(None);
            let url_error = weak
                .read_with(cx, |this, _| this.props_url_error.clone())
                .unwrap_or(None);
            let mut body = div()
                .flex()
                .flex_col()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.))
                        .w_full()
                        .pb(px(10.))
                        .child(
                            div()
                                .size(px(34.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.))
                                .bg(with_alpha(pal.accent, 0.12))
                                .border_1()
                                .border_color(with_alpha(pal.accent, 0.38))
                                .child(Icon::new(IconName::File).size(px(17.)).text_color(pal.accent)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(pal.text)
                                        .text_ellipsis()
                                        .child(name.clone()),
                                ),
                        ),
                )
                .child(div().h(px(1.)).w_full().bg(pal.line));
            for (label, value, copyable, color) in rows.iter() {
                body = body.child(property_row(
                    *label,
                    value.clone(),
                    *color,
                    *copyable,
                    copied.as_deref(),
                    weak.clone(),
                    pal,
                ));
                // The address is editable while the download is paused.
                if *label == "Save To" {
                    body = body.child(address_row(
                        url_input.clone(),
                        editable_url,
                        url_error.clone(),
                        copied.as_deref() == Some("Address"),
                        weak.clone(),
                        id.clone(),
                        pal,
                    ));
                }
            }
            dialog
                .title("File Properties")
                .width(px(560.))
                .child(body)
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("props-open")
                                .outline()
                                .label("Open")
                                .on_click({
                                    let target = open_target.clone();
                                    move |_, _, cx: &mut App| {
                                        open_in_background(target.clone(), cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("props-ok")
                                .primary()
                                .label("OK")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        ),
                )
        });
    }

    /// Validate and apply the address typed in the properties dialog.
    fn apply_props_url(
        &mut self,
        id: &str,
        url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = url.trim().to_string();
        if url.is_empty() {
            self.props_url_error = Some("Enter an address first.".into());
            cx.notify();
            return;
        }
        match self.svc.set_url(id, &url) {
            Ok(()) => {
                self.props_url_error = None;
                self.props_url
                    .update(cx, |state, cx| state.set_value(url, window, cx));
            }
            Err(e) => self.props_url_error = Some(e),
        }
        cx.notify();
    }

    /// Warn before downloading a completed file again, with a choice to keep
    /// the existing copy.
    pub(crate) fn confirm_redownload(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snap) = self.svc.view().snaps.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        self.redownload_keep = true;
        let pal = self.pal;
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _window, cx| {
            let keep = weak
                .read_with(cx, |this, _| this.redownload_keep)
                .unwrap_or(true);
            let name = snap.name.clone();
            let weak_toggle = weak.clone();
            let start = {
                let weak = weak.clone();
                let snap = snap.clone();
                move |window: &mut Window, cx: &mut App| {
                    weak.update(cx, |this, cx| {
                        if !this.redownload_keep {
                            let _ = std::fs::remove_file(&snap.dest);
                        }
                        let dir = snap
                            .dest
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| this.svc.save_dir());
                        let queued = this.svc.prefs().queued;
                        let events = this.svc.apply(Command::Add {
                            url: snap.url.clone(),
                            dir,
                            file_name: Some(snap.name.clone()),
                            queued,
                        });
                        this.handle_events(events, cx);
                        cx.notify();
                    })
                    .ok();
                    window.close_dialog(cx);
                }
            };
            dialog
                .title("Download again?")
                .width(px(470.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .w_full()
                        .child(
                            div()
                                .text_sm()
                                .text_color(pal.text)
                                .child(if keep {
                                    format!(
                                        "A new copy of “{name}” will be downloaded next to the existing file."
                                    )
                                } else {
                                    format!(
                                        "The existing “{name}” will be deleted and downloaded again."
                                    )
                                }),
                        )
                        .child(
                            Checkbox::new("redl-keep")
                                .checked(keep)
                                .label("Keep the old file")
                                .on_click(move |on: &bool, _, cx: &mut App| {
                                    let on = *on;
                                    weak_toggle
                                        .update(cx, |this, cx| {
                                            this.redownload_keep = on;
                                            cx.notify();
                                        })
                                        .ok();
                                }),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("redl-cancel")
                                .outline()
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("redl-ok")
                                .primary()
                                .label("Download again")
                                .on_click(move |_, window, cx| start(window, cx)),
                        ),
                )
        });
    }

    /// Temporary debug-log viewer, opened from the status bar.
    /// Small menu under the logo: guide, about, update check and full exit.
    fn logo_menu_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if !self.logo_menu {
            return div().into_any_element();
        }
        let pal = self.pal;
        let weak = cx.entity().downgrade();
        let close = cx.listener(|this, _, _, cx| {
            this.logo_menu = false;
            cx.notify();
        });
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("logo-menu-scrim")
                    .absolute()
                    .size_full()
                    .on_click(close),
            )
            .child(
                div()
                    .id("logo-menu-card")
                    .absolute()
                    .left(px(14.))
                    .top(px(74.))
                    .w(px(240.))
                    .rounded(px(10.))
                    .bg(pal.card)
                    .border_1()
                    .border_color(pal.line)
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(12.), crate::theme::alpha(0x000000, 0.42))
                            .blur_radius(px(24.))
                            .spread_radius(px(-8.)),
                    ])
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .on_click(|_, _, cx: &mut App| cx.stop_propagation())
                    .child(logo_menu_item(
                        gpui_kit::assets::IconName::BookOpen,
                        "User guide",
                        pal.accent,
                        pal,
                        weak.clone(),
                        |this, window, cx| this.user_guide_dialog(window, cx),
                    ))
                    .child(logo_menu_item(
                        gpui_kit::assets::IconName::Info,
                        "About",
                        pal.info,
                        pal,
                        weak.clone(),
                        |this, window, cx| this.about_dialog(window, cx),
                    ))
                    .child(logo_menu_item(
                        gpui_kit::assets::IconName::RefreshCw,
                        "Check for update",
                        pal.warn,
                        pal,
                        weak.clone(),
                        |this, window, cx| this.check_for_update(window, cx),
                    ))
                    .child(div().h(px(1.)).mx(px(6.)).my(px(4.)).bg(pal.line))
                    .child(logo_menu_item(
                        gpui_kit::assets::IconName::WindowClose,
                        "Exit",
                        pal.bad,
                        pal,
                        weak,
                        |this, window, cx| this.confirm_exit(window, cx),
                    )),
            )
            .into_any_element()
    }

    fn user_guide_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        let weak = cx.entity().downgrade();
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();
        let en = dir.join("USER-GUIDE.pdf");
        let fa = dir.join("USER-GUIDE.fa.pdf");
        let video = dir.join("install extension guide.mp4");
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("User guide")
                .width(px(540.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .w_full()
                        .child(guide_row(
                            "en",
                            gpui_kit::assets::IconName::BookOpenText,
                            pal.accent,
                            "English guide",
                            "DVIDM user guide (PDF)",
                            pal,
                            weak.clone(),
                            en.clone(),
                        ))
                        .child(guide_row(
                            "fa",
                            gpui_kit::assets::IconName::Languages,
                            pal.info,
                            "Persian guide",
                            "Persian user guide (PDF)",
                            pal,
                            weak.clone(),
                            fa.clone(),
                        ))
                        .child(guide_row(
                            "video",
                            gpui_kit::assets::IconName::FilePlay,
                            pal.warn,
                            "Install extension",
                            "Video: install extension guide.mp4",
                            pal,
                            weak.clone(),
                            video.clone(),
                        )),
                )
                .footer(DialogFooter::new().child(
                    Button::new("guide-close")
                        .outline()
                        .label("Close")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ))
        });
    }

    fn about_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("About DVIDM")
                .width(px(440.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(pal.text)
                                .child(format!(
                                    "DV Internet Download Manager {}",
                                    env!("CARGO_PKG_VERSION")
                                )),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(pal.muted)
                                .child("Segmented downloads, lists, scheduling and browser integration."),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(pal.muted)
                                .child("English and Persian guides, plus the extension video, are in the User guide menu."),
                        ),
                )
                .footer(DialogFooter::new().child(
                    Button::new("about-ok")
                        .primary()
                        .label("OK")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ))
        });
    }

    fn check_for_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("Check for updates")
                .width(px(470.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(pal.text)
                                .child(format!(
                                    "You are running DVIDM {}.",
                                    env!("CARGO_PKG_VERSION")
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(pal.muted)
                                .child("This build has no update feed configured, so it cannot check for a newer version."),
                        ),
                )
                .footer(DialogFooter::new().child(
                    Button::new("update-ok")
                        .primary()
                        .label("OK")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ))
        });
    }

    fn confirm_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        let active = self
            .svc
            .view()
            .snaps
            .iter()
            .filter(|s| matches!(s.status, Status::Connecting | Status::Downloading))
            .count();
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let message = if active > 0 {
                format!("{active} download(s) are still running. Exit completely and stop them?")
            } else {
                "Exit DVIDM completely? It will close the window and remove the tray icon."
                    .to_string()
            };
            dialog
                .title("Exit DVIDM?")
                .width(px(450.))
                .child(div().text_sm().text_color(pal.text).child(message))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("exit-cancel")
                                .outline()
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("exit-ok")
                                .danger()
                                .label("Exit")
                                .on_click({
                                    let weak = weak.clone();
                                    move |_, window, cx| {
                                        window.close_dialog(cx);
                                        weak.update(cx, |this, cx| this.quit(cx)).ok();
                                    }
                                }),
                        ),
                )
        });
    }

    /// Show the custom completion card in its own borderless, top-most window
    /// at the bottom-right of the primary display.
    fn show_toast(&mut self, name: String, cx: &mut Context<Self>) {
        let previous = self.toast_window.take();
        let (x, y) = match cx.primary_display() {
            Some(display) => {
                let bounds = display.bounds();
                (
                    bounds.origin.x + bounds.size.width - px(400.),
                    bounds.origin.y + bounds.size.height - px(170.),
                )
            }
            None => (px(800.), px(500.)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(x, y),
                size: size(px(380.), px(96.)),
            })),
            titlebar: None,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        };
        let pal = self.pal;
        cx.spawn(async move |this, cx| {
            if let Some(handle) = previous {
                cx.update(|app| {
                    handle
                        .update(app, |_, window, _| window.remove_window())
                        .ok();
                });
            }
            match cx.open_window(options, |window, cx| {
                cx.new(|cx| crate::toast::ToastView::new(window, cx, pal, name))
            }) {
                Ok(handle) => {
                    this.update(cx, |this, _| this.toast_window = Some(handle.into()))
                        .ok();
                }
                Err(err) => {
                    crate::core::log::write(format!("toast window failed: {err:#}"));
                }
            }
        })
        .detach();
    }

    /// Open the debug log in its own native window, or focus an existing one.
    pub(crate) fn open_logs_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.log_window {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
            self.log_window = None;
        }
        let bounds = window.bounds();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(bounds.origin.x + px(64.), bounds.origin.y + px(64.)),
                size: size(px(880.), px(560.)),
            })),
            window_min_size: Some(size(px(520.), px(320.))),
            titlebar: Some(TitlebarOptions {
                title: Some("DVIDM Debug Log".into()),
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
        let pal = self.pal;
        match gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| crate::logs::LogView::new(window, cx, pal))
        }) {
            Ok((handle, _view)) => self.log_window = Some(handle),
            Err(err) => {
                self.notice = Some(Notice {
                    text: format!("Could not open the log window: {err:#}"),
                    error: true,
                });
                cx.notify();
            }
        }
    }

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = Page::Settings;
        self.list_filter = None;
        let limit = self.svc.prefs().limit_kbps;
        self.limit_custom = limit != 0 && !crate::settings::LIMIT_PRESETS.contains(&limit);
        let text = if limit == 0 {
            String::new()
        } else {
            limit.to_string()
        };
        self.limit_input
            .update(cx, |state, cx| state.set_value(text, window, cx));
        cx.notify();
    }

    pub(crate) fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = crate::clipboard::read_text() else {
            self.notice = Some(Notice {
                text: "Clipboard is empty or busy".into(),
                error: true,
            });
            cx.notify();
            return;
        };
        let urls = crate::clipboard::extract_urls(&text);
        match urls.len() {
            0 => {
                self.notice = Some(Notice {
                    text: "No http(s) links in the clipboard".into(),
                    error: true,
                });
            }
            1 => {
                let url = urls.into_iter().next().unwrap();
                self.url
                    .update(cx, |state, cx| state.set_value(url, window, cx));
            }
            _ => {
                self.open_clip(urls, "clipboard", cx);
            }
        }
        cx.notify();
    }

    /// Start (or extend) a review of links that arrived together.
    pub(crate) fn open_clip(&mut self, urls: Vec<String>, source: &'static str, cx: &mut Context<Self>) {
        if urls.is_empty() {
            return;
        }
        match self.clip.as_mut() {
            // A second Chrome send joins the open window instead of replacing it.
            Some(clip) => clip.push_urls(urls),
            None => {
                self.clip_queued = self.svc.prefs().queued;
                self.clip = Some(ClipImport::new(urls, source));
            }
        }
        if self.clip_list.is_none() {
            self.clip_list = self
                .svc
                .lists()
                .iter()
                .find(|l| l.is_default)
                .or_else(|| self.svc.lists().first())
                .map(|l| l.name.clone());
        }
        cx.notify();
    }

    fn close_clip(&mut self, cx: &mut Context<Self>) {
        self.clip = None;
        cx.notify();
    }

    /// Add every checked link, then file the new downloads into the chosen list.
    fn commit_clip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(clip) = self.clip.take() else {
            return;
        };
        let urls: Vec<String> = clip
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.url.clone())
            .collect();
        if urls.is_empty() {
            self.notice = Some(Notice {
                text: "No links selected".into(),
                error: true,
            });
            cx.notify();
            return;
        }
        let dir = self.svc.save_dir();
        let queued = self.clip_queued;
        let mut added = Vec::new();
        let mut failures = Vec::new();
        for url in urls {
            let events = self.svc.apply(Command::Add {
                url,
                dir: dir.clone(),
                file_name: None,
                queued,
            });
            for ev in events {
                match ev {
                    Event::Added(id) => added.push(id),
                    Event::Error(e) => failures.push(e),
                    _ => {}
                }
            }
        }
        if let Some(target) = self.clip_list.clone() {
            if !added.is_empty() {
                self.svc.apply(Command::AssignList {
                    ids: added.clone(),
                    name: target,
                });
            }
        }
        self.notice = Some(match failures.last() {
            Some(e) => Notice {
                text: e.clone(),
                error: true,
            },
            None => Notice {
                text: format!("Added {} link(s)", added.len()),
                error: false,
            },
        });
        self.page = Page::Downloads;
        self.url
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();
    }

    /// Scrim plus a centered card, shared by every overlay.
    fn overlay_shell(card: Div) -> AnyElement {
        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(crate::theme::alpha(0x000000, 0.55))
            .child(card)
            .into_any_element()
    }

    fn overlay_card(&self) -> Div {
        div()
            .w(px(680.))
            .max_w(relative(0.92))
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(20.))
            .rounded(px(14.))
            .bg(self.pal.card)
            .border_1()
            .border_color(self.pal.line)
    }

    /// Clipboard / Chrome import: one card per link with its probed size, a
    /// checkbox, a destination list and a single Add button.
    fn clip_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let Some(clip) = self.clip.as_ref() else {
            return div().into_any_element();
        };
        let source = clip.source;
        let count = clip.items.len();
        let selected = clip.items.iter().filter(|i| i.selected).count();
        let known: u64 = clip.items.iter().filter_map(|i| i.size).sum();
        let loading = clip.items.iter().any(|i| i.loading);
        let items: Vec<(usize, String, Option<u64>, bool, Option<String>, bool)> = clip
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                (
                    i,
                    item.name.clone(),
                    item.size,
                    item.loading,
                    item.error.clone(),
                    item.selected,
                )
            })
            .collect();
        let all = count > 0 && selected == count;
        let lists: Vec<(String, Hsla)> = self
            .svc
            .lists()
            .iter()
            .map(|l| (l.name.clone(), pal.list_color(l.color)))
            .collect();
        let weak = cx.entity().downgrade();

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .text_color(pal.text)
                            .child(format!("From {source}")),
                    )
                    .child(
                        div().text_sm().text_color(pal.muted).child(format!(
                            "{count} links found{}",
                            if known > 0 {
                                format!(" · {}", crate::format::fmt_bytes(known))
                            } else {
                                String::new()
                            }
                        )),
                    ),
            )
            .child({
                let weak = weak.clone();
                Button::new("clip-close")
                    .ghost()
                    .compact()
                    .icon(IconName::Close)
                    .on_click(move |_, _, cx: &mut App| {
                        weak.update(cx, |this, cx| this.close_clip(cx)).ok();
                    })
            });

        let select_all = {
            let weak = weak.clone();
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            Checkbox::new("clip-all").checked(all).on_click(
                                move |_, _, cx: &mut App| {
                                    weak.update(cx, |this, cx| {
                                        let next = !this
                                            .clip
                                            .as_ref()
                                            .map(|c| c.items.iter().all(|i| i.selected))
                                            .unwrap_or(false);
                                        if let Some(clip) = this.clip.as_mut() {
                                            for item in &mut clip.items {
                                                item.selected = next;
                                            }
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                },
                            ),
                        )
                        .child(div().text_sm().text_color(pal.text).child("Select all")),
                )
                .child(div().text_xs().text_color(pal.muted).child(format!(
                    "{selected}/{count} selected{}",
                    if loading { " · checking sizes" } else { "" }
                )))
        };

        let mut rows = div()
            .id("clip-rows")
            .flex()
            .flex_col()
            .gap(px(6.))
            .max_h(px(340.))
            .overflow_y_scroll();
        for (i, name, size, item_loading, error, is_selected) in items {
            let weak_toggle = weak.clone();
            let (size_text, size_color) = if let Some(e) = error {
                (e, pal.bad)
            } else if item_loading {
                ("Checking…".to_string(), pal.muted)
            } else if let Some(bytes) = size {
                (crate::format::fmt_bytes(bytes), pal.ok)
            } else {
                ("Ready".to_string(), pal.ok)
            };
            rows = rows.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(pal.bg)
                    .border_1()
                    .border_color(pal.line)
                    .child(
                        Checkbox::new(ElementId::Name(format!("clip-{i}").into()))
                            .checked(is_selected)
                            .on_click(move |_, _, cx: &mut App| {
                                weak_toggle
                                    .update(cx, |this, cx| {
                                        if let Some(clip) = this.clip.as_mut() {
                                            if let Some(item) = clip.items.get_mut(i) {
                                                item.selected = !item.selected;
                                            }
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_sm()
                            .text_color(pal.text)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(size_color)
                            .child(size_text),
                    ),
            );
        }

        let weak = cx.entity().downgrade();
        let list_chips: Vec<AnyElement> = lists
            .into_iter()
            .map(|(name, color)| {
                let weak = weak.clone();
                let target = name.clone();
                let active = self.clip_list.as_deref() == Some(name.as_str());
                div()
                    .id(ElementId::Name(format!("clip-target-{name}").into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .px(px(9.))
                    .py(px(5.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if active { color } else { pal.line })
                    .bg(if active {
                        crate::theme::with_alpha(color, 0.16)
                    } else {
                        pal.bg
                    })
                    .hover(|s| s.bg(pal.card_hover))
                    .on_click(move |_, _, cx: &mut App| {
                        let target = target.clone();
                        weak.update(cx, |this, cx| {
                            this.clip_list = Some(target.clone());
                            cx.notify();
                        })
                        .ok();
                    })
                    .child(div().size(px(8.)).rounded_full().bg(color))
                    .child(div().text_xs().text_color(pal.text).child(name))
                    .into_any_element()
            })
            .collect();
        let list_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(div().text_xs().text_color(pal.muted).child("Add to"))
            .children(list_chips);

        let weak_queue = weak.clone();
        let queued = self.clip_queued;
        let queue_toggle = div()
            .id("clip-queue-toggle")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(9.))
            .cursor_pointer()
            .on_click(move |_, _, cx: &mut App| {
                weak_queue
                    .update(cx, |this, cx| {
                        this.clip_queued = !this.clip_queued;
                        cx.notify();
                    })
                    .ok();
            })
            .child(
                div()
                    .w(px(30.))
                    .h(px(16.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(if queued {
                        with_alpha(pal.accent, 0.7)
                    } else {
                        pal.line
                    })
                    .bg(if queued { pal.accent } else { pal.track })
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .size(px(10.))
                            .rounded_full()
                            .bg(if queued { pal.bg } else { pal.muted })
                            .ml(px(if queued { 16. } else { 2. })),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(pal.text)
                    .child("Don't start immediately"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(pal.muted)
                    .child("Add the links paused and start them later."),
            );

        let weak_cancel = weak.clone();
        let weak_add = weak.clone();
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(pal.muted)
                    .child(format!("{selected} selected · {}", crate::format::fmt_bytes(known))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        Button::new("clip-cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(move |_, _, cx: &mut App| {
                                weak_cancel.update(cx, |this, cx| this.close_clip(cx)).ok();
                            }),
                    )
                    .child(
                        Button::new("clip-add")
                            .primary()
                            .label(if selected > 0 {
                                format!("Add {selected}")
                            } else {
                                "Add".to_string()
                            })
                            .disabled(selected == 0)
                            .on_click(move |_, window, cx: &mut App| {
                                weak_add
                                    .update(cx, |this, cx| this.commit_clip(window, cx))
                                    .ok();
                            }),
                    ),
            );

        let card = self
            .overlay_card()
            .child(header)
            .child(select_all)
            .child(rows)
            .child(queue_toggle)
            .child(list_bar)
            .child(footer);
        Self::overlay_shell(card)
    }

    /// Delete confirmation with the optional "remove the files too" switch.
    fn delete_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let Some(pending) = self.confirm_delete.as_ref() else {
            return div().into_any_element();
        };
        let count = pending.ids.len();
        let with_files = pending.delete_files;
        let weak = cx.entity().downgrade();
        let weak_switch = weak.clone();
        let weak_ok = weak.clone();

        let card = self
            .overlay_card()
            .child(
                div()
                    .text_xl()
                    .font_semibold()
                    .text_color(pal.text)
                    .child(format!(
                        "Delete {count} download{}?",
                        if count == 1 { "" } else { "s" }
                    )),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(pal.muted)
                    .child("Queued transfers stop and disappear from the list."),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        Switch::new("delete-files")
                            .checked(with_files)
                            .on_change(move |on: &bool, _, cx: &mut App| {
                                weak_switch
                                    .update(cx, |this, cx| {
                                        if let Some(p) = this.confirm_delete.as_mut() {
                                            p.delete_files = *on;
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text)
                            .child("Also delete the downloaded files from disk"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("delete-cancel")
                            .outline()
                            .label("Cancel")
                            .on_click({
                                let weak = weak.clone();
                                move |_, _, cx: &mut App| {
                                    weak.update(cx, |this, cx| {
                                        this.confirm_delete = None;
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            }),
                    )
                    .child(
                        Button::new("delete-ok")
                            .primary()
                            .label("Delete")
                            .on_click(move |_, _, cx: &mut App| {
                                weak_ok
                                    .update(cx, |this, cx| {
                                        let Some(pending) = this.confirm_delete.take() else {
                                            return;
                                        };
                                        this.svc.apply(Command::Remove {
                                            ids: pending.ids,
                                            delete_files: pending.delete_files,
                                        });
                                        this.selected.clear();
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    ),
            );
        Self::overlay_shell(card)
    }

    /// Countdown for a scheduled machine shutdown, with a way out.
    fn shutdown_overlay(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let Some(at) = self.shutdown_at else {
            return div().into_any_element();
        };
        let left = at.saturating_sub(crate::engine::now_ms()) / 1000;
        let weak_cancel = cx.entity().downgrade();
        let weak_now = cx.entity().downgrade();
        let card = self
            .overlay_card()
            .child(
                div()
                    .text_xl()
                    .font_semibold()
                    .text_color(pal.warn)
                    .child(format!("Shutting down in {left}s")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(pal.muted)
                    .child(
                        "A download list reached the end of its schedule and is set to power off the PC.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("shutdown-cancel")
                            .outline()
                            .label("Cancel shutdown")
                            .on_click(move |_, _, cx: &mut App| {
                                weak_cancel
                                    .update(cx, |this, cx| {
                                        this.svc.apply(Command::CancelShutdown);
                                        this.shutdown_at = None;
                                        cx.notify();
                                    })
                                    .ok();
                            }),
                    )
                    .child(
                        Button::new("shutdown-now")
                            .primary()
                            .label("Shut down now")
                            .on_click(move |_, _, cx: &mut App| {
                                weak_now
                                    .update(cx, |this, cx| {
                                        force_shutdown();
                                        this.quit(cx);
                                    })
                                    .ok();
                            }),
                    ),
            );
        Self::overlay_shell(card)
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = self.pal;
        let content = match self.page {
            Page::Downloads => crate::downloads::table(self, window, cx),
            Page::Settings => crate::settings::page(self, window, cx),
        };
        let clip_overlay = self.clip_overlay(cx);
        let delete_overlay = self.delete_overlay(cx);
        let shutdown_overlay = self.shutdown_overlay(cx);
        let notice_overlay = self.notice_overlay(cx);
        // Settings takes the whole area beside the sidebar; the download
        // toolbar and status bar only belong to the downloads page.
        let main = match self.page {
            Page::Downloads => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(self.toolbar(window, cx))
                .child(content)
                .child(self.status_bar(cx)),
            Page::Settings => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(content),
        };
        let splitter = self.sidebar_splitter(cx);
        let resize_overlay = if self.resizing_sidebar {
            self.sidebar_resize_overlay(cx)
        } else {
            div().into_any_element()
        };
        let logo_menu = self.logo_menu_overlay(cx);
        div()
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .bg(pal.bg)
            .text_color(pal.text)
            .child(crate::sidebar::sidebar(self, window, cx))
            .child(splitter)
            .child(main)
            .children([
                clip_overlay,
                delete_overlay,
                shutdown_overlay,
                notice_overlay,
                resize_overlay,
                logo_menu,
            ])
    }
}


/// One label/value line in the File Properties window.
fn property_row(
    label: &'static str,
    value: String,
    color: Hsla,
    copyable: bool,
    copied: Option<&str>,
    weak: WeakEntity<MainView>,
    pal: Palette,
) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(10.))
        .py(px(7.))
        .w_full()
        .border_b_1()
        .border_color(with_alpha(pal.line, 0.65))
        .child(
            div()
                .w(px(104.))
                .flex_shrink_0()
                .pt(px(1.))
                .text_xs()
                .text_color(pal.muted)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(color)
                .child(value.clone()),
        );
    if copyable {
        let key = label;
        let text = value;
        let is_copied = copied == Some(label);
        row = row.child(
            Button::new(ElementId::Name(format!("copy-{label}").into()))
                .ghost()
                .compact()
                .icon(if is_copied {
                    IconName::Check
                } else {
                    IconName::Copy
                })
                .text_color(if is_copied { pal.ok } else { pal.muted })
                .tooltip(if is_copied { "Copied" } else { "Copy this value" })
                .on_click(move |_, _, cx: &mut App| {
                    let _ = crate::clipboard::write_text(&text);
                    weak.update(cx, |this, cx| {
                        this.props_copied = Some(key.to_string());
                        cx.notify();
                    })
                    .ok();
                }),
        );
    }
    row.into_any_element()
}

/// Address line in the File Properties window. It is editable only while the
/// download is paused, because resuming has to pick the new source up.
fn address_row(
    input: Entity<InputState>,
    editable: bool,
    error: Option<String>,
    copied: bool,
    weak: WeakEntity<MainView>,
    id: String,
    pal: Palette,
) -> AnyElement {
    let hint = error.clone().unwrap_or_else(|| {
        if editable {
            "Progress is kept — use a mirror or refreshed link of the same file.".to_string()
        } else {
            "Pause the download to change its address.".to_string()
        }
    });
    let hint_color = if error.is_some() { pal.bad } else { pal.muted };
    let mut row = div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(10.))
        .py(px(6.))
        .w_full()
        .border_b_1()
        .border_color(with_alpha(pal.line, 0.65))
        .child(
            div()
                .w(px(104.))
                .flex_shrink_0()
                .pt(px(7.))
                .text_xs()
                .text_color(pal.muted)
                .child("Address"),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(Input::new(&input).disabled(!editable))
                .child(div().text_xs().text_color(hint_color).child(hint)),
        );

    let weak_copy = weak.clone();
    let input_copy = input.clone();
    row = row.child(
        Button::new("copy-Address")
            .ghost()
            .compact()
            .icon(if copied {
                IconName::Check
            } else {
                IconName::Copy
            })
            .text_color(if copied { pal.ok } else { pal.muted })
            .tooltip(if copied { "Copied" } else { "Copy this address" })
            .on_click(move |_, _, cx: &mut App| {
                let text = input_copy.read(cx).value().to_string();
                let _ = crate::clipboard::write_text(&text);
                weak_copy
                    .update(cx, |this, cx| {
                        this.props_copied = Some("Address".to_string());
                        cx.notify();
                    })
                    .ok();
            }),
    );

    if editable {
        let weak_apply = weak.clone();
        let input_apply = input;
        row = row.child(
            Button::new("props-url-apply")
                .outline()
                .compact()
                .label("Apply")
                .tooltip("Use this address for the paused download")
                .on_click(move |_, window, cx: &mut App| {
                    let raw = input_apply.read(cx).value().trim().to_string();
                    weak_apply
                        .update(cx, |this, cx| {
                            this.apply_props_url(&id, raw, window, cx);
                        })
                        .ok();
                }),
        );
    }
    row.into_any_element()
}

/// Friendly file family for the properties window, e.g. "PDF Document".
fn file_type_label(name: &str) -> String {
    // Names may carry a URL fragment (`file.html#section`); only the leading
    // alphanumeric run after the last dot is the extension.
    let ext: String = name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .unwrap_or_default()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    let label = match ext.as_str() {
        "" => "File",
        "zip" | "rar" | "7z" | "tar" | "gz" | "xz" | "bz2" => "Compressed Archive",
        "pdf" => "PDF Document",
        "doc" | "docx" | "rtf" | "odt" => "Word Document",
        "xls" | "xlsx" | "csv" | "ods" => "Spreadsheet",
        "ppt" | "pptx" | "odp" => "Presentation",
        "exe" | "msi" | "msix" => "Application",
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" => "Video File",
        "mp3" | "wav" | "flac" | "m4a" | "ogg" | "aac" => "Audio File",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "ico" | "svg" => "Image",
        "txt" | "md" | "log" => "Text Document",
        "iso" | "img" => "Disc Image",
        _ => return format!("{} File", ext.to_ascii_uppercase()),
    };
    label.to_string()
}

fn logo_menu_item(
    icon: gpui_kit::assets::IconName,
    label: &'static str,
    tone: Hsla,
    pal: Palette,
    weak: WeakEntity<MainView>,
    action: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
) -> AnyElement {
    div()
        .id(ElementId::Name(format!("logo-menu-{label}").into()))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .px(px(12.))
        .py(px(7.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(pal.card_hover))
        .on_click(move |_, window, cx: &mut App| {
            weak.update(cx, |this, cx| {
                this.logo_menu = false;
                action(this, window, cx);
                cx.notify();
            })
            .ok();
        })
        .child(Icon::new(icon).size(px(15.)).text_color(tone))
        .child(div().text_sm().text_color(pal.text).child(label.to_string()))
        .into_any_element()
}

fn guide_row(
    id: &'static str,
    icon: gpui_kit::assets::IconName,
    tone: Hsla,
    title: &'static str,
    subtitle: &'static str,
    pal: Palette,
    weak: WeakEntity<MainView>,
    path: std::path::PathBuf,
) -> AnyElement {
    div()
        .id(ElementId::Name(format!("guide-{id}").into()))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .p(px(10.))
        .rounded(px(8.))
        .border_1()
        .border_color(pal.line)
        .bg(pal.panel)
        .cursor_pointer()
        .hover(|s| s.bg(pal.card_hover))
        .on_click(move |_, window, cx: &mut App| {
            if path.exists() {
                open_in_background(path.clone(), cx);
            } else {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Guide file".into());
                weak.update(cx, |this, cx| {
                    this.notice = Some(Notice {
                        text: format!("{name} was not found next to the application"),
                        error: true,
                    });
                    cx.notify();
                })
                .ok();
            }
            window.close_dialog(cx);
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(34.))
                .rounded(px(8.))
                .bg(with_alpha(tone, 0.12))
                .child(Icon::new(icon).size(px(17.)).text_color(tone)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .child(div().text_sm().text_color(pal.text).child(title))
                .child(div().text_xs().text_color(pal.muted).child(subtitle)),
        )
        .child(
            Icon::new(gpui_kit::assets::IconName::ExternalLink)
                .size(px(14.))
                .text_color(pal.muted),
        )
        .into_any_element()
}

/// Shell-open a path off the UI thread; ShellExecute can pump messages and
/// re-enter GPUI, which crashed the app. A plain thread keeps this independent
/// of the GUI executor, and Explorer/start is a fallback if the shell helper
/// cannot launch the target.
pub(crate) fn open_in_background(target: std::path::PathBuf, _cx: &mut App) {
    std::thread::spawn(move || {
        // Avoid the `open` crate here: its Windows path tries PowerShell first,
        // which may not return. Explorer/start launch the target directly.
        let outcome = if target.is_dir() {
            match std::process::Command::new("explorer.exe")
                .arg(&target)
                .spawn()
            {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("failed: {e}"),
            }
        } else {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let path = target.to_string_lossy().into_owned();
            match std::process::Command::new("cmd")
                .args(["/C", "start", "", &path])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
            {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("failed: {e}"),
            }
        };
        crate::core::log::write(format!("open {target:?} -> {outcome}"));
    });
}

/// The volume a path lives on, such as `G:`.
fn drive_label(dir: &std::path::Path) -> String {
    match dir.components().next() {
        Some(std::path::Component::Prefix(prefix)) => prefix
            .as_os_str()
            .to_string_lossy()
            .trim_end_matches('\\')
            .to_uppercase(),
        _ => "Disk".to_string(),
    }
}

/// Windows shuts down without prompting, matching the egui build.
pub(crate) fn force_shutdown() {
    let _ = std::process::Command::new("shutdown")
        .args(["/s", "/f", "/t", "0"])
        .spawn();
}
