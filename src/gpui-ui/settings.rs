//! Settings page: appearance, destination folder, connections, speed limit
//! and retry policy. Every control writes through [`Command`], so settings keep
//! living in the same `settings.json` the egui build used.
use crate::app::MainView;
use crate::core::Command;
use crate::format::fmt_speed;
use crate::theme::Palette;
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable as _, IconName, WindowExt as _};
use gpui_kit::base::{Scrollbar, ScrollbarMode};

/// The one-click speeds. Any other value is a custom limit.
pub(crate) const LIMIT_PRESETS: [u64; 5] = [0, 512, 1024, 2048, 5120];

pub(crate) fn page(
    view: &mut MainView,
    _window: &mut Window,
    cx: &mut Context<MainView>,
) -> AnyElement {
    let pal = view.pal;
    let prefs = view.svc.prefs().clone();
    let dir = view.svc.save_dir();

    let mut body = div()
        .id("settings-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap(px(12.))
        .px(px(24.))
        .py(px(20.))
        .child(
            div()
                .text_xl()
                .font_semibold()
                .text_color(pal.text)
                .child("Settings"),
        )
        .child(div().text_sm().text_color(pal.muted).child(
            "Appearance and defaults for new downloads. Changes are saved immediately.",
        ));

    // Appearance ---------------------------------------------------------
    body = body.child(
        card("Appearance", pal)
            .child(
                div()
                    .text_sm()
                    .text_color(pal.muted)
                    .child("Light and dark themes share the same layout and colors."),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        Button::new("theme-light")
                            .when(!prefs.theme_dark, |b| b.primary())
                            .when(prefs.theme_dark, |b| b.outline())
                            .icon(IconName::Sun)
                            .label("Light")
                            .on_click(cx.listener(|this, _, _, cx| this.set_theme(false, cx))),
                    )
                    .child(
                        Button::new("theme-dark")
                            .when(prefs.theme_dark, |b| b.primary())
                            .when(!prefs.theme_dark, |b| b.outline())
                            .icon(IconName::Moon)
                            .label("Dark")
                            .on_click(cx.listener(|this, _, _, cx| this.set_theme(true, cx))),
                    ),
            ),
    );

    // Startup ------------------------------------------------------------
    let autostart = crate::core::sys::autostart_enabled();
    body = body.child(
        card("Startup", pal)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        Switch::new("autostart")
                            .checked(autostart)
                            .on_change(cx.listener(|this, on: &bool, _, cx| {
                                match crate::core::sys::set_autostart(*on) {
                                    Ok(()) => {
                                        this.svc.apply(Command::AutostartConfigured);
                                    }
                                    Err(err) => {
                                        this.notice = Some(crate::app::Notice {
                                            text: err,
                                            error: true,
                                        });
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text)
                            .child("Start DVIDM with Windows"),
                    ),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_xs()
                    .text_color(pal.muted)
                    .child("Adds DVIDM to your sign-in startup apps."),
            ),
    );

    // Notifications ------------------------------------------------------
    let notify = prefs.notify_complete;
    body = body.child(
        card("Notifications", pal)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        Switch::new("notify-complete")
                            .checked(notify)
                            .on_change(cx.listener(|this, on: &bool, _, cx| {
                                this.svc.apply(Command::SetNotifyComplete(*on));
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text)
                            .child("Notify me when a download completes"),
                    ),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_xs()
                    .text_color(pal.muted)
                    .child("A card appears at the bottom-right of the screen."),
            ),
    );

    // Destination --------------------------------------------------------
    body = body.child(
        card("Save to", pal)
            .child(
                div()
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(7.))
                    .bg(pal.bg)
                    .border_1()
                    .border_color(pal.line)
                    .text_sm()
                    .text_color(pal.text)
                    .child(dir.to_string_lossy().into_owned()),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        Button::new("browse")
                            .outline()
                            .label("Browse…")
                            .on_click(cx.listener(|_, _, _, cx| {
                                // The native picker pumps its own modal message
                                // loop; running it inside a GPUI handler re-enters
                                // the app and crashes it, so use a worker thread.
                                cx.spawn(async move |this, cx| {
                                    let picked =
                                        smol::unblock(|| rfd::FileDialog::new().pick_folder())
                                            .await;
                                    if let Some(path) = picked {
                                        this.update(cx, |this, cx| {
                                            this.svc.apply(Command::SetSaveDir(path));
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                })
                                .detach();
                            })),
                    )
                    .child(Button::new("open-dir").outline().label("Open").on_click({
                        let dir = dir.clone();
                        move |_, _, cx: &mut App| {
                            crate::app::open_in_background(dir.clone(), cx);
                        }
                    })),
            ),
    );

    // Temporary files ----------------------------------------------------
    let temp = view.svc.temp_dir();
    body = body.child(
        card("Temporary files", pal)
            .child(div().text_sm().text_color(pal.muted).child(
                "Partial downloads stay here until they finish. Changing the folder moves the parts of paused downloads; pause active ones first.",
            ))
            .child(
                div()
                    .mt(px(8.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(7.))
                    .bg(pal.bg)
                    .border_1()
                    .border_color(pal.line)
                    .text_sm()
                    .text_color(pal.text)
                    .child(temp.to_string_lossy().into_owned()),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .child(
                        Button::new("browse-temp")
                            .outline()
                            .label("Browse…")
                            .on_click(cx.listener(|_, _, _, cx| {
                                // Same off-thread picker as the save folder.
                                cx.spawn(async move |this, cx| {
                                    let picked =
                                        smol::unblock(|| rfd::FileDialog::new().pick_folder())
                                            .await;
                                    if let Some(path) = picked {
                                        this.update(cx, |this, cx| {
                                            let events = this.svc.apply(Command::SetTempDir(path));
                                            this.handle_events(events, cx);
                                            cx.notify();
                                        })
                                        .ok();
                                    }
                                })
                                .detach();
                            })),
                    )
                    .child(Button::new("open-temp").outline().label("Open").on_click({
                        let temp = temp.clone();
                        move |_, _, cx: &mut App| {
                            crate::app::open_in_background(temp.clone(), cx);
                        }
                    })),
            ),
    );

    // Connections --------------------------------------------------------
    let conns = prefs.conns;
    body = body.child(
        card("Connections", pal)
            .child(
                div()
                    .text_sm()
                    .text_color(pal.muted)
                    .child(format!("{conns} parallel parts per file")),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("conns-minus")
                            .outline()
                            .icon(IconName::Minus)
                            .disabled(conns <= 1)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let n = this.svc.prefs().conns.saturating_sub(1).max(1);
                                this.svc.apply(Command::SetConnections(n));
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w(px(36.))
                            .text_center()
                            .text_sm()
                            .text_color(pal.text)
                            .child(conns.to_string()),
                    )
                    .child(
                        Button::new("conns-plus")
                            .outline()
                            .icon(IconName::Plus)
                            .disabled(conns >= 32)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let n = this.svc.prefs().conns.saturating_add(1).min(32);
                                this.svc.apply(Command::SetConnections(n));
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .ml(px(8.))
                            .flex()
                            .flex_row()
                            .gap(px(4.))
                            .children([1u32, 4, 8, 16, 32].into_iter().map(|n| {
                                let weak = cx.entity().downgrade();
                                Button::new(ElementId::Name(format!("conns-{n}").into()))
                                    .outline()
                                    .label(n.to_string())
                                    .on_click(move |_, _, cx: &mut App| {
                                        weak.update(cx, |this, cx| {
                                            this.svc.apply(Command::SetConnections(n));
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                            })),
                    ),
            ),
    );

    // Speed limit --------------------------------------------------------
    let limit = prefs.limit_kbps;
    // Presets and the typed field are two modes, never both at once.
    let custom = view.limit_custom;
    body = body.child(
        card("Speed limit", pal)
            .child(
                div()
                    .text_sm()
                    .text_color(pal.muted)
                    .child("Total download speed shared by every active download."),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap(px(6.))
                            .children(
                                [
                                    (0u64, "Unlimited".to_string()),
                                    (512, format!("512 KB/s")),
                                    (1024, format!("1 MB/s")),
                                    (2048, format!("2 MB/s")),
                                    (5120, format!("5 MB/s")),
                                ]
                                .into_iter()
                                .map(|(kbps, label)| {
                                    let weak = cx.entity().downgrade();
                                    let active = !custom && limit == kbps;
                                    Button::new(ElementId::Name(format!("limit-{kbps}").into()))
                                        .when(active, |b| b.primary())
                                        .when(!active, |b| b.outline())
                                        .label(label)
                                        .on_click(move |_, window, cx: &mut App| {
                                            weak.update(cx, |this, cx| {
                                                this.svc.apply(Command::SetLimitKbps(kbps));
                                                // Choosing a preset leaves custom mode.
                                                this.limit_custom = false;
                                                let text = if kbps == 0 {
                                                    String::new()
                                                } else {
                                                    kbps.to_string()
                                                };
                                                this.limit_input.update(cx, |state, cx| {
                                                    state.set_value(text, window, cx)
                                                });
                                                cx.notify();
                                            })
                                            .ok();
                                        })
                                }),
                            )
                            .child(
                                Button::new("limit-custom")
                                    .when(custom, |b| b.primary())
                                    .when(!custom, |b| b.outline())
                                    .label("Custom")
                                    .tooltip("Type any KB/s value of your own")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.limit_custom = true;
                                        this.limit_input
                                            .update(cx, |state, cx| state.focus(window, cx));
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .when(!custom, |d| d.opacity(0.5))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.muted)
                                    .child("Custom"),
                            )
                            .child(
                                div()
                                    .w(px(104.))
                                    .child(Input::new(&view.limit_input).disabled(!custom)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.muted)
                                    .child("KB/s"),
                            )
                            .child(
                                Button::new("limit-apply")
                                    .outline()
                                    .label("Apply")
                                    .disabled(!custom)
                                    .tooltip("Apply the typed limit (Enter in the field or leaving it also applies)")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let raw = this
                                            .limit_input
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .to_string();
                                        let kbps = if raw.is_empty() {
                                            0
                                        } else {
                                            raw.parse::<u64>().unwrap_or(0)
                                        };
                                        this.svc.apply(Command::SetLimitKbps(kbps));
                                        this.limit_custom = kbps != 0;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.muted)
                            .child(if limit == 0 {
                                "Unlimited — downloads use the full connection speed.".to_string()
                            } else if custom {
                                format!("Custom limit: {}", fmt_speed(limit * 1024))
                            } else {
                                format!("Preset limit: {}", fmt_speed(limit * 1024))
                            }),
                    )
            ),
    );

    // Retry --------------------------------------------------------------
    let retry_parts = prefs.retry_parts;
    let retry_after = prefs.retry_after;
    body = body.child(
        card("Retry", pal)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        Switch::new("retry-parts").checked(retry_parts).on_change(
                            cx.listener(|this, on: &bool, _, cx| {
                                let after = this.svc.prefs().retry_after;
                                this.svc.apply(Command::SetRetry {
                                    parts: *on,
                                    after_secs: after,
                                });
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text)
                            .child("Force re-download failed parts"),
                    ),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("retry-minus")
                            .outline()
                            .icon(IconName::Minus)
                            .disabled(retry_after <= 3)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let after = this.svc.prefs().retry_after.saturating_sub(5).max(3);
                                let parts = this.svc.prefs().retry_parts;
                                this.svc.apply(Command::SetRetry {
                                    parts,
                                    after_secs: after,
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w(px(52.))
                            .text_center()
                            .text_sm()
                            .text_color(pal.text)
                            .child(format!("{retry_after} s")),
                    )
                    .child(
                        Button::new("retry-plus")
                            .outline()
                            .icon(IconName::Plus)
                            .disabled(retry_after >= 300)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let after = this.svc.prefs().retry_after.saturating_add(5).min(300);
                                let parts = this.svc.prefs().retry_parts;
                                this.svc.apply(Command::SetRetry {
                                    parts,
                                    after_secs: after,
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.muted)
                            .child("Wait between attempts"),
                    ),
            ),
    );

    // Interface font -----------------------------------------------------
    let font = prefs.font.clone();
    let font_label = if font.trim().is_empty() {
        "System default".to_string()
    } else {
        font.clone()
    };
    body = body.child(
        card("Interface font", pal)
            .child(
                div()
                    .text_sm()
                    .text_color(pal.muted)
                    .child("Applies to every label, button and value in the app."),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .px(px(10.))
                            .py(px(7.))
                            .rounded(px(7.))
                            .bg(pal.bg)
                            .border_1()
                            .border_color(pal.line)
                            .text_sm()
                            .text_color(pal.text)
                            .child(font_label),
                    )
                    .child(
                        Button::new("font-change")
                            .outline()
                            .label("Change…")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.font_dialog(window, cx);
                            })),
                    ),
            ),
    );

    body.into_any_element()
}

impl MainView {
    /// Font picker for the GPUI build: every family the system reports, applied
    /// to the theme so labels, buttons and values change together.
    pub(crate) fn font_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pal = self.pal;
        let mut fonts = cx.text_system().all_font_names();
        fonts.sort_unstable();
        fonts.dedup();
        let current = self.svc.prefs().font.clone();
        let scroll = ScrollHandle::new();
        let scroll_bar = scroll.clone();
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let mut rows = div()
                .id("font-list")
                .flex()
                .flex_col()
                .gap(px(2.))
                .max_h(px(360.))
                .pr(px(8.))
                .overflow_y_scroll()
                .track_scroll(&scroll);
            let mut choices = vec![String::new()];
            choices.extend(fonts.iter().cloned());
            for name in choices {
                let weak = weak.clone();
                let target = name.clone();
                let active = current.eq_ignore_ascii_case(&name);
                let label = if name.is_empty() {
                    "System default".to_string()
                } else {
                    name.clone()
                };
                rows = rows.child(
                    div()
                        .id(ElementId::Name(
                            format!("font-{}", if name.is_empty() { "default" } else { &name })
                                .into(),
                        ))
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(6.))
                        .when(active, |d| d.bg(crate::theme::with_alpha(pal.accent, 0.16)))
                        .hover(|s| s.bg(pal.card_hover))
                        .on_click(move |_, window, cx: &mut App| {
                            let target = target.clone();
                            weak.update(cx, |this, cx| this.apply_font(&target, cx)).ok();
                            window.close_dialog(cx);
                        })
                        .child(div().text_sm().text_color(pal.text).child(label)),
                );
            }
            dialog
                .title("Interface font")
                .child(
                    div()
                        .relative()
                        .w_full()
                        .child(rows)
                        .child(Scrollbar::vertical(&scroll_bar).mode(ScrollbarMode::Always)),
                )
                .footer(DialogFooter::new().child(
                    Button::new("font-cancel")
                        .outline()
                        .label("Cancel")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ))
        });
    }

    /// Persist and install a font family, or reset to the system default.
    pub(crate) fn apply_font(&mut self, name: &str, cx: &mut Context<Self>) {
        self.svc
            .apply(crate::core::Command::SetFont(name.to_string()));
        crate::apply_font(name, cx);
        cx.notify();
    }
}

fn card(title: &str, pal: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .p(px(16.))
        .rounded(px(12.))
        .bg(pal.card)
        .border_1()
        .border_color(pal.line)
        .child(
            div()
                .mb(px(8.))
                .text_sm()
                .font_semibold()
                .text_color(pal.text)
                .child(title.to_string()),
        )
}
