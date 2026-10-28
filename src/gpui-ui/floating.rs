//! Minimal always-on-top card for active downloads.
use crate::app::MainView;
use crate::core::Command;
use crate::engine::Status;
use crate::i18n::{t, tf, Key};
use crate::rtl::{DirEdge as _, Mirror as _};
use crate::theme::{with_alpha, Palette};
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::FocusableExt as _;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::prelude::*;
use gpui_kit::*;
use std::collections::HashMap;
use std::time::Duration;

const CARD_W: f32 = 360.0;
const OUTER_PAD: f32 = 8.0;
const HEADER_H: f32 = 46.0;
const ROW_H: f32 = 70.0;
const IDLE_H: f32 = 48.0;

struct FloatItem {
    id: String,
    name: String,
    pct: f32,
    speed: String,
    eta: String,
    tone: Hsla,
    badge: String,
    badge_color: Hsla,
    paused: bool,
}

pub(crate) struct FloatingView {
    app: WeakEntity<MainView>,
    pal: Palette,
    items: Vec<FloatItem>,
    active: usize,
    total_speed: String,
    idle: bool,
    known: Vec<String>,
    expanded: bool,
    /// Old row index for items that moved in the latest list change.
    move_from: HashMap<String, usize>,
    move_serial: u64,
}

impl FloatingView {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        app: WeakEntity<MainView>,
        pal: Palette,
    ) -> Self {
        let (items, active, speed, pal_now) = app
            .read_with(cx, |main, _| collect(main, &[]))
            .unwrap_or((Vec::new(), 0, 0, pal));
        let known: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        let view = Self {
            app: app.clone(),
            pal: pal_now,
            idle: items.is_empty(),
            items,
            active,
            total_speed: crate::format::fmt_speed(speed),
            known,
            expanded: false,
            move_from: HashMap::new(),
            move_serial: 0,
        };
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(350))
                .await;
            let alive = this
                .update(cx, |this, cx| {
                    let known = this.known.clone();
                    let Ok((items, active, speed, pal)) =
                        app.read_with(cx, |main, _| collect(main, &known))
                    else {
                        return false;
                    };
                    let old_order: Vec<String> =
                        this.items.iter().map(|i| i.id.clone()).collect();
                    let new_order: Vec<String> =
                        items.iter().map(|i| i.id.clone()).collect();
                    if old_order != new_order {
                        this.move_from.clear();
                        for (new_index, item) in items.iter().enumerate() {
                            if let Some(old_index) =
                                old_order.iter().position(|id| id == &item.id)
                            {
                                if old_index != new_index {
                                    this.move_from
                                        .insert(item.id.clone(), old_index);
                                }
                            }
                        }
                        this.move_serial = this.move_serial.wrapping_add(1);
                    }
                    this.known = items.iter().map(|i| i.id.clone()).collect();
                    this.items = items;
                    this.active = active;
                    this.idle = this.items.is_empty();
                    this.total_speed = crate::format::fmt_speed(speed);
                    this.pal = pal;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
            if !alive {
                break;
            }
        })
        .detach();
        view
    }

    fn open_app(app: WeakEntity<MainView>, window: &mut Window, cx: &mut App) {
        app.update(cx, |this, cx| {
            crate::core::sys::restore(this.hwnd.get());
            if let Some(handle) = this.window_handle {
                handle
                    .update(cx, |_, window, _| window.activate_window())
                    .ok();
            }
            // Close the card so the main window is not hidden behind an
            // always-on-top overlay; the status-bar pin brings it back.
            this.floating_window = None;
            this.svc.apply(Command::SetFloatingCard(false));
            cx.notify();
        })
        .ok();
        window.remove_window();
    }

    fn close(app: WeakEntity<MainView>, window: &mut Window, cx: &mut App) {
        app.update(cx, |this, cx| {
            this.floating_window = None;
            this.svc.apply(Command::SetFloatingCard(false));
            cx.notify();
        })
        .ok();
        window.remove_window();
    }
}

