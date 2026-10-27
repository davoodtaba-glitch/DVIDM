//! Sidebar navigation: categories, download lists and the settings entry.
use crate::app::{MainView, Page};
use crate::core::Command;
use crate::engine::{Category, Status};
use crate::i18n::{t, tf, Key};
use crate::rtl::{AlignEnd as _, DirEdge as _, Mirror as _};
use crate::text::{display_fa_ui, ltr_isolate};
use crate::theme::{with_alpha, Palette};
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, WindowExt as _};
use gpui_kit::assets::IconName as CatIcon;

/// The application mark, shipped as a PNG and decoded once per process.
fn logo_source() -> ImageSource {
    use std::sync::{Arc, OnceLock};
    static LOGO: OnceLock<Arc<Image>> = OnceLock::new();
    let image = LOGO.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../resource/icon.png").to_vec(),
        ))
    });
    ImageSource::Image(image.clone())
}

pub(crate) fn sidebar(
    view: &mut MainView,
    _window: &mut Window,
    cx: &mut Context<MainView>,
) -> AnyElement {
    let pal = view.pal;
    let snaps = view.svc.view().snaps.clone();
    let count = |cat: Category| -> usize {
        snaps
            .iter()
            .filter(|s| crate::downloads::in_view(None, cat, s))
            .count()
    };
    let active_list = view.list_filter.clone();
    let page = view.page;
    let fold = view.nav_fold;
    let weak = cx.entity().downgrade();

    let mut nav = div().flex().flex_col().gap(px(2.)).px(px(8.));
    for (cat, label, icon, tone) in [
        (Category::All, t(Key::NavAll), CatIcon::Layers, pal.accent),
        (Category::Active, t(Key::NavInProgress), CatIcon::Download, pal.info),
        (Category::Done, t(Key::NavFinished), CatIcon::CircleCheck, pal.ok),
    ] {
        nav = nav.child(nav_item(
            format!("cat-{label}"),
            label,
            icon,
            tone,
            count(cat),
            page == Page::Downloads && active_list.is_none() && view.filter == cat,
            pal,
            {
                let weak = weak.clone();
                move |cx: &mut App| {
                    weak.update(cx, |this, cx| {
                        this.filter = cat;
                        this.list_filter = None;
                        this.page = Page::Downloads;
                        cx.notify();
                    })
                    .ok();
                }
            },
        ));
    }

    let mut library = div().flex().flex_col().gap(px(2.)).px(px(8.));
    for (cat, label, icon, tone) in [
        (Category::Compressed, t(Key::NavArchives), CatIcon::Archive, pal.warn),
        (Category::Documents, t(Key::NavDocuments), CatIcon::FileText, pal.accent),
        (Category::Media, t(Key::NavMedia), CatIcon::Clapperboard, pal.paused),
        (Category::Programs, t(Key::NavApps), CatIcon::Package, pal.list_color(1)),
        (Category::Other, t(Key::NavOther), CatIcon::Files, pal.muted),
    ] {
        library = library.child(nav_item(
            format!("cat-{label}"),
            label,
            icon,
            tone,
            count(cat),
            page == Page::Downloads && active_list.is_none() && view.filter == cat,
            pal,
            {
                let weak = weak.clone();
                move |cx: &mut App| {
                    weak.update(cx, |this, cx| {
                        this.filter = cat;
                        this.list_filter = None;
                        this.page = Page::Downloads;
                        cx.notify();
                    })
                    .ok();
                }
            },
        ));
    }

    let mut lists = div().flex().flex_col().gap(px(2.)).px(px(8.));
    for list in view.svc.lists() {
        let name = list.name.clone();
        let n = list
            .items
            .iter()
            .filter(|id| snaps.iter().any(|s| &s.id == *id))
            .count();
        let color = pal.list_color(list.color);
        let scheduled = list.sched_start.is_some() && list.sched_end.is_some();
        // The indicator describes the queue: completed rows do not count.
        let mut any_unfinished = false;
        let mut all_unfinished_paused = true;
        let mut has_active = false;
        for id in &list.items {
            let Some(s) = snaps.iter().find(|s| &s.id == id) else {
                continue;
            };
            if s.status == Status::Done {
                continue;
            }
            any_unfinished = true;
            if matches!(s.status, Status::Connecting | Status::Downloading) {
                has_active = true;
            }
            if s.status != Status::Paused {
                all_unfinished_paused = false;
            }
        }
        let all_paused = any_unfinished && all_unfinished_paused;
        let state_tip = if has_active {
            t(Key::ListTipActive)
        } else if list.items.is_empty() {
            t(Key::ListTipEmpty)
        } else if !any_unfinished {
            t(Key::ListTipAllDone)
        } else if all_paused {
            t(Key::ListTipAllPaused)
        } else {
            t(Key::ListTipIdle)
        };
        let sched_tip = if scheduled {
            let start = list.sched_start.unwrap_or(0);
            let end = list.sched_end.unwrap_or(0);
            let now = crate::core::sys::local_minutes();
            tf(
                Key::ListTipSchedule,
                &[
                    &human_minutes(crate::core::schedule::until_window_open(now, start, end)),
                    &human_minutes(crate::core::schedule::window_len(start, end)),
                ],
            )
        } else {
            t(Key::ListTipNoSchedule).to_string()
        };
        let selected = page == Page::Downloads && active_list.as_deref() == Some(name.as_str());
        let weak_click = weak.clone();
        let weak_delete = weak.clone();
        let name_click = name.clone();
        let name_delete = name.clone();
        let is_default = list.is_default;
        lists = lists.child(
            div()
                .id(ElementId::Name(format!("list-{name}").into()))
                .flex()
                .flex_row()
                .mirror()
                .items_center()
                .gap(px(6.))
                .h(px(30.))
                .padding_start(px(22.))
                .padding_end(px(4.))
                .rounded(px(7.))
                .cursor_pointer()
                .bg(if selected {
                    pal.accent_dim
                } else {
                    pal.panel
                })
                .hover(|s| s.bg(pal.row_hover))
                .on_click(move |_, _, cx: &mut App| {
                    let name = name_click.clone();
                    weak_click
                        .update(cx, |this, cx| {
                            this.list_filter = Some(name);
                            this.page = Page::Downloads;
                            cx.notify();
                        })
                        .ok();
                })
                // Right-click opens the list options (rename, colour, cap,
                // schedule); the kit's context menu never fired on these rows.
                .on_mouse_down(MouseButton::Right, {
                    let weak = weak.clone();
                    let name = name.clone();
                    move |_, window, cx: &mut App| {
                        let name = name.clone();
                        weak.update(cx, |this, cx| {
                            this.list_options_dialog(name, window, cx);
                        })
                        .ok();
                    }
                })
                // Fixed slots keep the icon, count, options and delete columns
                // aligned across rows, whether a list has a count or not.
                .child(
                    div()
                        .w(px(16.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(gpui_kit::assets::IconName::List)
                                .size(px(13.))
                                .text_color(color),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .ellipsize_ui()
                        .text_sm()
                        .text_color(if selected { pal.text } else { pal.muted })
                        .child(display_fa_ui(&name)),
                )
                // Play while any member is transferring; otherwise pause, even
                // for an empty or fully completed list.
                .child(
                    div()
                        .w(px(20.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Button::new(ElementId::Name(format!("state-{name}").into()))
                                .ghost()
                                .compact()
                                .icon(
                                    Icon::new(if has_active {
                                        CatIcon::Play
                                    } else {
                                        CatIcon::Pause
                                    })
                                    .size(px(11.))
                                    .text_color(if has_active {
                                        pal.ok
                                    } else if all_paused {
                                        pal.warn
                                    } else {
                                        with_alpha(pal.muted, 0.45)
                                    }),
                                )
                                .tooltip(state_tip),
                        ),
                )
                // The clock is always there; dim until a schedule is set.
                .child(
                    div()
                        .w(px(20.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Button::new(ElementId::Name(format!("sched-{name}").into()))
                                .ghost()
                                .compact()
                                .icon(
                                    Icon::new(gpui_kit::assets::IconName::Clock)
                                        .size(px(12.))
                                        .text_color(if scheduled {
                                            pal.warn
                                        } else {
                                            with_alpha(pal.muted, 0.3)
                                        }),
                                )
                                .tooltip(sched_tip),
                        ),
                )
                .child(
                    div()
                        .w(px(20.))
                        .flex_shrink_0()
                        .align_end()
                        .text_xs()
                        .text_color(if selected { pal.text } else { pal.muted })
                        .child(if n > 0 {
                            ltr_isolate(&n.to_string())
                        } else {
                            String::new()
                        }),
                )
                .child(
                    div()
                        .w(px(24.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child({
                            let weak_opts = weak.clone();
                            let name_opts = name.clone();
                            Button::new(ElementId::Name(format!("opts-{name}").into()))
                                .ghost()
                                .compact()
                                .icon(IconName::EllipsisVertical)
                                .tooltip(t(Key::ListTipOptions))
                                .on_click(move |_, window, cx: &mut App| {
                                    let name = name_opts.clone();
                                    weak_opts
                                        .update(cx, |this, cx| {
                                            this.list_options_dialog(name, window, cx);
                                        })
                                        .ok();
                                    cx.stop_propagation();
                                })
                        }),
                )
                .when(!is_default, |d| {
                    d.child(
                        div()
                            .w(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Button::new(ElementId::Name(format!("del-{name}").into()))
                                    .ghost()
                                    .compact()
                                    .icon(IconName::Delete)
                                    .text_color(pal.bad)
                                    .tooltip(t(Key::ListTipDelete))
                                    .on_click(move |event, window, cx: &mut App| {
                                        let name = name_delete.clone();
                                        if event.modifiers().shift {
                                            weak_delete
                                                .update(cx, |this, cx| {
                                                    this.svc.apply(Command::DeleteList(name));
                                                    this.list_filter = None;
                                                    cx.notify();
                                                })
                                                .ok();
                                        } else {
                                            weak_delete
                                                .update(cx, |this, cx| {
                                                    this.confirm_delete_list(name, window, cx);
                                                })
                                                .ok();
                                        }
                                        cx.stop_propagation();
                                    }),
                            ),
                    )
                })
                // The default list cannot be deleted, but its slot stays so the
                // other columns do not shift.
                .when(is_default, |d| d.child(div().w(px(24.)).flex_shrink_0())),
        );
    }

    let new_list_button = div()
        .id("new-list")
        .flex()
        .flex_row()
        .mirror()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(30.))
        .rounded(px(7.))
        .cursor_pointer()
        .border_1()
        .border_color(with_alpha(pal.line, 0.9))
        .bg(with_alpha(pal.accent, 0.05))
        .hover(|s| {
            s.bg(with_alpha(pal.accent, 0.12))
                .border_color(with_alpha(pal.accent, 0.45))
        })
        .on_click(cx.listener(|this, _, window, cx| this.new_list_dialog(window, cx)))
        .child(
            Icon::new(IconName::Plus)
                .size(px(13.))
                .text_color(with_alpha(pal.muted, 0.9)),
        )
        .child(
            div()
                .text_xs()
                .text_color(with_alpha(pal.muted, 0.95))
                .child(t(Key::NavNewList)),
        );

    let settings_active = page == Page::Settings;
    let weak_settings = weak.clone();

    div()
        .w(px(view.sidebar_width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(pal.panel)
        .when(crate::rtl::on(), |d| d.border_l_1())
        .when(!crate::rtl::on(), |d| d.border_r_1())
        .border_color(pal.line)
        .child(
            div()
                .flex()
                .flex_row()
                .mirror()
                .items_center()
                .gap(px(10.))
                .px(px(18.))
                .pt(px(18.))
                .pb(px(16.))
                .child(
                    div()
                        .id("brand-menu")
                        .flex_shrink_0()
                        .flex()
                        .flex_row()
                        .mirror()
                        .items_center()
                        .gap(px(10.))
                        .px(px(4.))
                        .py(px(2.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|s| s.bg(pal.card_hover))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.logo_menu = !this.logo_menu;
                            cx.notify();
                        }))
                        .child(img(logo_source()).w(px(38.)).h(px(38.)).flex_shrink_0())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(pal.text)
                                        .whitespace_nowrap()
                                        .child(t(Key::BrandLine1)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(pal.muted)
                                        .whitespace_nowrap()
                                        .child(t(Key::BrandLine2)),
                                ),
                        ),
                )
                .child(div().flex_1().min_w_0())
                .child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(pal.muted)
                        .child(env!("CARGO_PKG_VERSION")),
                ),
        )
        .child(section_header(t(Key::NavLibrary), fold[0], weak.clone(), 0, pal))
        .when(fold[0], |d| d.child(nav))
        .child(div().h(px(10.)))
        .child(section_header(t(Key::NavCategories), fold[1], weak.clone(), 1, pal))
        .when(fold[1], |d| d.child(library))
        .child(div().h(px(10.)))
        .child(section_header(t(Key::NavLists), fold[2], weak.clone(), 2, pal))
        .child(
            div()
                .id("lists-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .when(fold[2], |d| d.child(lists)),
        )
        .child(
            div()
                .px(px(10.))
                .pb(px(8.))
                .child(new_list_button),
        )
        .child(
            div()
                .px(px(8.))
                .pb(px(12.))
                .child(
                    div()
                        .id("settings-nav")
                        .flex()
                        .flex_row()
                        .mirror()
                        .items_center()
                        .justify_center()
                        .gap(px(8.))
                        .h(px(30.))
                        .rounded(px(7.))
                        .cursor_pointer()
                        .bg(if settings_active {
                            pal.accent_dim
                        } else {
                            pal.panel
                        })
                        .hover(|s| s.bg(pal.row_hover))
                        .on_click(move |_, window, cx: &mut App| {
                            weak_settings
                                .update(cx, |this, cx| {
                                    if this.page == Page::Settings {
                                        // The same entry closes the page again.
                                        this.flush_direct_formats(true, cx);
                                        this.page = Page::Downloads;
                                        cx.notify();
                                    } else {
                                        this.open_settings(window, cx);
                                    }
                                })
                                .ok();
                        })
                        .child(
                            Icon::new(if settings_active {
                                if crate::rtl::on() {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronLeft
                                }
                            } else {
                                IconName::Settings
                            })
                            .size(px(15.))
                            .text_color(if settings_active { pal.accent } else { pal.muted }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(if settings_active { pal.text } else { pal.muted })
                                .child(if settings_active {
                                    t(Key::NavCloseSettings)
                                } else {
                                    t(Key::NavSettings)
                                }),
                        ),
                ),
        )
        .into_any_element()
}

/// Live schedule summary under the list options fields. It listens to the two
/// clock inputs and to the schedule switch so the text follows every edit.
struct SchedulePreview {
    pal: Palette,
    start: Entity<InputState>,
    end: Entity<InputState>,
    enabled: Entity<bool>,
    _start_sub: Subscription,
    _end_sub: Subscription,
    _enabled_sub: Subscription,
}

impl SchedulePreview {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        pal: Palette,
        start: Entity<InputState>,
        end: Entity<InputState>,
        enabled: Entity<bool>,
    ) -> Self {
        let start_weak = cx.entity().downgrade();
        let start_sub = window.subscribe(&start, cx, move |_, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                start_weak.update(cx, |_, cx| cx.notify()).ok();
            }
        });
        let end_weak = cx.entity().downgrade();
        let end_sub = window.subscribe(&end, cx, move |_, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                end_weak.update(cx, |_, cx| cx.notify()).ok();
            }
        });
        let enabled_sub = cx.observe(&enabled, |_, _, cx| cx.notify());
        Self {
            pal,
            start,
            end,
            enabled,
            _start_sub: start_sub,
            _end_sub: end_sub,
            _enabled_sub: enabled_sub,
        }
    }

    fn lines(&self, cx: &App) -> Option<(String, String)> {
        let start = crate::core::schedule::parse_hm(&crate::text::from_fa_digits(self.start.read(cx).value().as_ref()))?;
        let end = crate::core::schedule::parse_hm(&crate::text::from_fa_digits(self.end.read(cx).value().as_ref()))?;
        let now = crate::core::sys::local_minutes();
        Some((
            tf(
                Key::ScheduleStartsIn,
                &[&human_minutes(crate::core::schedule::until_window_open(now, start, end))],
            ),
            tf(
                Key::ScheduleRunsFor,
                &[&human_minutes(crate::core::schedule::window_len(start, end))],
            ),
        ))
    }
}

