//! Settings persisted next to the executable (`settings.json`).
//!
//! Shared by every user interface: the layout of this file is part of the
//! application's contract, not of any particular GUI toolkit.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn def_retry_parts() -> bool { false }
fn def_retry_after() -> u32 { 15 }
fn def_font() -> String { "Segoe UI".into() }
fn def_theme_dark() -> bool { true }
fn def_sidebar_width() -> u32 { 268 }
fn def_notify_complete() -> bool { true }
fn def_focus_new() -> bool { true }

/// Upper bound for the global limit control: 1 GB/s.
pub(crate) const MAX_LIMIT_KBPS: u64 = 1024 * 1024;

/// Sidebar width limits, in logical pixels.
pub(crate) const MIN_SIDEBAR_WIDTH: u32 = 220;
pub(crate) const MAX_SIDEBAR_WIDTH: u32 = 420;

/// Number of label colours a download list can cycle through.
pub(crate) const LIST_COLOR_COUNT: usize = 8;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct DownloadList {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) items: Vec<String>,
    #[serde(default)]
    pub(crate) color: u8,
    /// Max downloads from this list running at once; 0 means unlimited.
    #[serde(default)]
    pub(crate) max_parallel: u32,
    /// Local time-of-day window (minutes since midnight) in which the list may
    /// run; `None` on either side means no schedule.
    #[serde(default)]
    pub(crate) sched_start: Option<u32>,
    #[serde(default)]
    pub(crate) sched_end: Option<u32>,
    /// Force shut down the system when this list's window closes.
    #[serde(default)]
    pub(crate) shutdown_at_end: bool,
    /// The list new downloads land in; renaming it must not change that.
    #[serde(default)]
    pub(crate) is_default: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Prefs {
    pub(crate) dir: String,
    pub(crate) conns: u32,
    pub(crate) queued: bool,
    #[serde(default = "def_retry_parts")]
    pub(crate) retry_parts: bool,
    #[serde(default = "def_retry_after")]
    pub(crate) retry_after: u32,
    #[serde(default)]
    pub(crate) limit_kbps: u64,
    #[serde(default)]
    pub(crate) lists: Vec<DownloadList>,
    #[serde(default = "def_font")]
    pub(crate) font: String,
    /// Interface theme; the egui build only ever uses the dark one.
    #[serde(default = "def_theme_dark")]
    pub(crate) theme_dark: bool,
    /// Width of the navigation sidebar in logical pixels.
    #[serde(default = "def_sidebar_width")]
    pub(crate) sidebar_width: u32,
    /// Folder for partial files; empty means the default next to the exe.
    #[serde(default)]
    pub(crate) temp_dir: String,
    /// Show the desktop card when a download finishes.
    #[serde(default = "def_notify_complete")]
    pub(crate) notify_complete: bool,
    /// Scroll the download list to a newly added download.
    #[serde(default = "def_focus_new")]
    pub(crate) focus_new_download: bool,
    /// Whether the startup-with-Windows choice has been made. Older settings
    /// files lack it, so those enable autostart once on the next launch.
    #[serde(default)]
    pub(crate) autostart_configured: bool,
    /// Table sort: column key (empty means unsorted) and direction.
    #[serde(default)]
    pub(crate) sort_col: String,
    #[serde(default)]
    pub(crate) sort_desc: bool,
    /// Unix seconds of the last update check (manual or background).
    #[serde(default)]
    pub(crate) last_update_check: u64,
    /// Newest version already announced by the background check.
    #[serde(default)]
    pub(crate) update_notified: String,
}
pub(crate) fn prefs_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("settings.json")))
        .unwrap_or_else(|| PathBuf::from("settings.json"))
}

pub(crate) fn load_prefs(default_dir: &std::path::Path) -> Prefs {
    if let Ok(s) = std::fs::read_to_string(prefs_path()) {
        if let Ok(p) = serde_json::from_str::<Prefs>(&s) {
            return Prefs {
                dir: if p.dir.trim().is_empty() {
                    default_dir.to_string_lossy().into()
                } else {
                    p.dir
                },
                conns: p.conns.clamp(1, 32),
                queued: p.queued,
                retry_parts: p.retry_parts,
                retry_after: p.retry_after.clamp(3, 300),
                limit_kbps: p.limit_kbps.min(MAX_LIMIT_KBPS),
                lists: p.lists,
                font: if p.font.trim().is_empty() { def_font() } else { p.font },
                theme_dark: p.theme_dark,
                sidebar_width: p
                    .sidebar_width
                    .clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH),
                temp_dir: p.temp_dir,
                notify_complete: p.notify_complete,
                focus_new_download: p.focus_new_download,
                autostart_configured: p.autostart_configured,
                sort_col: p.sort_col,
                sort_desc: p.sort_desc,
                last_update_check: p.last_update_check,
                update_notified: p.update_notified,
            };
        }
    }
    Prefs {
        dir: default_dir.to_string_lossy().into(),
        conns: 8,
        queued: false,
        retry_parts: false,
        retry_after: 15,
        limit_kbps: 0,
        lists: Vec::new(),
        font: def_font(),
        theme_dark: true,
        sidebar_width: def_sidebar_width(),
        temp_dir: String::new(),
        notify_complete: def_notify_complete(),
        focus_new_download: def_focus_new(),
        autostart_configured: false,
        sort_col: String::new(),
        sort_desc: false,
        last_update_check: 0,
        update_notified: String::new(),
    }
}

impl Prefs {
    pub(crate) fn theme_dark(&self) -> bool {
        self.theme_dark
    }
}

impl Prefs {
    pub(crate) fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(prefs_path(), s);
        }
    }
}