fn make_item(main: &MainView, s: &crate::engine::Snapshot, paused: bool) -> FloatItem {
    let pct = s
        .total
        .filter(|t| *t > 0)
        .map(|t| (s.received as f64 / t as f64 * 100.0) as f32)
        .unwrap_or(0.0);
    let ext = s.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let mut badge: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(3)
        .collect();
    if badge.is_empty() {
        badge = "FILE".into();
    } else {
        badge.make_ascii_uppercase();
    }
    FloatItem {
        id: s.id.clone(),
        name: s.name.clone(),
        pct,
        speed: if paused {
            t(Key::StatusPaused).to_string()
        } else if s.speed > 0 {
            crate::format::fmt_speed(s.speed)
        } else {
            "—".into()
        },
        eta: if paused {
            "—".into()
        } else {
            crate::format::eta_text(s)
        },
        tone: if paused {
            main.pal.status(Status::Paused)
        } else {
            main.pal.status(s.status)
        },
        badge,
        badge_color: main.pal.file_color(&s.name),
        paused,
    }
}

fn collect(
    main: &MainView,
    known: &[String],
) -> (Vec<FloatItem>, usize, u64, Palette) {
    let snaps = &main.svc.view().snaps;
    let active: Vec<&crate::engine::Snapshot> = snaps
        .iter()
        .filter(|s| matches!(s.status, Status::Connecting | Status::Downloading))
        .collect();
    let active_ids: Vec<&str> = active.iter().map(|s| s.id.as_str()).collect();
    let mut items: Vec<FloatItem> = active
        .iter()
        .map(|s| make_item(main, s, false))
        .collect();
    // Keep paused downloads that were active while this card was open, so the
    // play button can resume them from the card.
    for id in known {
        if active_ids.iter().any(|active| *active == id.as_str()) {
            continue;
        }
        if let Some(s) = snaps.iter().find(|s| &s.id == id) {
            if s.status == Status::Paused {
                items.push(make_item(main, s, true));
            }
        }
    }
    let count = active.len();
    let speed: u64 = active.iter().map(|s| s.speed).sum();
    (items, count, speed, main.pal)
}