impl Render for SchedulePreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = self.pal;
        let enabled = *self.enabled.read(cx);
        let body = if let Some((start_line, run_line)) = self.lines(cx) {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .w_full()
                .align_end()
                .child(div().text_xs().text_color(pal.muted).align_end().child(start_line))
                .child(div().text_xs().text_color(pal.muted).align_end().child(run_line))
                .into_any_element()
        } else {
            div()
                .w_full()
                .text_xs()
                .text_color(pal.muted)
                .align_end()
                .child(t(Key::ScheduleEnterTimes))
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .w_full()
            .opacity(if enabled { 1.0 } else { 0.45 })
            .child(body)
            .into_any_element()
    }
}

/// Minutes as natural-language units, with correct singular and plural forms.
fn human_minutes(minutes: u32) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    let mut parts = Vec::new();
    if hours > 0 {
        let n = ltr_isolate(&hours.to_string());
        parts.push(if hours == 1 {
            tf(Key::DurationHour, &[&n])
        } else {
            tf(Key::DurationHours, &[&n])
        });
    }
    if mins > 0 || hours == 0 {
        let n = ltr_isolate(&mins.to_string());
        parts.push(if mins == 1 {
            tf(Key::DurationMinute, &[&n])
        } else {
            tf(Key::DurationMinutes, &[&n])
        });
    }
    parts.join(" ")
}

