//! Custom desktop notification shown in its own borderless, top-most window.
use crate::theme::{with_alpha, Palette};
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::Icon;
use std::time::Duration;

pub(crate) struct ToastView {
    pal: Palette,
    name: String,
}

impl ToastView {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        pal: Palette,
        name: String,
    ) -> Self {
        cx.spawn_in(window, async move |_this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(5))
                .await;
            cx.update(|window, _| window.remove_window()).ok();
        })
        .detach();
        Self { pal, name }
    }
}

impl Render for ToastView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = self.pal;
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
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, window, _cx| {
                window.remove_window()
            }))
            .child(
                div()
                    .size(px(32.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(with_alpha(pal.ok, 0.15))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(gpui_kit::assets::IconName::CircleCheck)
                            .size(px(18.))
                            .text_color(pal.ok),
                    ),
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
                            .child("Download complete"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(self.name.clone()),
                    ),
            )
    }
}
