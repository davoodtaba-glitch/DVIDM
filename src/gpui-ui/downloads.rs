//! The download table: virtualized rows, status badges, column sorting and the
//! segmented progress bar that shows each connection's own slice.
use crate::app::{MainView, SortCol};
use crate::core::Command;
use crate::engine::{Category, Snapshot, Status};
use crate::format::{eta_text, fmt_speed, size_text};
use crate::i18n::{self, t, tf, Key};
use crate::rtl::{AlignEnd as _, DirEdge as _, Mirror as _};
use crate::text::{self, ltr_isolate};
use crate::theme::{with_alpha, Palette};
use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::base::{Scrollbar, ScrollbarMode};
use std::collections::HashSet;

/// Row height including the gap below it; uniform_list needs one height.
const ROW_H: f32 = 70.0;
/// Space kept clear under every card, so rows read as separate cards.
const ROW_GAP: f32 = 10.0;

// Column widths. The file column takes whatever is left.
const NUM_W: f32 = 34.0;
const STATUS_W: f32 = 108.0;
const LISTS_W: f32 = 36.0;
const SIZE_W: f32 = 116.0;
const PROGRESS_W: f32 = 66.0;
const SPEED_W: f32 = 74.0;
const ETA_W: f32 = 56.0;
const PARTS_W: f32 = 36.0;
const LAST_W: f32 = 108.0;
const ACTIONS_W: f32 = 156.0;

/// Optional columns, in table order. Number and file name are always shown.
pub(crate) const COLUMN_KEYS: [&str; 9] = [
    "status", "lists", "size", "progress", "speed", "eta", "parts", "last", "actions",
];

pub(crate) fn column_label(key: &str) -> Key {
    match key {
        "status" => Key::ColStatus,
        "lists" => Key::ColLists,
        "size" => Key::ColSize,
        "progress" => Key::ColProgress,
        "speed" => Key::ColSpeed,
        "eta" => Key::ColEta,
        "parts" => Key::ColParts,
        "last" => Key::ColLastDate,
        _ => Key::ColActions,
    }
}

fn column_default_width(key: &str) -> u32 {
    match key {
        "status" => STATUS_W as u32,
        "lists" => LISTS_W as u32,
        "size" => SIZE_W as u32,
        "progress" => PROGRESS_W as u32,
        "speed" => SPEED_W as u32,
        "eta" => ETA_W as u32,
        "parts" => PARTS_W as u32,
        "last" => LAST_W as u32,
        _ => ACTIONS_W as u32,
    }
}

fn column_min_width(key: &str) -> u32 {
    match key {
        "lists" | "parts" => 26,
        "actions" => 116,
        _ => 44,
    }
}

fn column_width(view: &MainView, key: &str) -> f32 {
    if let Some(resize) = view.col_resize.as_ref() {
        if resize.key == key {
            return resize.current as f32;
        }
    }
    view.svc
        .prefs()
        .column_widths
        .get(key)
        .copied()
        .unwrap_or_else(|| column_default_width(key)) as f32
}

fn column_shown(view: &MainView, key: &str) -> bool {
    !view.svc.prefs().column_hidden.iter().any(|k| k == key)
}

/// The visible optional columns and their current widths, captured once per
/// table render so the header and every row agree.
#[derive(Clone, Copy)]
pub(crate) struct Cols {
    pub(crate) status: Option<f32>,
    pub(crate) lists: Option<f32>,
    pub(crate) size: Option<f32>,
    pub(crate) progress: Option<f32>,
    pub(crate) speed: Option<f32>,
    pub(crate) eta: Option<f32>,
    pub(crate) parts: Option<f32>,
    pub(crate) last: Option<f32>,
    pub(crate) actions: Option<f32>,
}

impl Cols {
    pub(crate) fn from(view: &MainView) -> Self {
        let mut cols = Self {
            status: None,
            lists: None,
            size: None,
            progress: None,
            speed: None,
            eta: None,
            parts: None,
            last: None,
            actions: None,
        };
        for key in COLUMN_KEYS {
            if !column_shown(view, key) {
                continue;
            }
            let width = Some(column_width(view, key));
            match key {
                "status" => cols.status = width,
                "lists" => cols.lists = width,
                "size" => cols.size = width,
                "progress" => cols.progress = width,
                "speed" => cols.speed = width,
                "eta" => cols.eta = width,
                "parts" => cols.parts = width,
                "last" => cols.last = width,
                _ => cols.actions = width,
            }
        }
        cols
    }
}

/// Finish time, falling back to the added time for pre-0.5.21 completed rows.
fn last_started_ms(s: &Snapshot) -> u64 {
    // Not started yet: the add time is the best "last date" we have.
    s.started.or(s.finished).unwrap_or(s.added)
}

