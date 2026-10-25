//! Queue planning: list caps and time-of-day schedule windows.
//!
//! Everything here is pure so the rules can be tested without a manager or a
//! window. The user interfaces call it once per tick and only apply the
//! resulting pause/resume lists.
use super::prefs::DownloadList;
use crate::engine::{Snapshot, Status};
use std::collections::{HashMap, HashSet};

pub(crate) fn move_selection(order: &mut [String], selected: &HashSet<String>, up: bool) -> bool {
    let mut moved = false;
    if up {
        for i in 1..order.len() {
            if selected.contains(&order[i]) && !selected.contains(&order[i - 1]) {
                order.swap(i - 1, i);
                moved = true;
            }
        }
    } else {
        for i in (0..order.len().saturating_sub(1)).rev() {
            if selected.contains(&order[i]) && !selected.contains(&order[i + 1]) {
                order.swap(i, i + 1);
                moved = true;
            }
        }
    }
    moved
}

pub(crate) struct ListRule<'a> {
    pub(crate) items: &'a [String],
    pub(crate) limit: u32,
    /// Has a start/end window. Its paused members may start on their own.
    pub(crate) scheduled: bool,
}

/// Queue plan for the list caps: over the limit the downloads latest in the
/// queue wait, and freed slots go to the earliest waiting ones.
pub(crate) fn plan_limit_actions(
    rules: &[ListRule<'_>],
    snaps: &[Snapshot],
    queued: &HashSet<String>,
    user_paused: &HashSet<String>,
    user_started: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let by_id: HashMap<&str, &Snapshot> = snaps.iter().map(|s| (s.id.as_str(), s)).collect();
    let is_active = |s: &Snapshot| matches!(s.status, Status::Connecting | Status::Downloading);
    let active: Vec<usize> = rules
        .iter()
        .map(|r| {
            r.items
                .iter()
                .filter(|id| by_id.get(id.as_str()).map(|s| is_active(s)).unwrap_or(false))
                .count()
        })
        .collect();
    // A task held back by any full list must not be resumed by another list.
    let blocked = |id: &str| {
        rules.iter().enumerate().any(|(ri, r)| {
            r.items.iter().any(|m| m.as_str() == id) && active[ri] >= r.limit as usize
        })
    };
    let mut pause: Vec<String> = Vec::new();
    for (ri, r) in rules.iter().enumerate() {
        let limit = r.limit as usize;
        if active[ri] > limit {
            // A window that has closed stops the schedule's own downloads, but
            // never one the user started by hand.
            let closed = r.scheduled && r.limit == 0;
            let mut excess: Vec<&Snapshot> = r
                .items
                .iter()
                .filter_map(|id| by_id.get(id.as_str()).copied())
                .filter(|s| is_active(s))
                .filter(|s| !(closed && user_started.contains(s.id.as_str())))
                .collect();
            excess.sort_by_key(|s| s.order);
            for s in excess.into_iter().rev().take(active[ri] - limit) {
                pause.push(s.id.clone());
            }
        }
    }
    let mut resume: Vec<String> = Vec::new();
    for (ri, r) in rules.iter().enumerate() {
        let limit = r.limit as usize;
        if active[ri] < limit {
            let mut waiting: Vec<&Snapshot> = r
                .items
                .iter()
                .filter_map(|id| by_id.get(id.as_str()).copied())
                .filter(|s| {
                    s.status == Status::Paused
                        && !user_paused.contains(s.id.as_str())
                        && (r.scheduled || queued.contains(s.id.as_str()))
                        && !blocked(s.id.as_str())
                })
                .collect();
            waiting.sort_by_key(|s| s.order);
            for s in waiting.into_iter().take(limit - active[ri]) {
                if !resume.iter().any(|x| x == &s.id) {
                    resume.push(s.id.clone());
                }
            }
        }
    }
    (pause, resume)
}

/// Is `now` (minutes since midnight) inside the [start, end) window?
/// start > end wraps midnight; start == end means the window is always closed.
pub(crate) fn window_open(now: u32, start: u32, end: u32) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

/// Minutes from `now` until the next opening of the [start, end) window.
pub(crate) fn until_window_open(now: u32, start: u32, end: u32) -> u32 {
    let day = 24 * 60;
    if start == end {
        // Never opens; report the next nominal start.
        return if now < start { start - now } else { day - now + start };
    }
    if window_open(now, start, end) {
        0
    } else if now < start {
        start - now
    } else {
        day - now + start
    }
}

/// Length of the [start, end) window in minutes.
pub(crate) fn window_len(start: u32, end: u32) -> u32 {
    let day = 24 * 60;
    if start == end {
        0
    } else if start < end {
        end - start
    } else {
        day - start + end
    }
}

pub(crate) fn list_window_open(l: &DownloadList, now_min: u32) -> bool {
    match (l.sched_start, l.sched_end) {
        (Some(a), Some(b)) => window_open(now_min, a, b),
        _ => true,
    }
}

#[derive(Default)]
pub(crate) struct ScheduleEvents {
    /// Members of lists whose window just opened. The opening moment overrides
    /// any paused state, including a pause the user set by hand.
    pub(crate) opened: HashSet<String>,
    /// A list that asked for a shutdown just reached its end time.
    pub(crate) shutdown: bool,
}

pub(crate) fn schedule_events(
    lists: &[DownloadList],
    now_min: u32,
    was_open: &mut HashMap<String, bool>,
) -> ScheduleEvents {
    let mut events = ScheduleEvents::default();
    for l in lists {
        if l.sched_start.is_none() || l.sched_end.is_none() {
            was_open.remove(&l.name);
            continue;
        }
        let open = list_window_open(l, now_min);
        let prev = was_open.insert(l.name.clone(), open);
        if open && prev != Some(true) {
            events.opened.extend(l.items.iter().cloned());
        }
        if !open && prev == Some(true) && l.shutdown_at_end {
            events.shutdown = true;
        }
    }
    was_open.retain(|name, _| lists.iter().any(|l| &l.name == name));
    events
}

/// Rules for every list that can limit or start downloads right now. A
/// scheduled list always gets a rule: closed means zero slots, open means its
/// cap (or unlimited), so its paused members can start on their own.
pub(crate) fn list_rules(lists: &[DownloadList], now_min: u32) -> Vec<ListRule<'_>> {
    lists
        .iter()
        .filter_map(|l| {
            let scheduled = l.sched_start.is_some() && l.sched_end.is_some();
            let open = list_window_open(l, now_min);
            if !scheduled && !(open && l.max_parallel > 0) {
                return None;
            }
            Some(ListRule {
                items: &l.items,
                limit: if !open {
                    0
                } else if l.max_parallel == 0 {
                    u32::MAX
                } else {
                    l.max_parallel
                },
                scheduled,
            })
        })
        .collect()
}

pub(crate) fn fmt_hm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub(crate) fn parse_hm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.trim().parse().ok()?;
    let m: u32 = m.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}
