use crate::engine::{Snapshot, Status};

pub(crate) fn status_text(s: &Snapshot) -> String {
    match s.status {
        Status::Connecting => "Connecting".into(),
        Status::Downloading => "Downloading".into(),
        Status::Paused => "Paused".into(),
        Status::Done => "Complete".into(),
        Status::Failed => s.error.clone().unwrap_or_else(|| "Failed".into()),
    }
}

pub(crate) fn size_text(s: &Snapshot) -> String {
    match s.total {
        Some(t) => format!("{} / {}", fmt_bytes(s.received), fmt_bytes(t)),
        None => fmt_bytes(s.received),
    }
}

pub(crate) fn eta_text(s: &Snapshot) -> String {
    if s.status != Status::Downloading || s.speed == 0 {
        return "—".into();
    }
    let Some(t) = s.total else { return "—".into() };
    let left = t.saturating_sub(s.received);
    let sec = left / s.speed.max(1);
    if sec >= 3600 {
        format!("{}h {}m", sec / 3600, (sec % 3600) / 60)
    } else if sec >= 60 {
        format!("{}m {}s", sec / 60, sec % 60)
    } else {
        format!("{sec}s")
    }
}

pub(crate) fn fmt_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 4 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", U[i]) }
}

pub(crate) fn fmt_speed(n: u64) -> String {
    if n == 0 {
        "—".into()
    } else {
        format!("{}/s", fmt_rate(n))
    }
}

/// Rates keep two decimals (1.94 MB/s) so small changes are visible.
fn fmt_rate(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 4 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.2} {}", U[i])
    }
}

/// Active download time, excluding pauses.
pub(crate) fn fmt_duration(ms: u64) -> String {
    let secs = ms / 1000;
    if secs >= 3600 {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}