impl Render for FloatingView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = self.pal;
        let view_rows = if self.idle {
            0
        } else if self.expanded {
            self.items.len().min(6)
        } else {
            self.items.len().min(3)
        };
        let shown = if self.idle {
            0
        } else if self.expanded {
            self.items.len()
        } else {
            self.items.len().min(3)
        };
        let items_h = if self.idle {
            IDLE_H
        } else {
            view_rows as f32 * ROW_H
        };
        let line_h = if self.items.len() > 3 { 18.0 } else { 0.0 };
        let desired = HEADER_H + items_h + line_h + OUTER_PAD * 2.0 + 8.0;
        let current = window.viewport_size().height.as_f32();
        if (current - desired).abs() > 0.5 {
            window.resize(size(px(CARD_W + OUTER_PAD * 2.0), px(desired)));
        }

        let mut inner = div()
            .relative()
            .w_full()
            .h(px(if self.idle {
                IDLE_H
            } else {
                shown as f32 * ROW_H
            }));
        if self.idle {
            inner = inner.child(
                div()
                    .h(px(IDLE_H))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(pal.muted)
                    .child(t(Key::FloatIdle)),
            );
        } else {
            for (index, item) in self.items.iter().take(shown).enumerate() {
                let weak = self.app.clone();
                let id = item.id.clone();
                let row =                     div()
                        .id(ElementId::Name(format!("float-{}", item.id).into()))
                        .absolute()
                        .left(px(8.))
                        .right(px(8.))
                        .flex()
                        .flex_col()
                        .h(px(ROW_H))
                        .gap(px(5.))
                        .px(px(8.))
                        .py(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|s| s.bg(pal.card_hover))
                        .on_click(move |event, _, cx: &mut App| {
                            if event.click_count() < 2 {
                                return;
                            }
                            weak.update(cx, |this, cx| {
                                this.selected.clear();
                                this.selected.insert(id.clone());
                                this.list_filter = None;
                                this.page = crate::app::Page::Downloads;
                                this.pending_focus = Some(id.clone());
                                crate::core::sys::restore(this.hwnd.get());
                                if let Some(handle) = this.window_handle {
                                    handle
                                        .update(cx, |_, window, _| window.activate_window())
                                        .ok();
                                }
                                cx.notify();
                            })
                            .ok();
                        })
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .mirror()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .size(px(28.))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(7.))
                                        .bg(with_alpha(item.badge_color, 0.12))
                                        .border_1()
                                        .border_color(with_alpha(item.badge_color, 0.36))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(item.badge_color)
                                                .child(item.badge.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(pal.text)
                                        .ellipsize_ui()
                                        .child(item.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(pal.muted)
                                        .child(crate::text::ltr_isolate(&format!(
                                            "{:.1}%",
                                            item.pct
                                        ))),
                                )
                                .child({
                                    let weak_toggle = self.app.clone();
                                    let id_toggle = item.id.clone();
                                    let paused = item.paused;
                                    div()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                                            cx.stop_propagation()
                                        })
                                        .child(
                                            Button::new(ElementId::Name(format!(
                                                "float-toggle-{}",
                                                item.id
                                            ).into()))
                                            .custom(ButtonCustomVariant::new(cx))
                                            .compact()
                                            .focus_ring(false)
                                            .icon(
                                                Icon::new(if paused {
                                                    gpui_kit::assets::IconName::Play
                                                } else {
                                                    gpui_kit::assets::IconName::Pause
                                                })
                                                .size(px(12.))
                                                .text_color(if paused { pal.ok } else { pal.warn }),
                                            )
                                            .tooltip(if paused {
                                                t(Key::RowStartTip)
                                            } else {
                                                t(Key::RowPauseTip)
                                            })
                                            .on_click(move |_, _, cx: &mut App| {
                                                cx.stop_propagation();
                                                let id = id_toggle.clone();
                                                weak_toggle
                                                    .update(cx, |main, cx| {
                                                        if paused {
                                                            main.svc.apply(Command::Resume(vec![id]));
                                                        } else {
                                                            main.svc.apply(Command::Pause(vec![id]));
                                                        }
                                                        cx.notify();
                                                    })
                                                    .ok();
                                            }),
                                        )
                                }),
                        )
                        .child(
                            div()
                                .h(px(4.))
                                .w_full()
                                .rounded_full()
                                .bg(pal.track)
                                .overflow_hidden()
                                .mirror()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative((item.pct / 100.0).clamp(0.0, 1.0)))
                                        .rounded_full()
                                        .bg(item.tone),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .mirror()
                                .justify_between()
                                .text_xs()
                                .text_color(pal.muted)
                                .child(crate::text::ltr_isolate(&item.eta))
                                .child(crate::text::ltr_isolate(&item.speed)),
                        );
                let y = index as f32 * ROW_H;
                let row = if let Some(&from_index) = self.move_from.get(&item.id) {
                    let start = from_index as f32 * ROW_H;
                    let end = y;
                    let animation_id =
                        format!("float-move-{}-{}", item.id, self.move_serial);
                    row.with_animation(
                        animation_id,
                        Animation::new(Duration::from_millis(240))
                            .with_easing(|t| 1.0 - (1.0 - t) * (1.0 - t)),
                        move |row, progress| {
                            row.top(px(start + (end - start) * progress))
                        },
                    )
                    .into_any_element()
                } else {
                    row.top(px(y)).into_any_element()
                };
                inner = inner.child(row);
            }
        }

        let mut items_area = div()
            .id("float-rows")
            .relative()
            .w_full()
            .h(px(items_h))
            .overflow_hidden();
        if self.expanded && self.items.len() > 6 {
            items_area = items_area.overflow_y_scroll();
        }
        let items_area = items_area.child(inner);

        let line = if self.items.len() > 3 {
            let expand = !self.expanded;
            let label = if expand {
                tf(
                    Key::FloatMore,
                    &[&crate::text::ltr_isolate(
                        &(self.items.len() - 3).to_string(),
                    )],
                )
            } else {
                t(Key::FloatLess).to_string()
            };
            Some(
                div()
                    .id("float-more")
                    .w_full()
                    .h(px(line_h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(pal.muted)
                    .cursor_pointer()
                    .hover(|s| s.text_color(pal.accent))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.expanded = expand;
                        cx.notify();
                    }))
                    .child(label),
            )
        } else {
            None
        };
        let line_el = if let Some(line) = line {
            line.into_any_element()
        } else {
            div().into_any_element()
        };

        let subtitle = if self.active > 0 {
            format!(
                "{}  ·  {}",
                tf(
                    Key::FloatActive,
                    &[&crate::text::ltr_isolate(&self.active.to_string())]
                ),
                self.total_speed
            )
        } else if !self.items.is_empty() {
            tf(
                Key::FloatPaused,
                &[&crate::text::ltr_isolate(&self.items.len().to_string())],
            )
        } else {
            t(Key::FloatIdle).to_string()
        };

        let weak_open = self.app.clone();
        let weak_close = self.app.clone();
        div()
            .size_full()
            .p(px(OUTER_PAD))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .rounded(px(14.))
                    .bg(pal.card)
                    .border_1()
                    .border_color(pal.line)
                    .shadow(vec![
                        BoxShadow::new(px(0.), px(12.), crate::theme::alpha(0x000000, 0.45))
                            .blur_radius(px(26.))
                            .spread_radius(px(-8.)),
                    ])
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .mirror()
                            .items_center()
                            .gap(px(9.))
                            .h(px(HEADER_H))
                            .px(px(12.))
                            .border_b_1()
                            .border_color(with_alpha(pal.line, 0.75))
                            .child(
                                div()
                                    .id("float-drag")
                                    .flex()
                                    .flex_row()
                                    .mirror()
                                    .flex_1()
                                    .min_w_0()
                                    .items_center()
                                    .gap(px(9.))
                                    .h_full()
                                    .window_control_area(WindowControlArea::Drag)
                                    .child(
                                        div()
                                            .size(px(26.))
                                            .flex_shrink_0()
                                            .rounded(px(7.))
                                            .bg(with_alpha(pal.accent, 0.14))
                                            .border_1()
                                            .border_color(with_alpha(pal.accent, 0.4))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                Icon::new(IconName::ArrowDown)
                                                    .size(px(14.))
                                                    .text_color(pal.accent),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .min_w_0()
                                            .gap(px(1.))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child(t(Key::FloatTitle)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(pal.muted)
                                                    .child(subtitle),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(2.))
                                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| {
                                        cx.stop_propagation()
                                    })
                                    .child(
                                        Button::new("float-open-app")
                                            .custom(ButtonCustomVariant::new(cx))
                                            .compact()
                                            .focus_ring(false)
                                            .icon(
                                                Icon::new(IconName::ExternalLink)
                                                    .size(px(14.))
                                                    .text_color(pal.muted),
                                            )
                                            .tooltip(t(Key::FloatOpenAppTip))
                                            .on_click(move |_, window, cx: &mut App| {
                                                FloatingView::open_app(
                                                    weak_open.clone(),
                                                    window,
                                                    cx,
                                                )
                                            }),
                                    )
                                    .child(
                                        Button::new("float-close")
                                            .custom(ButtonCustomVariant::new(cx))
                                            .compact()
                                            .focus_ring(false)
                                            .icon(
                                                Icon::new(IconName::Close)
                                                    .size(px(14.))
                                                    .text_color(pal.muted),
                                            )
                                            .tooltip(t(Key::CommonClose))
                                            .on_click(move |_, window, cx: &mut App| {
                                                FloatingView::close(
                                                    weak_close.clone(),
                                                    window,
                                                    cx,
                                                )
                                            }),
                                    ),
                            ),
                    )
                    .child(items_area)
                    .child(line_el),
            )
    }
}
