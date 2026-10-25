//! Custom desktop notification shown in its own borderless, top-most window.
use crate::app::MainView;
use crate::theme::{with_alpha, Palette};
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::Icon;
use std::time::Duration;

/// Clicking the notification adds the installer to DVIDM's own downloads.
#[derive(Clone)]
pub(crate) struct UpdateAction {
    pub(crate) app: WeakEntity<MainView>,
    pub(crate) version: String,
    pub(crate) url: String,
}

pub(crate) struct ToastView {
    pal: Palette,
    title: String,
    message: String,
    icon: gpui_kit::assets::IconName,
    tone: Hsla,
    /// When set, clicking the card starts the installer download.
    action: Option<UpdateAction>,
    /// Fade progress, 0 = invisible, 1 = fully shown.
    alpha: f32,
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
        action: Option<UpdateAction>,
        secs: u64,
    ) -> Self {
        let hold = Duration::from_millis(secs.saturating_mul(1000).saturating_sub(400).max(200));
        cx.spawn_in(window, async move |this, cx| {
            for step in 1..=8u32 {
                cx.background_executor()
                    .timer(Duration::from_millis(18))
                    .await;
                let _ = cx.update(|_, cx| {
                    this.update(cx, |this, cx| {
                        this.alpha = step as f32 / 8.0;
                        cx.notify();
                    })
                    .ok();
                });
            }
            cx.background_executor().timer(hold).await;
            for step in (0..=10u32).rev() {
                cx.background_executor()
                    .timer(Duration::from_millis(22))
                    .await;
                let _ = cx.update(|_, cx| {
                    this.update(cx, |this, cx| {
                        this.alpha = step as f32 / 10.0;
                        cx.notify();
                    })
                    .ok();
                });
            }
            cx.update(|window, _| window.remove_window()).ok();
        })
        .detach();
        Self {
            pal,
            title,
            message,
            icon,
            tone,
            action,
            alpha: 0.0,
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
                if let Some(UpdateAction { app, version, url }) = this.action.take() {
                    app.update(cx, |this, cx| this.add_update_download(version, url, cx))
                        .ok();
                }
                window.remove_window()
            }))
            .opacity(self.alpha)
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