/// Keep a clock field to four digits and place the colon: `0800` becomes
/// `08:00`, and a one-digit hour such as `800` becomes `8:00`. Anything that
/// is not a digit is dropped, so the field only ever accepts numbers.
fn normalize_clock(text: &str) -> String {
    let digits: String = text
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(4)
        .collect();
    if digits.len() < 3 {
        return digits;
    }
    let split = if digits[..2].parse::<u32>().map(|h| h > 23).unwrap_or(false) {
        1
    } else {
        2
    };
    let (hour, minute) = digits.split_at(split);
    format!("{hour}:{minute}")
}

/// Rewrite a clock field into `HH:MM` after every keystroke.
fn watch_clock(
    field: &Entity<InputState>,
    cx: &mut Context<MainView>,
    window: &mut Window,
) -> Subscription {
    let state = field.clone();
    window.subscribe(field, cx, move |_, event: &InputEvent, window, cx| {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        let state = state.clone();
        // The field is still borrowed while its change event is delivered, so
        // rewrite the text once the current update has finished.
        window.defer(cx, move |window, cx| {
            let current = state.read(cx).value().to_string();
            let fixed = crate::text::fa_input(&normalize_clock(&crate::text::from_fa_digits(&current)));
            if fixed != current {
                state.update(cx, |field, cx| field.set_value(fixed, window, cx));
            }
        });
    })
}

