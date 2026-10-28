//! Settings page: appearance, destination folder, connections, speed limit
//! and retry policy. Every control writes through [`Command`], so settings keep
//! living in the same `settings.json` the egui build used.
use crate::app::MainView;
use crate::core::Command;
use crate::format::fmt_speed;
use crate::i18n::{t, tf, Key};
use crate::rtl::{AlignEnd as _, DirEdge as _, Mirror as _};
use crate::text::ltr_isolate;
use crate::theme::Palette;
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, Textarea};
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
    let ui_lang = prefs.ui_lang.clone();
    let dir = view.svc.save_dir();

    let mut content = div()
        .w_full()
        .max_w(px(1280.))
        .mx_auto()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .text_xl()
                .font_semibold()
                .text_color(pal.text)
                .align_end()
                .child(t(Key::SettingsTitle)),
        )
        .child(div().text_sm().text_color(pal.muted).align_end().child(t(Key::SettingsIntro)));

    // Appearance: theme, language and interface font ---------------------
    let font_label = if prefs.font.trim().is_empty() {
        t(Key::SettingsFontSystem).to_string()
    } else {
        prefs.font.clone()
    };
    let appearance = card(t(Key::SettingsAppearance), pal)
        .child(
            div()
                .w_full()
                .text_sm()
                .text_color(pal.muted)
                .child(t(Key::SettingsThemeHint)),
        )
        .child(
            div()
                .mt(px(10.))
                .flex()
                .flex_row().mirror()
                .gap(px(8.))
                .child(
                    Button::new("theme-light")
                        .when(!prefs.theme_dark, |b| b.primary())
                        .when(prefs.theme_dark, |b| b.outline())
                        .icon(IconName::Sun)
                        .label(t(Key::SettingsThemeLight))
                        .on_click(cx.listener(|this, _, _, cx| this.set_theme(false, cx))),
                )
                .child(
                    Button::new("theme-dark")
                        .when(prefs.theme_dark, |b| b.primary())
                        .when(!prefs.theme_dark, |b| b.outline())
                        .icon(IconName::Moon)
                        .label(t(Key::SettingsThemeDark))
                        .on_click(cx.listener(|this, _, _, cx| this.set_theme(true, cx))),
                ),
        )
        .child(divider(pal))
        .child(
            div()
                .flex()
                .flex_row()
                .mirror()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm()
                        .text_color(pal.text)
                        .child(t(Key::SettingsLanguage)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .mirror()
                        .gap(px(6.))
                        .child(ui_lang_button(
                            "lang-system",
                            t(Key::SettingsLangSystem),
                            "system",
                            &ui_lang,
                            pal,
                            cx,
                        ))
                        .child(ui_lang_button(
                            "lang-en",
                            t(Key::SettingsLangEnglish),
                            "en",
                            &ui_lang,
                            pal,
                            cx,
                        ))
                        .child(ui_lang_button(
                            "lang-fa",
                            t(Key::SettingsLangPersian),
                            "fa",
                            &ui_lang,
                            pal,
                            cx,
                        )),
                ),
        )
        .child(divider(pal))
        .child(
            div()
                .flex()
                .flex_row()
                .mirror()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm()
                        .text_color(pal.text)
                        .child(t(Key::SettingsFont)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(7.))
                        .bg(pal.bg)
                        .border_1()
                        .border_color(pal.line)
                        .text_sm()
                        .text_color(pal.text)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(font_label),
                )
                .child(
                    Button::new("font-change")
                        .outline()
                        .label(t(Key::SettingsFontChange))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.font_dialog(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .mt(px(6.))
                .w_full()
                .text_xs()
                .text_color(pal.muted)
                .child(t(Key::SettingsFontHint)),
        );

    // Behavior: startup, notifications and list focus --------------------
    let autostart = crate::core::sys::autostart_enabled();
    let notify = prefs.notify_complete;
    let focus_new = prefs.focus_new_download;
    let behavior = card(t(Key::SettingsBehavior), pal)
        .child(toggle_row(
            Switch::new("autostart")
                .checked(autostart)
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    match crate::core::sys::set_autostart(*on) {
                        Ok(()) => {
                            this.svc.apply(Command::AutostartConfigured);
                        }
                        Err(err) => {
                            this.show_notice(crate::i18n::error(&err), true, cx);
                        }
                    }
                    cx.notify();
                }))
                .into_any_element(),
            t(Key::SettingsAutostart),
            t(Key::SettingsAutostartHint),
            pal,
        ))
        .child(divider(pal))
        .child(toggle_row(
            Switch::new("notify-complete")
                .checked(notify)
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    this.svc.apply(Command::SetNotifyComplete(*on));
                    cx.notify();
                }))
                .into_any_element(),
            t(Key::SettingsNotifyComplete),
            t(Key::SettingsNotifyHint),
            pal,
        ))
        .child(divider(pal))
        .child(toggle_row(
            Switch::new("focus-new")
                .checked(focus_new)
                .on_change(cx.listener(|this, on: &bool, _, cx| {
                    this.svc.apply(Command::SetFocusNewDownload(*on));
                    cx.notify();
                }))
                .into_any_element(),
            t(Key::SettingsFocusNew),
            t(Key::SettingsFocusHint),
            pal,
        ));

    // Browser shortcut ----------------------------------------------------
    let modifier = prefs.browser_modifier.clone();
    let shortcut_label = match modifier.as_str() {
        "ctrl" => t(Key::ShortcutCtrlClick),
        "alt" => t(Key::ShortcutAltClick),
        "shift" => t(Key::ShortcutShiftClick),
        "insert" => t(Key::ShortcutInsertClick),
        "ctrl_alt" => t(Key::ShortcutCtrlAltClick),
        "ctrl_shift" => t(Key::ShortcutCtrlShiftClick),
        "alt_shift" => t(Key::ShortcutAltShiftClick),
        _ => t(Key::ShortcutBothClick),
    };
    let browser = card(t(Key::SettingsBrowserShortcut), pal)
        .child(div().w_full().text_sm().text_color(pal.muted).child(tf(
            Key::SettingsBrowserHint,
            &[shortcut_label.as_str()],
        )))
        .child(
            div()
                .mt(px(8.))
                .flex()
                .flex_row().mirror()
                .flex_wrap()
                .gap(px(6.))
                .child(browser_modifier_button(
                    "ctrl-click",
                    t(Key::ModCtrl),
                    "ctrl",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "alt-click",
                    t(Key::ModAlt),
                    "alt",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "shift-click",
                    t(Key::ModShift),
                    "shift",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "insert-click",
                    t(Key::ModInsert),
                    "insert",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "ctrl-alt-click",
                    t(Key::ModCtrlAlt),
                    "ctrl_alt",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "ctrl-shift-click",
                    t(Key::ModCtrlShift),
                    "ctrl_shift",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "alt-shift-click",
                    t(Key::ModAltShift),
                    "alt_shift",
                    &modifier,
                    pal,
                    cx,
                ))
                .child(browser_modifier_button(
                    "both-click",
                    t(Key::ModBoth),
                    "both",
                    &modifier,
                    pal,
                    cx,
                )),
        )
        .child(divider(pal))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .w_full()
                        .child(Textarea::new(&view.formats_input).w_full().h(px(96.))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row().mirror()
                        .gap(px(6.))
                        .child(
                            Button::new("formats-apply")
                                .outline()
                                .label(t(Key::CommonApply))
                                .tooltip(t(Key::SettingsFormatsApplyTip))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.apply_direct_formats(cx);
                                })),
                        )
                        .child(
                            Button::new("formats-reset")
                                .outline()
                                .label(t(Key::CommonReset))
                                .tooltip(t(Key::SettingsFormatsResetTip))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.svc.apply(Command::SetDirectFormats(String::new()));
                                    this.sync_formats_input(window, cx);
                                    cx.notify();
                                })),
                        ),
                ),
        );

    // Proxy / SOCKS ------------------------------------------------------
    let proxy_mode = prefs.proxy_mode.clone();
    let proxy_kind = prefs.proxy_kind.clone();
    let manual_proxy = proxy_mode == "manual";
    let system_note = if proxy_mode == "system" {
        Some(match crate::core::sys::system_proxy() {
            Some(url) => (tf(Key::SettingsWindowsProxy, &[&ltr_isolate(&url)]), pal.muted),
            None => (
                t(Key::SettingsNoWindowsProxy).to_string(),
                pal.warn,
            ),
        })
    } else {
        None
    };
    let (ph, ps, pf) = (prefs.proxy_http, prefs.proxy_https, prefs.proxy_ftp);
    let proxy = card(t(Key::SettingsProxy), pal)
        .child(
            div()
                .w_full()
                .text_sm()
                .text_color(pal.muted)
                .child(t(Key::SettingsProxyHint)),
        )
        .child(
            div()
                .mt(px(10.))
                .flex()
                .flex_row().mirror()
                .items_center()
                .justify_between()
                .gap(px(10.))
                .child(
                    div()
                        .flex()
                        .flex_row().mirror()
                        .gap(px(6.))
                        .child(proxy_mode_button("proxy-none", t(Key::SettingsProxyNone), "none", &proxy_mode, cx))
                        .child(proxy_mode_button("proxy-system", t(Key::SettingsProxySystem), "system", &proxy_mode, cx))
                        .child(proxy_mode_button("proxy-manual", t(Key::SettingsProxyManual), "manual", &proxy_mode, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row().mirror()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_xs()
                                .text_color(pal.muted)
                                .child(t(Key::SettingsProxyType)),
                        )
                        .child(proxy_kind_button("proxy-kind-http", t(Key::SettingsProxyHttp), "http", &proxy_kind, cx))
                        .child(proxy_kind_button(
                            "proxy-kind-socks",
                            t(Key::SettingsProxySocks5),
                            "socks5",
                            &proxy_kind,
                            cx,
                        )),
                ),
        )
        .when(system_note.is_some(), |card| {
            let (text, color) = system_note.clone().unwrap();
            card.child(
                div()
                    .mt(px(6.))
                    .text_xs()
                    .text_color(color)
                    .child(text),
            )
        })
        .child(
            div()
                .mt(px(10.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .text_xs()
                        .text_color(pal.muted)
                        .child(t(Key::SettingsProxyAddressHint)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row().mirror()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&view.proxy_host_input).disabled(!manual_proxy)),
                        )
                        .child(
                            div()
                                .w(px(90.))
                                .child(Input::new(&view.proxy_port_input).disabled(!manual_proxy)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row().mirror()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&view.proxy_user_input).disabled(!manual_proxy)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&view.proxy_pass_input).disabled(!manual_proxy)),
                        ),
                ),
        )
        .child(divider(pal))
        .child(
            div()
                .flex()
                .flex_row().mirror()
                .flex_wrap()
                .items_center()
                .gap(px(14.))
                .child(
                    Checkbox::new("proxy-http")
                        .checked(ph)
                        .label(t(Key::SettingsProxyProtoHttp))
                        .on_click(cx.listener(move |this, on, _, cx| {
                            this.svc.apply(Command::SetProxyProtocols {
                                http: *on,
                                https: ps,
                                ftp: pf,
                            });
                            cx.notify();
                        })),
                )
                .child(
                    Checkbox::new("proxy-https")
                        .checked(ps)
                        .label(t(Key::SettingsProxyProtoHttps))
                        .on_click(cx.listener(move |this, on, _, cx| {
                            this.svc.apply(Command::SetProxyProtocols {
                                http: ph,
                                https: *on,
                                ftp: pf,
                            });
                            cx.notify();
                        })),
                )
                .child(
                    Checkbox::new("proxy-ftp")
                        .checked(pf)
                        .label(t(Key::SettingsProxyProtoFtp))
                        .on_click(cx.listener(move |this, on, _, cx| {
                            this.svc.apply(Command::SetProxyProtocols {
                                http: ph,
                                https: ps,
                                ftp: *on,
                            });
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("proxy-apply")
                        .outline()
                        .label(t(Key::CommonApply))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let host = this.proxy_host_input.read(cx).value().to_string();
                            let port = this
                                .proxy_port_input
                                .read(cx)
                                .value()
                                .trim()
                                .to_string();
                            let port = crate::text::from_fa_digits(&port)
                                .parse::<u16>()
                                .unwrap_or(8080)
                                .max(1);
                            let user = this.proxy_user_input.read(cx).value().to_string();
                            let pass = this.proxy_pass_input.read(cx).value().to_string();
                            this.svc.apply(Command::SetProxyFields {
                                host,
                                port,
                                user,
                                pass,
                            });
                            cx.notify();
                        })),
                ),
        )
        .child(
            Checkbox::new("proxy-browser")
                .checked(prefs.proxy_from_browser)
                .label(t(Key::SettingsProxyFromBrowser))
                .on_click(cx.listener(|this, on, _, cx| {
                    this.svc.apply(Command::SetProxyFromBrowser(*on));
                    cx.notify();
                })),
        );

    // Destination --------------------------------------------------------
    let save = card(t(Key::SettingsSaveTo), pal)
            .child(
                div()
                    .w_full()
                    .text_sm()
                    .text_color(pal.muted)
                    .child(t(Key::SettingsSaveHint)),
            )
            .child(
                div()
                    .mt(px(8.))
                    .w_full()
                    .min_w_0()
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(7.))
                    .bg(pal.bg)
                    .border_1()
                    .border_color(pal.line)
                    .text_sm()
                    .text_color(pal.text)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(dir.to_string_lossy().into_owned()),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row().mirror()
                    .gap(px(8.))
                    .child(
                        Button::new("browse")
                            .outline()
                            .label(t(Key::CommonBrowse))
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
                    .child(Button::new("open-dir").outline().label(t(Key::CommonOpen)).on_click({
                        let dir = dir.clone();
                        move |_, _, cx: &mut App| {
                            crate::app::open_in_background(dir.clone(), cx);
                        }
                    })));


    // Temporary files ----------------------------------------------------
    let temp = view.svc.temp_dir();
    let temp_card = card(t(Key::SettingsTempFiles), pal)
            .child(div().w_full().text_sm().text_color(pal.muted).child(
                t(Key::SettingsTempHint),
            ))
            .child(
                div()
                    .mt(px(8.))
                    .w_full()
                    .min_w_0()
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(7.))
                    .bg(pal.bg)
                    .border_1()
                    .border_color(pal.line)
                    .text_sm()
                    .text_color(pal.text)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(temp.to_string_lossy().into_owned()),
            )
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_row().mirror()
                    .gap(px(8.))
                    .child(
                        Button::new("browse-temp")
                            .outline()
                            .label(t(Key::CommonBrowse))
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
                    .child(Button::new("open-temp").outline().label(t(Key::CommonOpen)).on_click({
                        let temp = temp.clone();
                        move |_, _, cx: &mut App| {
                            crate::app::open_in_background(temp.clone(), cx);
                        }
                    })));

    // Connections and retry ---------------------------------------------
    let conns = prefs.conns;
    let retry_parts = prefs.retry_parts;
    let retry_after = prefs.retry_after;
    let performance = card(t(Key::SettingsConnections), pal)
        .child(
            div()
                .text_sm()
                .text_color(pal.muted)
                .child(tf(Key::SettingsConnsLabel, &[&ltr_isolate(&conns.to_string())])),
        )
        .child(
            div()
                .mt(px(10.))
                .flex()
                .flex_row().mirror()
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
                        .margin_start(px(8.))
                        .flex()
                        .flex_row().mirror()
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
        )
        .child(divider(pal))
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(pal.text)
                .child(t(Key::SettingsRetry)),
        )
        .child(
            div()
                .mt(px(8.))
                .flex()
                .flex_row().mirror()
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
                        .child(t(Key::SettingsRetryParts)),
                ),
        )
        .child(
            div()
                .mt(px(8.))
                .flex()
                .flex_row().mirror()
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
                        .child(tf(Key::SettingsRetryWaitValue, &[&ltr_isolate(&retry_after.to_string())])),
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
                        .child(t(Key::SettingsRetryWait)),
                ),
        );

    // Speed limit --------------------------------------------------------
    let limit = prefs.limit_kbps;
    // Presets and the typed field are two modes, never both at once.
    let custom = view.limit_custom;
    let speed = card(t(Key::SettingsSpeedLimit), pal)
            .child(
                div()
                    .w_full()
                    .text_sm()
                    .text_color(pal.muted)
                    .child(t(Key::SettingsSpeedHint)),
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
                            .flex_row().mirror()
                            .flex_wrap()
                            .items_center()
                            .gap(px(6.))
                            .children(
                                [
                                    (0u64, t(Key::SettingsUnlimited).to_string()),
                                    (512, t(Key::SettingsLimit512).to_string()),
                                    (1024, t(Key::SettingsLimit1m).to_string()),
                                    (2048, t(Key::SettingsLimit2m).to_string()),
                                    (5120, t(Key::SettingsLimit5m).to_string()),
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
                                                    crate::text::fa_input(&kbps.to_string())
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
                                    .label(t(Key::CommonCustom))
                                    .tooltip(t(Key::SettingsCustomLimitTip))
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
                            .flex_row().mirror()
                            .items_center()
                            .gap(px(6.))
                            .when(!custom, |d| d.opacity(0.5))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.muted)
                                    .child(t(Key::CommonCustom)),
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
                                    .child(t(Key::UnitsKbps)),
                            )
                            .child(
                                Button::new("limit-apply")
                                    .outline()
                                    .label(t(Key::CommonApply))
                                    .disabled(!custom)
                                    .tooltip(t(Key::SettingsApplyLimitTip))
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
                                            crate::text::from_fa_digits(&raw).parse::<u64>().unwrap_or(0)
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
                                t(Key::SettingsUnlimitedStatus).to_string()
                            } else if custom {
                                tf(Key::SettingsCustomLimitStatus, &[&ltr_isolate(&fmt_speed(limit * 1024))])
                            } else {
                                tf(Key::SettingsPresetLimitStatus, &[&ltr_isolate(&fmt_speed(limit * 1024))])
                            }),
                    )
            );

    // Long format list and the settings path sit below the pair so the
    // browser and proxy cards keep the same height. The list splits into two
    // even lines instead of one line per eight entries.
    let formats = crate::core::prefs::DEFAULT_DIRECT_FORMATS;
    let formats_per_line = formats.len().div_ceil(2);
    let browser_note = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .align_end()
        .child(
            div()
                .w_full()
                .text_xs()
                .text_color(pal.muted)
                .child(tf(
                    Key::SettingsFormatsDefaults,
                    &[&formats
                        .chunks(formats_per_line)
                        .map(|c| c.join(", "))
                        .collect::<Vec<_>>()
                        .join(",\n")],
                )),
        )
        .child(
            div()
                .w_full()
                .text_xs()
                .text_color(pal.muted)
                .child(tf(
                    Key::SettingsPrefsPath,
                    &[&crate::core::prefs::prefs_path().display().to_string()],
                )),
        );

    let content = content
        .child(group_label(t(Key::SettingsGroupGeneral), pal))
        .child(grid(vec![appearance, behavior]))
        .child(group_label(t(Key::SettingsGroupDownloads), pal))
        .child(grid(vec![save, temp_card]))
        .child(grid(vec![performance, speed]))
        .child(group_label(t(Key::SettingsGroupBrowserNetwork), pal))
        .child(grid(vec![browser, proxy]))
        .child(browser_note);

    div()
        .id("settings-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_y_scroll()
        .px(px(24.))
        .py(px(20.))
        .child(content)
        .into_any_element()

}