fn last_download_full(s: &Snapshot) -> String {
    let ms = last_started_ms(s);
    if ms == 0 {
        "—".to_string()
    } else {
        crate::core::sys::format_epoch_ms(ms)
    }
}

fn last_download_text(s: &Snapshot) -> String {
    let ms = last_started_ms(s);
    if ms == 0 {
        "—".to_string()
    } else {
        let full = crate::core::sys::format_epoch_ms(ms);
        full.get(5..16).unwrap_or(&full).to_string()
    }
}

/// Update one row's selection and report whether it changed.
fn set_selected(selected: &mut HashSet<String>, id: &str, on: bool) -> bool {
    if on {
        selected.insert(id.to_string())
    } else {
        selected.remove(id)
    }
}

fn progress_ratio(s: &Snapshot) -> f64 {
    s.total
        .filter(|t| *t > 0)
        .map(|t| s.received as f64 / t as f64)
        .unwrap_or(0.0)
}

/// Sort order for the Status column: running first, finished last.
fn status_rank(status: Status) -> u8 {
    match status {
        Status::Connecting => 0,
        Status::Downloading => 1,
        Status::Paused => 2,
        Status::Failed => 3,
        Status::Done => 4,
    }
}

fn eta_seconds(s: &Snapshot) -> f64 {
    match s.total {
        Some(t) if s.speed > 0 => (t.saturating_sub(s.received) as f64) / s.speed as f64,
        _ => f64::MAX,
    }
}

pub(crate) fn in_view(listed: Option<&HashSet<String>>, filter: Category, s: &Snapshot) -> bool {
    match listed {
        Some(ids) => ids.contains(&s.id),
        None => match filter {
            Category::All => true,
            Category::Active => matches!(
                s.status,
                Status::Connecting | Status::Downloading | Status::Paused | Status::Failed
            ),
            Category::Done => s.status == Status::Done,
            other => s.category == other,
        },
    }
}

/// Rows the table shows after the category/list filter, search and sort.
pub(crate) fn visible(view: &MainView) -> Vec<Snapshot> {
    let query = text::fold_search(view.search_query.trim());
    let listed: Option<HashSet<String>> = view.list_filter.as_ref().and_then(|name| {
        view.svc
            .lists()
            .iter()
            .find(|l| &l.name == name)
            .map(|l| l.items.iter().cloned().collect())
    });
    let mut rows: Vec<Snapshot> = view
        .svc
        .view()
        .snaps
        .iter()
        .filter(|s| in_view(listed.as_ref(), view.filter, s))
        .filter(|s| query.is_empty() || text::fold_search(&s.name).contains(&query))
        .cloned()
        .collect();
    if let Some((col, desc)) = view.sort {
        rows.sort_by(|a, b| {
            let ord = match col {
                SortCol::Num => a.order.cmp(&b.order),
                SortCol::Status => status_rank(a.status).cmp(&status_rank(b.status)),
                SortCol::Name => text::fold_search(&a.name).cmp(&text::fold_search(&b.name)),
                SortCol::Size => a.total.unwrap_or(0).cmp(&b.total.unwrap_or(0)),
                SortCol::Progress => progress_ratio(a).total_cmp(&progress_ratio(b)),
                SortCol::Speed => a.speed.cmp(&b.speed),
                SortCol::Eta => eta_seconds(a).total_cmp(&eta_seconds(b)),
                SortCol::LastDate => last_started_ms(a).cmp(&last_started_ms(b)),
            };
            if desc { ord.reverse() } else { ord }
        });
    }
    rows
}