/// A switch drawn by hand: the kit `Switch` does not take clicks in this app.
fn toggle_switch(
    id: &'static str,
    on: bool,
    pal: Palette,
    change: impl Fn(bool, &mut App) + 'static,
) -> AnyElement {
    let change = std::rc::Rc::new(change);
    div()
        .id(ElementId::Name(id.into()))
        .w(px(30.))
        .h(px(16.))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(if on {
            with_alpha(pal.accent, 0.7)
        } else {
            pal.line
        })
        .bg(if on { pal.accent } else { pal.track })
        .flex()
        .items_center()
        .cursor_pointer()
        .on_click(move |_, _, cx: &mut App| change(!on, cx))
        .child(
            div()
                .size(px(10.))
                .rounded_full()
                .bg(if on { pal.bg } else { pal.muted })
                .ml(px(crate::rtl::switch_thumb_ml(on))),
        )
        .into_any_element()
}

fn field_label(label: impl AsRef<str>, pal: Palette) -> AnyElement {
    div()
        .w_full()
        .text_xs()
        .text_color(pal.muted)
        .align_end()
        .child(label.as_ref().to_string())
        .into_any_element()
}

/// A collapsible section title: the whole row toggles the group open or shut.
fn section_header(
    label: impl AsRef<str>,
    open: bool,
    weak: WeakEntity<MainView>,
    idx: usize,
    pal: Palette,
) -> AnyElement {
    let label = label.as_ref();
    div()
        .id(ElementId::Name(format!("section-{label}").into()))
        .flex()
        .flex_row()
        .mirror()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .mx(px(8.))
        .mt(px(6.))
        .px(px(6.))
        .rounded(px(7.))
        .cursor_pointer()
        .hover(|s| s.bg(pal.row_hover))
        .on_click(move |_, _, cx: &mut App| {
            weak.update(cx, |this, cx| {
                this.nav_fold[idx] = !this.nav_fold[idx];
                cx.notify();
            })
            .ok();
        })
        // Accent tick marks these as group titles rather than nav rows.
        .child(
            div()
                .w(px(3.))
                .h(px(13.))
                .flex_shrink_0()
                .rounded(px(2.))
                .bg(if open {
                    pal.accent
                } else {
                    with_alpha(pal.accent, 0.35)
                }),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(if open {
                    with_alpha(pal.text, 0.82)
                } else {
                    pal.muted
                })
                .child(label.to_string()),
        )
        .child(div().flex_1())
        .child(
            Icon::new(if open {
                IconName::ChevronDown
            } else if crate::rtl::on() {
                IconName::ChevronLeft
            } else {
                IconName::ChevronRight
            })
            .size(px(12.))
            .text_color(with_alpha(pal.muted, 0.85)),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn nav_item(
    id: String,
    label: impl AsRef<str>,
    icon: CatIcon,
    tone: Hsla,
    count: usize,
    active: bool,
    pal: Palette,
    on_click: impl Fn(&mut App) + 'static,
) -> AnyElement {
    let label = label.as_ref().to_string();
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .flex_row()
        .mirror()
        .items_center()
        .gap(px(8.))
        .h(px(30.))
        .padding_start(px(22.))
        .padding_end(px(9.))
        .rounded(px(7.))
        .cursor_pointer()
        .bg(if active { pal.accent_dim } else { pal.panel })
        .hover(|s| s.bg(pal.row_hover))
        .on_click(move |_, _, cx: &mut App| on_click(cx))
        .child(
            div()
                .w(px(16.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .size(px(14.))
                        .text_color(if active {
                            tone
                        } else {
                            with_alpha(tone, 0.8)
                        }),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(if active { pal.text } else { pal.muted })
                .child(label.to_string()),
        )
        .child({
            let cell = div()
                .w(px(28.))
                .flex_shrink_0()
                .text_xs()
                .text_color(if active { pal.text } else { pal.muted })
                .child(if count > 0 {
                    ltr_isolate(&count.to_string())
                } else {
                    String::new()
                });
            if crate::rtl::on() {
                cell
            } else {
                cell.text_right()
            }
        })
        .into_any_element()
}

impl MainView {
    /// Rename a list, cap its parallel downloads and set its time window.
    /// Write the list's unfinished URLs to a text file.
    fn export_list(&mut self, name: String, _window: &mut Window, cx: &mut Context<Self>) {
        let ids = self
            .svc
            .lists()
            .iter()
            .find(|l| l.name == name)
            .map(|l| l.items.clone())
            .unwrap_or_default();
        let urls: Vec<String> = ids
            .iter()
            .filter_map(|id| self.svc.view().snaps.iter().find(|s| &s.id == id))
            .filter(|s| s.status != Status::Done)
            .map(|s| s.url.clone())
            .collect();
        if urls.is_empty() {
            self.show_notice(t(Key::ListExportEmpty).to_string(), true, cx);
            return;
        }
        let count = urls.len();
        let default_name = format!("{}.txt", name);
        let text = urls.join("\n");
        cx.spawn(async move |this, cx| {
            let picked = smol::unblock(move || {
                rfd::FileDialog::new()
                    .set_file_name(default_name)
                    .add_filter(t(Key::DialogFilterText), &["txt"])
                    .save_file()
            })
            .await;
            let Some(path) = picked else {
                return;
            };
            let result = std::fs::write(&path, text);
            let failed = result.is_err();
            this.update(cx, |this, cx| {
                let text = match result {
                    Ok(()) => tf(
                        Key::ListExportSaved,
                        &[
                            &ltr_isolate(&count.to_string()),
                            &path.display().to_string(),
                        ],
                    ),
                    Err(e) => tf(Key::ListExportFailed, &[&e.to_string()]),
                };
                this.show_notice(text, failed, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Read URLs from chosen text files into the clipboard-style review window.
    fn import_list(&mut self, name: String, _window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let picked = smol::unblock(|| {
                rfd::FileDialog::new()
                    .add_filter(t(Key::DialogFilterText), &["txt"])
                    .pick_files()
            })
            .await;
            let Some(paths) = picked else {
                return;
            };
            let mut urls = Vec::new();
            for path in paths {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    urls.extend(crate::clipboard::extract_urls(&text));
                }
            }
            let mut seen = std::collections::HashSet::new();
            urls.retain(|url| seen.insert(url.clone()));
            this.update(cx, |this, cx| {
                if urls.is_empty() {
                    this.show_notice(t(Key::ListImportEmpty).to_string(), true, cx);
                } else {
                    this.clip_list = Some(name.clone());
                    this.open_clip(urls, t(Key::ClipSourceFile), cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn list_options_dialog(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(list) = self
            .svc
            .lists()
            .iter()
            .find(|l| l.name == name)
            .cloned()
        else {
            return;
        };
        let pal = self.pal;
        self.list_opts_cap = list.max_parallel;
        self.list_opts_color = list.color;
        self.list_opts_shutdown = list.shutdown_at_end;
        self.list_opts_sched = list.sched_start.is_some() && list.sched_end.is_some();
        let sched_signal = cx.new(|_| self.list_opts_sched);
        let shutdown_at_end = list.shutdown_at_end;
        let selecting = list.name.clone();
        let running = self
            .svc
            .view()
            .snaps
            .iter()
            .filter(|s| s.status == crate::engine::Status::Downloading)
            .filter(|s| {
                self.svc
                    .lists()
                    .iter()
                    .find(|l| l.name == selecting)
                    .map(|l| l.items.iter().any(|id| id == &s.id))
                    .unwrap_or(false)
            })
            .count();
        let members = list.items.len();

        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(t(Key::ListNamePlaceholder)));
        name_input.update(cx, |state, cx| {
            state.set_value(crate::text::fa_input(&name), window, cx)
        });
        // A typed value beats the presets; empty means "no cap".
        let cap_input = cx.new(|cx| InputState::new(window, cx).placeholder(t(Key::ListCapCustomPlaceholder)));
        if list.max_parallel > 0 {
            cap_input.update(cx, |state, cx| {
                state.set_value(crate::text::fa_input(&list.max_parallel.to_string()), window, cx)
            });
        }
        let start = cx.new(|cx| InputState::new(window, cx).placeholder(crate::text::fa_input("08:00")));
        let end = cx.new(|cx| InputState::new(window, cx).placeholder(crate::text::fa_input("22:00")));
        start.update(cx, |state, cx| {
            let text = list
                .sched_start
                .map(crate::core::schedule::fmt_hm)
                .unwrap_or_else(|| "08:00".into());
            state.set_value(crate::text::fa_input(&text), window, cx)
        });
        end.update(cx, |state, cx| {
            let text = list
                .sched_end
                .map(crate::core::schedule::fmt_hm)
                .unwrap_or_else(|| "22:00".into());
            state.set_value(crate::text::fa_input(&text), window, cx)
        });
        // Keep the colon in place as the time is typed: `0800` -> `08:00`.
        self.clock_subs = vec![
            watch_clock(&start, cx, window),
            watch_clock(&end, cx, window),
        ];
        let preview = cx.new(|cx| {
            SchedulePreview::new(
                window,
                cx,
                pal,
                start.clone(),
                end.clone(),
                sched_signal.clone(),
            )
        });

        let weak = cx.entity().downgrade();
        let cap_field_preset = cap_input.clone();
        let cap_field_render = cap_input.clone();
        let cap_field_save_src = cap_input.clone();
        window.open_dialog(cx, move |dialog, _window, cx| {
            // The typed count drives the preset highlight, so they never
            // disagree once a custom value is entered.
            let typed_cap = crate::text::from_fa_digits(cap_field_render.read(cx).value().trim());
            let cap = match typed_cap.parse::<u32>() {
                Ok(n) => n,
                Err(_) if typed_cap.is_empty() => 0,
                Err(_) => weak
                    .read_with(cx, |this, _| this.list_opts_cap)
                    .unwrap_or(0),
            };
            let sched_on = weak
                .read_with(cx, |this, _| this.list_opts_sched)
                .unwrap_or(false);
            // The dialog builder is `Fn`, so each rebuild takes its own handle.
            let cap_field_save = cap_field_save_src.clone();
            let mut caps = div().w_full().flex().flex_row().mirror().items_center().gap(px(4.));
            for (value, label) in [
                (0u32, t(Key::ListCapNone)),
                (1, ltr_isolate("1")),
                (2, ltr_isolate("2")),
                (4, ltr_isolate("4")),
                (8, ltr_isolate("8")),
            ] {
                let weak_cap = weak.clone();
                let cap_field = cap_field_preset.clone();
                caps = caps.child(
                    Button::new(ElementId::Name(format!("cap-{value}").into()))
                        .when(cap == value, |b| b.primary())
                        .when(cap != value, |b| b.outline())
                        .label(label)
                        .on_click(move |_, window, cx: &mut App| {
                            // Keep the typed field in step with the preset.
                            cap_field.update(cx, |state, cx| {
                                let text = if value == 0 {
                                    String::new()
                                } else {
                                    crate::text::fa_input(&value.to_string())
                                };
                                state.set_value(text, window, cx)
                            });
                            weak_cap
                                .update(cx, |this, cx| {
                                    this.list_opts_cap = value;
                                    cx.notify();
                                })
                                .ok();
                        }),
                );
            }
            caps = caps
                .child(div().w(px(18.)))
                .child(
                    div()
                        .text_sm()
                        .text_color(pal.muted)
                        .child(t(Key::CommonCustom)),
                )
                .child(div().w(px(72.)).child(Input::new(&cap_field_render)));

            let mut swatches = div().w_full().flex().flex_row().mirror().gap(px(6.));
            let current_color = weak
                .read_with(cx, |this, _| this.list_opts_color)
                .unwrap_or(0);
            for i in 0..8u8 {
                let tone = pal.list_color(i);
                let weak_swatch = weak.clone();
                swatches = swatches.child(
                    div()
                        .id(ElementId::Name(format!("list-color-{i}").into()))
                        .size(px(20.))
                        .rounded_full()
                        .bg(tone)
                        .border_2()
                        .border_color(if i == current_color {
                            pal.text
                        } else {
                            pal.card
                        })
                        .hover(|s| s.border_color(pal.accent))
                        .on_click(move |_, _, cx: &mut App| {
                            weak_swatch
                                .update(cx, |this, cx| {
                                    this.list_opts_color = i;
                                    cx.notify();
                                })
                                .ok();
                        }),
                );
            }

            let weak_ok = weak.clone();
            let name_ok = name.clone();
            let name_field = name_input.clone();
            let start_field = start.clone();
            let end_field = end.clone();
            dialog
                .title(tf(Key::ListOptionsTitle, &[&name]))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .w_full()
                        .align_end()
                        .child(field_label(t(Key::ListFieldName), pal))
                        .child(Input::new(&name_input))
                        .child(field_label(t(Key::ListFieldCap), pal))
                        .child(caps)
                        .child(field_label(t(Key::ListFieldColor), pal))
                        .child(swatches)
                        .child(field_label(t(Key::ListFieldWindow), pal))
                        .child({
                            let weak_sched = weak.clone();
                            let sched_signal_toggle = sched_signal.clone();
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.))
                                .child(toggle_switch("list-sched", sched_on, pal, move |on, cx| {
                                    sched_signal_toggle.update(cx, |value, cx| {
                                        *value = on;
                                        cx.notify();
                                    });
                                    weak_sched
                                        .update(cx, |this, cx| {
                                            this.list_opts_sched = on;
                                            cx.notify();
                                        })
                                        .ok();
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(pal.text)
                                        .align_end()
                                        .child(t(Key::ListEnableSchedule)),
                                )
                        })
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .mirror()
                                .items_center()
                                .gap(px(6.))
                                .when(!sched_on, |d| d.opacity(0.45))
                                .child(
                                    div()
                                        .flex_1()
                                        .child(Input::new(&start).disabled(!sched_on)),
                                )
                                .child(div().text_sm().text_color(pal.muted).child(t(Key::ScheduleTo)))
                                .child(
                                    div()
                                        .flex_1()
                                        .child(Input::new(&end).disabled(!sched_on)),
                                ),
                        )
                        .child(if sched_on {
                            let weak_shutdown = weak.clone();
                            let shutdown_now = weak
                                .read_with(cx, |this, _| this.list_opts_shutdown)
                                .unwrap_or(shutdown_at_end);
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.))
                                .child(toggle_switch(
                                    "list-shutdown",
                                    shutdown_now,
                                    pal,
                                    move |on, cx| {
                                        weak_shutdown
                                            .update(cx, |this, cx| {
                                                this.list_opts_shutdown = on;
                                                cx.notify();
                                            })
                                            .ok();
                                    },
                                ))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(pal.text)
                                        .align_end()
                                        .child(t(Key::ListForceShutdown)),
                                )
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        })
                        .child(
                            div()
                                .w_full()
                                .text_xs()
                                .text_color(pal.muted)
                                .align_end()
                                .child(tf(
                                    Key::ListDownloadingNow,
                                    &[
                                        &ltr_isolate(&running.to_string()),
                                        &ltr_isolate(&members.to_string()),
                                    ],
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .mirror()
                        .items_center()
                        .gap(px(8.))
                        .py(px(10.))
                        .child(
                            Button::new("list-export")
                                .outline()
                                .label(t(Key::ListSaveList))
                                .tooltip(t(Key::ListSaveListTip))
                                .on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, window, cx: &mut App| {
                                        window.close_dialog(cx);
                                        weak.update(cx, |this, cx| {
                                            this.export_list(name.clone(), window, cx);
                                        })
                                        .ok();
                                    }
                                }),
                        )
                        .child(
                            Button::new("list-import")
                                .outline()
                                .label(t(Key::ListLoadList))
                                .tooltip(t(Key::ListLoadListTip))
                                .on_click({
                                    let weak = weak.clone();
                                    let name = name.clone();
                                    move |_, window, cx: &mut App| {
                                        window.close_dialog(cx);
                                        weak.update(cx, |this, cx| {
                                            this.import_list(name.clone(), window, cx);
                                        })
                                        .ok();
                                    }
                                }),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .pt(px(14.))
                        .border_t_1()
                        .border_color(pal.line)
                        .child(preview.clone()),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("list-opts-cancel")
                                .outline()
                                .label(t(Key::CommonCancel))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("list-opts-ok")
                                .primary()
                                .label(t(Key::CommonSave))
                                .on_click(move |_, window, cx| {
                                    let new_name = crate::text::from_fa_digits(
                                        name_field.read(cx).value().trim(),
                                    );
                                    let sched_start = crate::core::schedule::parse_hm(
                                        &crate::text::from_fa_digits(start_field.read(cx).value().as_ref()),
                                    );
                                    let sched_end = crate::core::schedule::parse_hm(
                                        &crate::text::from_fa_digits(end_field.read(cx).value().as_ref()),
                                    );
                            let sched_on = weak_ok
                                .read_with(cx, |this, _| this.list_opts_sched)
                                .unwrap_or(false);
                            let sched = if sched_on {
                                match (sched_start, sched_end) {
                                    (Some(a), Some(b)) => Some((a, b)),
                                    _ => None,
                                }
                            } else {
                                None
                            };
                            let (stored_cap, color, shutdown) = weak_ok
                                .read_with(cx, |this, _| {
                                    (
                                        this.list_opts_cap,
                                        this.list_opts_color,
                                        this.list_opts_shutdown,
                                    )
                                })
                                .unwrap_or((0, 0, false));
                            // A typed count wins over the presets; an empty box
                            // means the list has no cap.
                            let typed = crate::text::from_fa_digits(cap_field_save.read(cx).value().trim());
                            let cap = if typed.is_empty() {
                                0
                            } else {
                                typed
                                    .parse::<u32>()
                                    .map(|n| n.min(999))
                                    .unwrap_or(stored_cap)
                            };
                            // Shutting down only makes sense with a window.
                            let shutdown = shutdown && sched.is_some();
                                    let from = name_ok.clone();
                                    let target = if new_name.is_empty() {
                                        from.clone()
                                    } else {
                                        new_name
                                    };
                                    weak_ok
                                        .update(cx, |this, cx| {
                                            if target != from {
                                                this.svc.apply(Command::RenameList {
                                                    from: from.clone(),
                                                    to: target.clone(),
                                                });
                                                if this.list_filter.as_deref()
                                                    == Some(from.as_str())
                                                {
                                                    this.list_filter = Some(target.clone());
                                                }
                                            }
                                            this.svc.apply(Command::SetListOptions {
                                                name: target.clone(),
                                                max_parallel: cap,
                                                sched,
                                                shutdown_at_end: shutdown,
                                            });
                                            this.svc.apply(Command::SetListColor {
                                                name: target,
                                                color,
                                            });
                                            cx.notify();
                                        })
                                        .ok();
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    /// Confirm a list deletion; Shift-clicking the row's trash skips this.
    fn confirm_delete_list(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        let weak = cx.entity().downgrade();
        let label = name.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(t(Key::ListDeleteTitle))
                .width(px(430.))
                .child(
                    div()
                        .text_sm()
                        .text_color(pal.text)
                        .child(tf(Key::ListDeleteBody, &[&label])),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("del-list-cancel")
                                .outline()
                                .label(t(Key::CommonCancel))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("del-list-ok")
                                .danger()
                                .label(t(Key::ListDeleteConfirm))
                                .on_click({
                                    let weak = weak.clone();
                                    let target = name.clone();
                                    move |_, window, cx| {
                                        weak.update(cx, |this, cx| {
                                            this.svc.apply(Command::DeleteList(target.clone()));
                                            if this.list_filter.as_deref()
                                                == Some(target.as_str())
                                            {
                                                this.list_filter = None;
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    fn new_list_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(t(Key::ListNamePlaceholder)));
        let weak = cx.entity().downgrade();
        let input = name.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let weak_ok = weak.clone();
            let input_ok = input.clone();
            dialog
                .title(t(Key::ListNewTitle))
                .child(Input::new(&input))
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("new-list-cancel")
                                .outline()
                                .label(t(Key::CommonCancel))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("new-list-ok")
                                .primary()
                                .label(t(Key::CommonCreate))
                                .on_click(move |_, window, cx| {
                                    let name = input_ok.read(cx).value().trim().to_string();
                                    if name.is_empty() {
                                        return;
                                    }
                                    weak_ok
                                        .update(cx, |this, cx| {
                                            this.svc.apply(Command::CreateList(name.clone()));
                                            this.list_filter = Some(name);
                                            this.page = Page::Downloads;
                                            cx.notify();
                                        })
                                        .ok();
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
}