impl MainView {
    /// Apply button: write the list and always report the result.
    pub(crate) fn apply_direct_formats(&mut self, cx: &mut Context<Self>) {
        self.save_direct_formats(true, true, cx);
    }

    /// Leaving the settings page: only write (and report) when the list
    /// actually changed. Quitting passes `announce = false`.
    pub(crate) fn flush_direct_formats(&mut self, announce: bool, cx: &mut Context<Self>) {
        self.save_direct_formats(false, announce, cx);
    }

    fn save_direct_formats(
        &mut self,
        force: bool,
        announce: bool,
        cx: &mut Context<Self>,
    ) {
        let raw = self.formats_input.read(cx).value().to_string();
        let parsed = crate::core::prefs::collect_direct_formats(&raw);
        let ignored = crate::core::prefs::invalid_direct_formats(&raw);
        if parsed.is_empty() {
            if force {
                let message = if ignored.is_empty() {
                    t(Key::SettingsFormatsEmpty).to_string()
                } else {
                    tf(Key::SettingsFormatsUnsupported, &[&ignored.join(", ")])
                };
                self.show_notice(message, true, cx);
            }
            return;
        }
        let changed = parsed != self.svc.prefs().direct_formats;
        if !force && !changed {
            return;
        }
        self.svc.apply(crate::core::Command::SetDirectFormats(
            crate::core::prefs::format_direct_formats(&parsed),
        ));
        let path = crate::core::prefs::prefs_path();
        let saved = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|json| {
                json.get("direct_formats")
                    .and_then(|value| value.as_array())
                    .map(|list| {
                        list.iter()
                            .filter_map(|value| value.as_str().map(str::to_string))
                            .collect::<Vec<_>>()
                    })
            })
            .map(|actual| actual == parsed)
            .unwrap_or(false);
        crate::core::log::write(format!(
            "saved {} file types to {path:?}: {}",
            parsed.len(),
            if saved { "ok" } else { "failed" }
        ));
        if saved {
            if announce {
                let mut message = tf(
                    Key::SettingsFormatsSaved,
                    &[&ltr_isolate(&parsed.len().to_string())],
                );
                if !ignored.is_empty() {
                    message.push_str(&tf(Key::SettingsFormatsIgnored, &[&ignored.join(", ")]));
                }
                self.show_notice(message, false, cx);
            }
        } else if force || announce {
            self.show_notice(
                tf(Key::SettingsFormatsWriteFailed, &[&path.display().to_string()]),
                true,
                cx,
            );
        }
    }

    pub(crate) fn sync_formats_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = crate::core::prefs::format_direct_formats(&self.svc.prefs().direct_formats);
        if self.formats_input.read(cx).value().as_ref() == text {
            return;
        }
        self.formats_input
            .update(cx, |state, cx| state.set_value(text, window, cx));
    }

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
                    t(Key::SettingsFontSystem).to_string()
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
                        .flex_row().mirror()
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
                .title(t(Key::SettingsFont))
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
                        .label(t(Key::CommonCancel))
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