pub(crate) fn table(
    view: &mut MainView,
    _window: &mut Window,
    cx: &mut Context<MainView>,
) -> AnyElement {
    let pal = view.pal;
    let rows = visible(view);
    // Bring a freshly added download into view, unless the user turned this
    // off in Settings > Behavior.
    if let Some(id) = view.pending_focus.take() {
        if let Some(ix) = rows.iter().position(|s| s.id == id) {
            view.row_scroll
                .scroll_to_item(ix, ScrollStrategy::Nearest);
        }
    }
    let total_rows = view.svc.view().snaps.len();
    let all_selected = !rows.is_empty() && rows.iter().all(|s| view.selected.contains(&s.id));
    // Queue positions are stable across sorting.
    let numbers: std::collections::HashMap<String, usize> = view
        .svc
        .view()
        .snaps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i + 1))
        .collect();
    let list_dots: Vec<(String, Hsla)> = view
        .svc
        .lists()
        .iter()
        .map(|l| (l.name.clone(), pal.list_color_of(l)))
        .collect();
    let mut badge_map: std::collections::HashMap<String, Vec<Hsla>> =
        std::collections::HashMap::new();
    for list in view.svc.lists() {
        let color = pal.list_color_of(list);
        for id in &list.items {
            badge_map.entry(id.clone()).or_default().push(color);
        }
    }
    let auto_queued = view.svc.auto_queued_ids();
    let scheduled = view.svc.scheduled_ids();

    let cols = Cols::from(view);
    let header = header_row(view, &rows, all_selected, cols, cx);
    let weak = cx.entity().downgrade();

    if rows.is_empty() {
        let (title, hint) = if total_rows == 0 {
            (t(Key::EmptyTitle), t(Key::EmptyHint))
        } else {
            (t(Key::NoMatchTitle), t(Key::NoMatchHint))
        };
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .flex_1()
                    .on_mouse_down(MouseButton::Left, {
                        let weak = weak.clone();
                        move |_, _, cx: &mut App| {
                            weak.update(cx, |this, cx| {
                                if !this.selected.is_empty() {
                                    this.selected.clear();
                                    cx.notify();
                                }
                            })
                            .ok();
                        }
                    })
                    .child(div().text_lg().font_semibold().text_color(pal.text).align_end().child(title))
                    .child(div().text_sm().text_color(pal.muted).align_end().child(hint)),
            )
            .into_any_element();
    }

    struct Row {
        snap: Snapshot,
        num: usize,
        selected: bool,
        auto: bool,
        scheduled: bool,
        dots: Vec<Hsla>,
    }
    let row_data: Vec<Row> = rows
        .iter()
        .map(|s| Row {
            snap: s.clone(),
            num: numbers.get(&s.id).copied().unwrap_or(0),
            selected: view.selected.contains(&s.id),
            auto: auto_queued.contains(&s.id),
            scheduled: scheduled.contains(&s.id),
            dots: badge_map.get(&s.id).cloned().unwrap_or_default(),
        })
        .collect();
    let _ = list_dots;
    let count = row_data.len();
    let dark = pal.dark;
    let selected_count = view.selected.len();
    let scroll = view.row_scroll.clone();
    let weak_list = weak.clone();

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .w_full()
        .child(header)
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .min_h_0()
                .min_w_0()
                .child(
                    uniform_list("download-rows", count, move |range, _window, _cx| {
                let pal = Palette::new(dark);
                range
                    .map(|ix| {
                        let r = &row_data[ix];
                        row(
                            weak.clone(),
                            r.snap.clone(),
                            r.num,
                            r.selected,
                            r.auto,
                            r.scheduled,
                            r.dots.clone(),
                            cols,
                            &pal,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .on_mouse_down(MouseButton::Left, {
                let weak = weak_list.clone();
                move |_, _, cx: &mut App| {
                    weak.update(cx, |this, cx| {
                        if !this.selected.is_empty() {
                            this.selected.clear();
                            cx.notify();
                        }
                    })
                    .ok();
                }
            })
            .track_scroll(&scroll)
            .flex_1()
            .w_full()
            .min_h_0()
            .min_w_0()
            .px(px(12.))
            .pt(px(6.)),
                )
                // Always-visible scrollbar for long lists; it sits over the
                // right edge of the table without widening the rows.
                .child(
                    Scrollbar::vertical(&RowScroll::new(scroll.clone()))
                        .mode(ScrollbarMode::Always),
                ),
        )
        .when(selected_count > 0, |d| {
            d.child(
                div()
                    .h(px(26.))
                    .px(px(20.))
                    .text_xs()
                    .text_color(pal.muted)
                    .child(tf(Key::SelectedCount, &[&ltr_isolate(&selected_count.to_string())])),
            )
        })
        .into_any_element()
}

fn header_row(
    view: &mut MainView,
    rows: &[Snapshot],
    all_selected: bool,
    cols: Cols,
    cx: &mut Context<MainView>,
) -> AnyElement {
    let pal = view.pal;
    let weak = cx.entity().downgrade();
    let ids: Vec<String> = rows.iter().map(|s| s.id.clone()).collect();
    let sort = view.sort;

    let mut header = div()
        .w_full()
        .flex()
        .flex_row()
        .mirror()
        .items_center()
        .gap(px(8.))
        .h(px(34.))
        .px(px(16.))
        .border_b_1()
        .border_color(pal.line)
        .text_xs()
        .text_color(pal.muted)
        .child(
            Checkbox::new("select-all")
                .checked(all_selected)
                .on_click({
                    let weak = weak.clone();
                    move |on: &bool, _, cx: &mut App| {
                        let ids = ids.clone();
                        let on = *on;
                        weak.update(cx, |this, cx| {
                            for id in &ids {
                                if on {
                                    this.selected.insert(id.clone());
                                } else {
                                    this.selected.remove(id);
                                }
                            }
                            cx.notify();
                        })
                        .ok();
                    }
                }),
        )
        .child(static_header(
            NUM_W,
            sort_label(t(Key::ColNum), SortCol::Num, sort, weak.clone(), pal, false),
        ));
    if let Some(w) = cols.status {
        header = header.child(resizable_header(
            w,
            "status",
            sort_label(t(Key::ColStatus), SortCol::Status, sort, weak.clone(), pal, false),
            weak.clone(),
            pal,
            column_default_width("status"),
            column_min_width("status"),
        ));
    }
    if let Some(w) = cols.lists {
        header = header.child(resizable_header(
            w,
            "lists",
            header_text(t(Key::ColLists)),
            weak.clone(),
            pal,
            column_default_width("lists"),
            column_min_width("lists"),
        ));
    }
    header = header.child(sort_label(
        t(Key::ColFileName),
        SortCol::Name,
        sort,
        weak.clone(),
        pal,
        true,
    ));
    if let Some(w) = cols.size {
        header = header.child(resizable_header(
            w,
            "size",
            sort_label(t(Key::ColSize), SortCol::Size, sort, weak.clone(), pal, false),
            weak.clone(),
            pal,
            column_default_width("size"),
            column_min_width("size"),
        ));
    }
    if let Some(w) = cols.progress {
        header = header.child(resizable_header(
            w,
            "progress",
            sort_label(t(Key::ColProgress), SortCol::Progress, sort, weak.clone(), pal, false),
            weak.clone(),
            pal,
            column_default_width("progress"),
            column_min_width("progress"),
        ));
    }
    if let Some(w) = cols.speed {
        header = header.child(resizable_header(
            w,
            "speed",
            sort_label(t(Key::ColSpeed), SortCol::Speed, sort, weak.clone(), pal, false),
            weak.clone(),
            pal,
            column_default_width("speed"),
            column_min_width("speed"),
        ));
    }
    if let Some(w) = cols.eta {
        header = header.child(resizable_header(
            w,
            "eta",
            sort_label(t(Key::ColEta), SortCol::Eta, sort, weak.clone(), pal, false),
            weak.clone(),
            pal,
            column_default_width("eta"),
            column_min_width("eta"),
        ));
    }
    if let Some(w) = cols.parts {
        header = header.child(resizable_header(
            w,
            "parts",
            header_text(t(Key::ColParts)),
            weak.clone(),
            pal,
            column_default_width("parts"),
            column_min_width("parts"),
        ));
    }
    if let Some(w) = cols.last {
        header = header.child(resizable_header(
            w,
            "last",
            sort_label(t(Key::ColLastDate), SortCol::LastDate, sort, weak.clone(), pal, false),
            weak.clone(),
            pal,
            column_default_width("last"),
            column_min_width("last"),
        ));
    }
    if let Some(w) = cols.actions {
        header = header.child(resizable_header(
            w,
            "actions",
            header_text(t(Key::ColActions)),
            weak,
            pal,
            column_default_width("actions"),
            column_min_width("actions"),
        ));
    }
    header.into_any_element()
}

fn header_text(label: impl AsRef<str>) -> AnyElement {
    div()
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .align_end()
        .child(label.as_ref().to_string())
        .into_any_element()
}

fn static_header(width: f32, content: AnyElement) -> AnyElement {
    div()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .child(content)
        .into_any_element()
}

/// Header cell with a grip on its end edge. Dragging the grip resizes the
/// column through the full-window overlay in `app`.
#[allow(clippy::too_many_arguments)]
fn resizable_header(
    width: f32,
    key: &'static str,
    content: AnyElement,
    weak: WeakEntity<MainView>,
    pal: Palette,
    default_width: u32,
    min_width: u32,
) -> AnyElement {
    div()
        .relative()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .child(content)
        .child(
            div()
                .id(ElementId::Name(format!("col-grip-{key}").into()))
                .absolute()
                .top_0()
                .bottom_0()
                .w(px(7.))
                .edge_end(px(-4.))
                .cursor_col_resize()
                .hover(|s| s.bg(with_alpha(pal.accent, 0.45)))
                .on_mouse_down(MouseButton::Left, move |event, _, cx: &mut App| {
                    let start_x = event.position.x.as_f32();
                    weak.update(cx, |this, cx| {
                        let start_w = this
                            .svc
                            .prefs()
                            .column_widths
                            .get(key)
                            .copied()
                            .unwrap_or(default_width);
                        this.col_resize = Some(crate::app::ColResize {
                            key: key.to_string(),
                            start_x,
                            start_w: start_w as f32,
                            current: start_w,
                            min: min_width,
                            max: 420,
                        });
                        cx.notify();
                    })
                    .ok();
                }),
        )
        .into_any_element()
}

fn sort_label(
    label: impl AsRef<str>,
    col: SortCol,
    sort: Option<(SortCol, bool)>,
    weak: WeakEntity<MainView>,
    pal: Palette,
    grow: bool,
) -> AnyElement {
    let label = label.as_ref();
    let active = sort.map(|(c, _)| c) == Some(col);
    let desc = sort.map_or(false, |(c, d)| c == col && d);
    let arrow = Icon::new(if desc {
        IconName::ArrowDown
    } else {
        IconName::ArrowUp
    })
    .size(px(9.))
    .flex_shrink_0()
    .text_color(if active {
        pal.accent
    } else {
        with_alpha(pal.muted, 0.45)
    });
    let text = label.to_string();
    let el = div()
        .id(ElementId::Name(format!("sort-{label}").into()))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.))
        .h_full()
        .overflow_hidden()
        .whitespace_nowrap()
        .cursor_pointer()
        .text_color(if active { pal.text } else { pal.muted })
        .on_click(move |_, _, cx: &mut App| {
            weak.update(cx, |this, cx| this.set_sort(col, cx)).ok();
        });
    let el = if crate::rtl::on() {
        el.justify_end().child(arrow).child(text)
    } else {
        el.child(text).child(arrow)
    };
    if grow {
        el.flex_1().min_w_0().into_any_element()
    } else {
        el.w_full().into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn row(
    weak: WeakEntity<MainView>,
    snap: Snapshot,
    num: usize,
    selected: bool,
    auto: bool,
    scheduled: bool,
    dots: Vec<Hsla>,
    cols: Cols,
    pal: &Palette,
) -> AnyElement {
    let id = snap.id.clone();
    let status_color = if scheduled && snap.status == Status::Paused {
        pal.warn
    } else {
        pal.status(snap.status)
    };
    let status_label = i18n::row_status_label(snap.status, scheduled, auto);
    let name = snap.name.clone();
    let size = size_text(&snap);
    let pct = if snap.total.unwrap_or(0) > 0 {
        format!("{:.1}%", progress_ratio(&snap) * 100.0)
    } else {
        "—".to_string()
    };
    let speed = if snap.status == Status::Downloading {
        fmt_speed(snap.speed)
    } else {
        "—".to_string()
    };
    let eta = eta_text(&snap);
    // Completed parts over the parts this download was cut into.
    let parts_done = snap
        .parts
        .iter()
        .filter(|p| p.2 >= p.1.saturating_sub(p.0).saturating_add(1))
        .count();
    let parts_total = snap.parts.len();
    let parts = if parts_total > 0 {
        format!("{parts_done}/{parts_total}")
    } else {
        "—".to_string()
    };
    let status = snap.status;
    let dest = snap.dest.clone();
    let bar = progress_bar(&snap, pal);

    // Open the file when it is finished, its folder otherwise; right-click
    // always opens the folder.
    let action = {
        let done = status == Status::Done;
        let dest_click = dest.clone();
        let dest_right = dest.clone();
        Button::new(ElementId::Name(format!("open-{id}").into()))
            .outline()
            .compact()
            .icon(if done {
                IconName::FolderOpen
            } else {
                IconName::Folder
            })
            .text_color(pal.accent)
            .tooltip(if done {
                t(Key::RowOpenFileTip)
            } else {
                t(Key::RowOpenFolderTip)
            })
            .on_click(move |_, _, cx: &mut App| {
                if done {
                    crate::app::open_in_background(
                        std::path::PathBuf::from(&dest_click),
                        cx,
                    );
                } else {
                    crate::app::open_folder_selecting(
                        std::path::PathBuf::from(&dest_click),
                        cx,
                    );
                }
            })
            .on_mouse_down(MouseButton::Right, move |_, _, cx: &mut App| {
                crate::app::open_folder_selecting(
                    std::path::PathBuf::from(&dest_right),
                    cx,
                );
                cx.stop_propagation();
            })
    };

    // Pause or start this download without touching the toolbar. Every row
    // keeps the slot: a completed download shows a check instead of vanishing.
    let toggle = {
        let weak = weak.clone();
        let id = id.clone();
        let done = status == Status::Done;
        let running = matches!(status, Status::Downloading | Status::Connecting);
        let (icon, tone, tip) = if done {
            (
                gpui_kit::assets::IconName::RotateCw,
                pal.warn,
                t(Key::RowRedownloadTip),
            )
        } else if running {
            (
                gpui_kit::assets::IconName::Pause,
                pal.warn,
                t(Key::RowPauseTip),
            )
        } else {
            (
                gpui_kit::assets::IconName::Play,
                pal.ok,
                t(Key::RowStartTip),
            )
        };
        Button::new(ElementId::Name(format!("toggle-{id}").into()))
            .outline()
            .compact()
            // The icon inherits the button color: yellow normally, white on
            // hover like the other action buttons.
            .icon(Icon::new(icon))
            .text_color(tone)
            .tooltip(tip)
            .on_click(move |_, window, cx: &mut App| {
                if done {
                    weak.update(cx, |this, cx| {
                        this.confirm_redownload(id.clone(), window, cx);
                    })
                    .ok();
                } else {
                    weak.update(cx, |this, cx| {
                        if running {
                            this.svc.apply(Command::Pause(vec![id.clone()]));
                        } else {
                            this.svc.apply(Command::Resume(vec![id.clone()]));
                        }
                        cx.notify();
                    })
                    .ok();
                }
            })
            .into_any_element()
    };

    // Always-visible "put this download in a list" shortcut, like the egui row.
    let list_btn = {
        let weak = weak.clone();
        let id = id.clone();
        Button::new(ElementId::Name(format!("list-{id}").into()))
            .outline()
            .compact()
            .icon(IconName::Inbox)
            .text_color(pal.paused)
            .tooltip(t(Key::RowAddToListTip))
            .on_click(move |_, window, cx: &mut App| {
                let id = id.clone();
                weak.update(cx, |this, cx| {
                    this.selected.clear();
                    this.selected.insert(id.clone());
                    this.pick_list_dialog(window, cx);
                    cx.notify();
                })
                .ok();
            })
    };

    let remove = {
        let weak = weak.clone();
        let id = id.clone();
        Button::new(ElementId::Name(format!("remove-{id}").into()))
            .outline()
            .compact()
            .icon(IconName::Delete)
            .text_color(pal.bad)
            .tooltip(t(Key::RowDeleteTip))
            .on_click(move |_, _, cx: &mut App| {
                weak.update(cx, |this, cx| {
                    this.confirm_delete = Some(crate::app::ConfirmDelete {
                        ids: vec![id.clone()],
                        delete_files: false,
                    });
                    cx.notify();
                })
                .ok();
            })
    };

    let props_weak = weak.clone();
    let props_id = id.clone();

    div()
        .id(ElementId::Name(format!("row-{id}").into()))
        // Clicking the card body behaves like its checkbox, and dragging
        // across rows extends the same selection.
        .on_mouse_down(MouseButton::Left, {
            let weak = weak.clone();
            let id = id.clone();
            move |_, _, cx: &mut App| {
                cx.stop_propagation();
                let add = !selected;
                let id = id.clone();
                weak.update(cx, |this, _cx| {
                    this.drag_select = Some(crate::app::DragSelect {
                        add,
                        start_id: id,
                        dragging: false,
                    });
                })
                .ok();
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let weak = weak.clone();
            let id = id.clone();
            move |_, _, cx: &mut App| {
                cx.stop_propagation();
                let id = id.clone();
                weak.update(cx, |this, cx| {
                    let Some(drag) = this.drag_select.take() else {
                        return;
                    };
                    if drag.dragging {
                        cx.notify();
                    } else if drag.start_id == id {
                        if drag.add {
                            this.selected.insert(id);
                        } else {
                            this.selected.remove(&id);
                        }
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        // Right-click selects the card (unless it is already part of the
        // selection) and opens its properties.
        .on_mouse_down(MouseButton::Right, move |_, window, cx: &mut App| {
            props_weak
                .update(cx, |this, cx| {
                    // First right-click clears any current selection; the next
                    // one selects the card under the pointer and opens
                    // Properties.
                    if !this.selected.is_empty() {
                        this.selected.clear();
                        cx.notify();
                        return;
                    }
                    this.selected.insert(props_id.clone());
                    this.props_dialog(props_id.clone(), window, cx);
                    cx.notify();
                })
                .ok();
        })
        // Crossing rows while the button is held extends the checkbox drag.
        .on_mouse_move({
            let weak = weak.clone();
            let id = id.clone();
            move |event, _, cx: &mut App| {
                let left = event.pressed_button == Some(MouseButton::Left);
                let row_id = id.clone();
                weak.update(cx, |this, cx| {
                    let mut clear = false;
                    let mut changed = false;
                    if let Some(drag) = this.drag_select.as_mut() {
                        if !left {
                            clear = true;
                        } else {
                            let add = drag.add;
                            if !drag.dragging {
                                drag.dragging = true;
                                let start = drag.start_id.clone();
                                changed |= set_selected(&mut this.selected, &start, add);
                            }
                            changed |= set_selected(&mut this.selected, &row_id, add);
                        }
                    }
                    if clear {
                        this.drag_select = None;
                    }
                    if changed {
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .h(px(ROW_H - ROW_GAP))
        .mb(px(ROW_GAP))
        .px(px(6.))
        .rounded(px(10.))
        .bg(if selected { with_alpha(pal.accent, 0.10) } else { pal.card })
        .border_1()
        .border_color(if selected {
            with_alpha(pal.accent, 0.52)
        } else {
            pal.line
        })
        .hover(|s| s.border_color(with_alpha(pal.accent, 0.38)))
        .flex()
        .flex_row()
        .mirror()
        .items_center()
        .gap(px(8.))
        .child(
            {
                let weak_start = weak.clone();
                let id_start = id.clone();
                let weak_end = weak.clone();
                let id_end = id.clone();
                Checkbox::new(ElementId::Name(format!("check-{id}").into()))
                    .checked(selected)
                    // Press starts a drag; a release without crossing another
                    // row is a plain click.
                    .on_mouse_down(MouseButton::Left, move |_, _, cx: &mut App| {
                        cx.stop_propagation();
                        let add = !selected;
                        let id = id_start.clone();
                        weak_start
                            .update(cx, |this, _cx| {
                                this.drag_select = Some(crate::app::DragSelect {
                                    add,
                                    start_id: id,
                                    dragging: false,
                                });
                            })
                            .ok();
                    })
                    .on_mouse_up(MouseButton::Left, move |_, _, cx: &mut App| {
                        cx.stop_propagation();
                        let id = id_end.clone();
                        weak_end
                            .update(cx, |this, cx| {
                                let Some(drag) = this.drag_select.take() else {
                                    return;
                                };
                                if drag.dragging {
                                    cx.notify();
                                } else if drag.start_id == id {
                                    if drag.add {
                                        this.selected.insert(id);
                                    } else {
                                        this.selected.remove(&id);
                                    }
                                    cx.notify();
                                }
                            })
                            .ok();
                    })
            }
        )
        .child(
            div()
                .w(px(NUM_W))
                .flex_shrink_0()
                .text_xs()
                .text_color(if selected { pal.text } else { pal.muted })
                .child(ltr_isolate(&format!("#{num}"))),
        )
        .when(cols.status.is_some(), |d| {
            d.child(
                div()
                    .w(px(cols.status.unwrap_or(0.)))
                    .flex_shrink_0()
                    .child(status_pill(status_label, status_color)),
            )
        })
        .when(cols.lists.is_some(), |d| {
            d.child(
                div()
                    .w(px(cols.lists.unwrap_or(0.)))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .mirror()
                    .gap(px(3.))
                    .children(dots.iter().take(2).map(|c| {
                        Icon::new(gpui_kit::assets::IconName::List)
                            .size(px(13.))
                            .text_color(*c)
                    })),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .mirror()
                .items_center()
                .gap(px(9.))
                .child(file_badge(&name, pal))
                .child(
                    div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .gap(px(5.))
                        .child(
                            div()
                                .id(ElementId::Name(format!("name-{id}").into()))
                                .min_w_0()
                                .overflow_hidden()
                                .text_sm()
                                .font_semibold()
                                .text_color(pal.text)
                                .ellipsize_ui()
                                .child(name.clone()),
                        )
                        .child(bar),
                ),
        )
        .when(cols.size.is_some(), |d| {
            d.child(cell_text(cols.size.unwrap_or(0.), ltr_isolate(&size), pal.text))
        })
        .when(cols.progress.is_some(), |d| {
            d.child(cell_text(cols.progress.unwrap_or(0.), ltr_isolate(&pct), pal.text))
        })
        .when(cols.speed.is_some(), |d| {
            d.child(cell_text(cols.speed.unwrap_or(0.), ltr_isolate(&speed), pal.text))
        })
        .when(cols.eta.is_some(), |d| {
            d.child(cell_text(cols.eta.unwrap_or(0.), ltr_isolate(&eta), pal.muted))
        })
        .when(cols.parts.is_some(), |d| {
            d.child(cell_text(cols.parts.unwrap_or(0.), ltr_isolate(&parts), pal.muted))
        })
        .when(cols.last.is_some(), |d| {
            let full = last_download_full(&snap);
            d.child(
                div()
                    .w(px(cols.last.unwrap_or(0.)))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(
                        Button::new(ElementId::Name(format!("last-{id}").into()))
                            .ghost()
                            .compact()
                            .label(ltr_isolate(&last_download_text(&snap)))
                            .text_color(pal.muted)
                            .tooltip(full),
                    ),
            )
        })
        .when(cols.actions.is_some(), |d| {
            d.child(
                div()
                    .w(px(cols.actions.unwrap_or(0.)))
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .mirror()
                    .items_center()
                    .gap(px(2.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx: &mut App| cx.stop_propagation())
                    .child(action)
                    .child(toggle)
                    .child(list_btn)
                    .child(remove),
            )
        })
        .into_any_element()
}

fn cell_text(width: f32, text: String, color: Hsla) -> AnyElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .overflow_hidden()
        .text_sm()
        .text_color(color)
        .ellipsize_ui()
        .align_end()
        .child(text)
        .into_any_element()
}

fn status_pill(label: impl AsRef<str>, color: Hsla) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .mirror()
        .items_center()
        .justify_center()
        .px(px(10.))
        .py(px(3.))
        .rounded_full()
        .bg(with_alpha(color, 0.15))
        .border_1()
        .border_color(with_alpha(color, 0.42))
        .child(div().text_xs().text_color(color).child(label.as_ref().to_string()))
        .into_any_element()
}

fn file_badge(name: &str, pal: &Palette) -> AnyElement {
    let color = pal.file_color(name);
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let mut label: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(3)
        .collect();
    if label.is_empty() {
        label = "FILE".into();
    } else {
        label.make_ascii_uppercase();
    }
    div()
        .size(px(30.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .bg(with_alpha(color, 0.12))
        .border_1()
        .border_color(with_alpha(color, 0.38))
        .child(div().text_xs().text_color(color).child(label))
        .into_any_element()
}

/// Bridges GPUI's uniform-list scroll handle to the component scrollbar.
#[derive(Clone)]
pub(crate) struct RowScroll(UniformListScrollHandle);

impl RowScroll {
    pub(crate) fn new(handle: UniformListScrollHandle) -> Self {
        Self(handle)
    }
}

impl gpui_kit::base::ScrollbarHandle for RowScroll {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.0 .0.borrow().base_handle.bounds()
    }

    fn offset(&self) -> Point<Pixels> {
        self.0 .0.borrow().base_handle.offset()
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.0 .0.borrow().base_handle.set_offset(offset)
    }

    fn content_size(&self) -> Size<Pixels> {
        self.0 .0.borrow().base_handle.content_size()
    }
}

/// One slice per connection. Each slice is as wide as the byte range it covers
/// and fills from its own left edge, so the segmentation survives instead of
/// collapsing into a single progress bar.
fn progress_bar(snap: &Snapshot, pal: &Palette) -> AnyElement {
    let color = pal.status(snap.status);
    let total = snap.total.unwrap_or(0);
    let track = div()
        .h(px(5.))
        .w_full()
        .rounded(px(3.))
        .bg(pal.track)
        .overflow_hidden()
        .flex()
        .flex_row()
        .mirror();

    if snap.status == Status::Done && total > 0 {
        return track
            .child(div().h_full().w_full().bg(color))
            .into_any_element();
    }

    if total > 0 && !snap.parts.is_empty() {
        let mut segments = div().flex().flex_row().mirror().h_full().w_full().gap(px(1.));
        // Parts keep their creation order, and split-off tails are appended, so
        // sort by byte offset before drawing the strip.
        let mut ordered: Vec<&(u64, u64, u64)> = snap.parts.iter().collect();
        ordered.sort_by_key(|p| p.0);
        for (start, end, have) in ordered {
            let span = end.saturating_sub(*start).saturating_add(1);
            if span == 0 {
                continue;
            }
            let filled = (*have).min(span) as f32 / span as f32;
            segments = segments.child(
                div()
                    .flex_grow(span as f32)
                    .flex_basis(px(0.))
                    .min_w(px(1.))
                    .h_full()
                    .rounded(px(2.))
                    .bg(pal.track)
                    .overflow_hidden()
                    .when(filled > 0.0, |d| {
                        d.child(div().h_full().w(relative(filled)).bg(color))
                    }),
            );
        }
        return track.child(segments).into_any_element();
    }

    let pct = if total > 0 {
        (snap.received as f32 / total as f32).clamp(0.0, 1.0)
    } else if snap.received > 0 {
        0.12
    } else {
        0.0
    };
    track
        .when(pct > 0.0, |d| {
            d.child(div().h_full().w(relative(pct)).bg(color))
        })
        .into_any_element()
}
