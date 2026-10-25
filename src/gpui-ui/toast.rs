//! Custom desktop notification shown in its own borderless, top-most window.
use crate::theme::{with_alpha, Palette};
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::Icon;
use std::time::Duration;

pub(crate) struct ToastView {
    pal: Palette,
    title: String,
    message: String,
    icon: gpui_kit::assets::IconName,
    tone: Hsla,
    /// When set, clicking the card opens this link.
    url: Option<String>,
}

impl ToastView {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        pal: Palette,
        title: String,
        message: String,
        icon: gpui_kit::assets::IconName,
        tone: Hsla,
        url: Option<String>,
        secs: u64,
    ) -> Self {
        cx.spawn_in(window, async move |_this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(secs))
                .await;
            cx.update(|window, _| window.remove_window()).ok();
        })
        .detach();
        Self {
            pal,
            title,
            message,
            icon,
            tone,
            url,
        }
    }
}

impl Render for ToastView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = self.pal;
        let title = self.title.clone();
        let message = self.message.clone();
        div()
            .id("toast-root")
            .size_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .rounded(px(14.))
            .bg(pal.card)
            .px(px(16.))
            .py(px(12.))
            .cursor_pointer()
            // The whole message closes itself on click.
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                if let Some(url) = this.url.clone() {
                    crate::app::open_url_in_background(url, cx);
                }
                window.remove_window()
            }))
            .child(
                div()
                    .size(px(32.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(with_alpha(self.tone, 0.15))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(self.icon).size(px(18.)).text_color(self.tone)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(pal.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(message),
                    ),
            )
    }
}