fn card(title: impl AsRef<str>, pal: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .p(px(16.))
        .rounded(px(12.))
        .bg(pal.card)
        .border_1()
        .border_color(pal.line)
        .align_end()
        .child(
            div()
                .mb(px(8.))
                .text_sm()
                .font_semibold()
                .text_color(pal.text)
                .align_end()
                .child(title.as_ref().to_string()),
        )
}

/// Two cards per row on wide windows; they share the line evenly and wrap
/// to a single column on narrow ones.
fn grid(cards: Vec<Div>) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row().mirror()
        .flex_wrap()
        .items_stretch()
        .gap(px(14.))
        .children(
            cards
                .into_iter()
                .map(|card| card.flex_basis(px(0.)).flex_grow(1.).min_w(px(400.))),
        )
}

/// Small muted heading that introduces a group of cards.
fn group_label(title: String, pal: Palette) -> Div {
    div()
        .mt(px(6.))
        .text_xs()
        .font_semibold()
        .text_color(pal.muted)
        .align_end()
        .child(title)
}

fn divider(pal: Palette) -> Div {
    div().w_full().h(px(1.)).my(px(12.)).bg(pal.line)
}

/// One setting row: switch on the start edge, label and hint beside it.
fn toggle_row(
    switch: AnyElement,
    label: String,
    hint: String,
    pal: Palette,
) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row().mirror()
        .items_start()
        .gap(px(10.))
        .child(switch)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_sm().text_color(pal.text).child(label))
                .child(div().text_xs().text_color(pal.muted).child(hint)),
        )
}

