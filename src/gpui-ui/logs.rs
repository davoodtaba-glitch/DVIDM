//! Temporary debug-log window. It lives in its own native window so it does
//! not cover the download manager.
use crate::i18n::{t, tf, Key};
use crate::rtl::Mirror as _;
use crate::theme::Palette;
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use std::path::PathBuf;

pub(crate) struct LogView {
    pal: Palette,
    text: Entity<TextareaState>,
    path: PathBuf,
}

impl LogView {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>, pal: Palette) -> Self {
        let text = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(crate::core::log::snapshot().join("\n"))
        });
        Self {
            pal,
            text,
            path: crate::core::log::path(),
        }
    }
}

impl Render for LogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = self.pal;
        let path = self.path.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(pal.bg)
            .text_color(pal.text)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px(px(16.))
                    .pt(px(14.))
                    .pb(px(10.))
                    .child(
                        div()
                            .text_lg()
                            .font_semibold()
                            .text_color(pal.text)
                            .child(t(Key::LogsTitle)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.muted)
                            .child(tf(Key::LogsHint, &[&path.to_string_lossy()])),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(px(16.))
                    .pb(px(10.))
                    .child(Textarea::new(&self.text).readonly(true).h_full()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .mirror()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(16.))
                    .pb(px(14.))
                    .child(
                        Button::new("log-refresh")
                            .outline()
                            .label(t(Key::LogsRefresh))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let text = crate::core::log::snapshot().join("\n");
                                this.text.update(cx, |state, cx| {
                                    state.set_value(text, window, cx)
                                });
                            })),
                    )
                    .child(
                        Button::new("log-copy")
                            .outline()
                            .label(t(Key::LogsCopyAll))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let text = this.text.read(cx).value().to_string();
                                let _ = crate::clipboard::write_text(&text);
                            })),
                    )
                    .child(
                        Button::new("log-open")
                            .outline()
                            .label(t(Key::LogsOpenFile))
                            .on_click(move |_, _, cx: &mut App| {
                                crate::app::open_in_background(path.clone(), cx);
                            }),
                    )
                    .child(
                        Button::new("log-close")
                            .primary()
                            .label(t(Key::CommonClose))
                            .on_click(|_, window, _cx| window.remove_window()),
                    ),
            )
    }
}