fn browser_modifier_button(
    id: &'static str,
    label: impl Into<String>,
    value: &'static str,
    selected: &str,
    _pal: Palette,
    cx: &mut Context<MainView>,
) -> AnyElement {
    Button::new(id)
        .when(selected == value, |b| b.primary())
        .when(selected != value, |b| b.outline())
        .label(label.into())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.svc.apply(Command::SetBrowserModifier(value.into()));
            cx.notify();
        }))
        .into_any_element()
}

fn ui_lang_button(
    id: &'static str,
    label: impl Into<String>,
    value: &'static str,
    selected: &str,
    _pal: Palette,
    cx: &mut Context<MainView>,
) -> AnyElement {
    Button::new(id)
        .when(selected == value, |b| b.primary())
        .when(selected != value, |b| b.outline())
        .label(label.into())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.svc.apply(Command::SetUiLang(value.into()));
            let lang = crate::core::prefs::normalize_ui_lang(value);
            crate::i18n::apply(&lang);
            cx.notify();
        }))
        .into_any_element()
}

fn proxy_mode_button(
    id: &'static str,
    label: impl Into<String>,
    value: &'static str,
    selected: &str,
    cx: &mut Context<MainView>,
) -> AnyElement {
    Button::new(id)
        .when(selected == value, |b| b.primary())
        .when(selected != value, |b| b.outline())
        .label(label.into())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.svc.apply(Command::SetProxyMode(value.into()));
            cx.notify();
        }))
        .into_any_element()
}

fn proxy_kind_button(
    id: &'static str,
    label: impl Into<String>,
    value: &'static str,
    selected: &str,
    cx: &mut Context<MainView>,
) -> AnyElement {
    Button::new(id)
        .when(selected == value, |b| b.primary())
        .when(selected != value, |b| b.outline())
        .label(label.into())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.svc.apply(Command::SetProxyKind(value.into()));
            cx.notify();
        }))
        .into_any_element()
}
