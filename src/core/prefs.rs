//! Settings persisted next to the executable (`settings.json`).
//!
//! Shared by every user interface: the layout of this file is part of the
//! application's contract, not of any particular GUI toolkit.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn def_retry_parts() -> bool { false }
fn def_retry_after() -> u32 { 15 }
fn def_font() -> String { "Segoe UI".into() }
fn def_theme_dark() -> bool { true }
fn def_ui_lang() -> String { "system".into() }
fn def_sidebar_width() -> u32 { 268 }
fn def_notify_complete() -> bool { true }
fn def_focus_new() -> bool { true }

/// Extensions the browser extension captures on Ctrl/Alt+click, lowercase
/// without the leading dot. Chosen so the common large-file downloads
/// (archives, disk images, installers, media) are covered.
pub(crate) const DEFAULT_DIRECT_FORMATS: [&str; 38] = [
    "7z", "zip", "rar", "tar", "gz", "bz2", "xz", "tgz", "cab", "iso", "img", "dmg", "vhd", "wim", "exe", "msi", "msix", "apk", "deb", 
	"rpm", "pkg", "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg","mp3", "flac", "wav", "ogg", "m4a", "pdf", "epub", "torrent",
];

fn def_proxy_mode() -> String { "none".into() }
fn def_proxy_kind() -> String { "http".into() }
fn def_proxy_port() -> u16 { 8080 }
fn def_proxy_true() -> bool { true }

/// Normalize a typed colour to `#rrggbb`; `None` when it is not a 6-digit hex
/// value. The leading `#` is optional.
pub(crate) fn normalize_hex_color(raw: &str) -> Option<String> {
    let s = raw.trim().trim_start_matches('#');
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", s.to_ascii_lowercase()))
}

pub(crate) fn normalize_proxy_kind(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "socks" | "socks5" => "socks5".into(),
        "socks4" | "socks4a" => "socks4".into(),
        _ => "http".into(),
    }
}

/// Longest extension the shortcut accepts. Model formats such as
/// `safetensors` (11) exceed the original 8-character cap.
pub(crate) const MAX_FORMAT_LEN: usize = 32;

pub(crate) fn normalize_ui_lang(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "en" | "english" => "en".into(),
        "fa" | "persian" | "farsi" => "fa".into(),
        _ => "system".into(),
    }
}

/// Whether the chosen interface language renders right-to-left. The system
/// choice follows the Windows UI language passed in.
pub(crate) fn ui_lang_is_rtl(lang: &str, system_lang: &str) -> bool {
    match lang {
        "fa" | "ar" | "he" => true,
        "en" => false,
        _ => matches!(system_lang, "fa" | "ar" | "he"),
    }
}

pub(crate) fn normalize_proxy_mode(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "system" => "system".into(),
        "manual" => "manual".into(),
        _ => "none".into(),
    }
}

pub(crate) fn default_browser_modifier() -> String {
    "both".into()
}

pub(crate) fn normalize_browser_modifier(raw: &str) -> String {
    let key = raw.trim().to_ascii_lowercase();
    let parts: Vec<&str> = key
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect();
    let ctrl = parts.contains(&"ctrl");
    let alt = parts.contains(&"alt");
    let shift = parts.contains(&"shift");
    // Insert is not a mouse modifier; the extension tracks it while held.
    if parts.contains(&"insert") && !ctrl && !alt && !shift {
        return "insert".into();
    }
    match (ctrl, alt, shift) {
        (true, false, false) => "ctrl".into(),
        (false, true, false) => "alt".into(),
        (false, false, true) => "shift".into(),
        (true, true, false) => "ctrl_alt".into(),
        (true, false, true) => "ctrl_shift".into(),
        (false, true, true) => "alt_shift".into(),
        _ => "both".into(),
    }
}

pub(crate) fn default_direct_formats() -> Vec<String> {
    DEFAULT_DIRECT_FORMATS.iter().map(|s| s.to_string()).collect()
}

/// Split the raw settings text into normalized extensions: commas, spaces,
/// semicolons and newlines separate entries, casing and leading dots go away.
/// Returns empty when nothing valid was found; the settings field uses that
/// to keep the saved list instead of restoring defaults.
pub(crate) fn collect_direct_formats(raw: &str) -> Vec<String> {
    let mut list: Vec<String> = Vec::new();
    for part in raw.split([',', ';', '\n', '\r', ' ', '\t']) {
        let ext = part.trim().trim_start_matches('.').to_ascii_lowercase();
        if !ext.is_empty()
            && ext.len() <= MAX_FORMAT_LEN
            && ext.bytes().all(|b| b.is_ascii_alphanumeric())
            && !list.contains(&ext)
        {
            list.push(ext);
        }
    }
    list
}

/// Entries from the settings text that [`collect_direct_formats`] drops, so
/// the page can tell the user instead of losing them silently.
pub(crate) fn invalid_direct_formats(raw: &str) -> Vec<String> {
    raw.split([',', ';', '\n', '\r', ' ', '\t'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter(|part| {
            let ext = part.trim_start_matches('.').to_ascii_lowercase();
            ext.is_empty()
                || ext.len() > MAX_FORMAT_LEN
                || !ext.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .map(str::to_string)
        .collect()
}

/// Like [`collect_direct_formats`], but an empty result falls back to the
/// defaults so a blank Reset or an older settings file cannot disable the
/// browser shortcut.
pub(crate) fn parse_direct_formats(raw: &str) -> Vec<String> {
    let list = collect_direct_formats(raw);
    if list.is_empty() {
        default_direct_formats()
    } else {
        list
    }
}

pub(crate) fn format_direct_formats(list: &[String]) -> String {
    list.join(", ")
}

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
    /// Optional custom label colour (#rrggbb); overrides `color` when set.
    #[serde(default)]
    pub(crate) color_hex: Option<String>,
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
    /// Extensions captured by the browser Ctrl/Alt+click shortcut.
    #[serde(default = "default_direct_formats")]
    pub(crate) direct_formats: Vec<String>,
    /// Interface language: "system", "en", "fa" or "ar". RTL follows it.
    #[serde(default = "def_ui_lang")]
    pub(crate) ui_lang: String,
    /// Browser shortcut modifier: "ctrl", "alt", "shift", "insert",
    /// "ctrl_alt", "ctrl_shift", "alt_shift" or "both" (Ctrl or Alt).
    #[serde(default = "default_browser_modifier")]
    pub(crate) browser_modifier: String,
    /// Proxy mode: "none", "system" or "manual".
    #[serde(default = "def_proxy_mode")]
    pub(crate) proxy_mode: String,
    /// Proxy protocol: "http" or "socks5".
    #[serde(default = "def_proxy_kind")]
    pub(crate) proxy_kind: String,
    #[serde(default)]
    pub(crate) proxy_host: String,
    #[serde(default = "def_proxy_port")]
    pub(crate) proxy_port: u16,
    #[serde(default)]
    pub(crate) proxy_user: String,
    #[serde(default)]
    pub(crate) proxy_pass: String,
    #[serde(default = "def_proxy_true")]
    pub(crate) proxy_http: bool,
    #[serde(default = "def_proxy_true")]
    pub(crate) proxy_https: bool,
    #[serde(default)]
    pub(crate) proxy_ftp: bool,
    #[serde(default)]
    pub(crate) proxy_from_browser: bool,
    /// Column keys the user chose to hide in the download table.
    #[serde(default)]
    pub(crate) column_hidden: Vec<String>,
    /// Persisted per-column widths, keyed by the column name.
    #[serde(default)]
    pub(crate) column_widths: BTreeMap<String, u32>,
}
pub(crate) fn prefs_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("settings.json")))
        .unwrap_or_else(|| PathBuf::from("settings.json"))
}

pub(crate) fn settings_have_direct_formats() -> bool {
    std::fs::read_to_string(prefs_path())
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .is_some_and(|v| v.get("direct_formats").is_some())
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
                ui_lang: normalize_ui_lang(&p.ui_lang),
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
                direct_formats: parse_direct_formats(&p.direct_formats.join(",")),
                browser_modifier: normalize_browser_modifier(&p.browser_modifier),
                proxy_mode: normalize_proxy_mode(&p.proxy_mode),
                proxy_kind: normalize_proxy_kind(&p.proxy_kind),
                proxy_host: p.proxy_host,
                proxy_port: p.proxy_port.max(1),
                proxy_user: p.proxy_user,
                proxy_pass: p.proxy_pass,
                proxy_http: p.proxy_http,
                proxy_https: p.proxy_https,
                proxy_ftp: p.proxy_ftp,
                proxy_from_browser: p.proxy_from_browser,
                column_hidden: p.column_hidden,
                column_widths: p.column_widths,
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
        ui_lang: def_ui_lang(),
        sidebar_width: def_sidebar_width(),
        temp_dir: String::new(),
        notify_complete: def_notify_complete(),
        focus_new_download: def_focus_new(),
        autostart_configured: false,
        sort_col: String::new(),
        sort_desc: false,
        last_update_check: 0,
        update_notified: String::new(),
        direct_formats: default_direct_formats(),
        browser_modifier: default_browser_modifier(),
        proxy_mode: def_proxy_mode(),
        proxy_kind: def_proxy_kind(),
        proxy_host: String::new(),
        proxy_port: def_proxy_port(),
        proxy_user: String::new(),
        proxy_pass: String::new(),
        proxy_http: true,
        proxy_https: true,
        proxy_ftp: false,
        proxy_from_browser: false,
        column_hidden: Vec::new(),
        column_widths: BTreeMap::new(),
    }
}

impl Prefs {
    pub(crate) fn theme_dark(&self) -> bool {
        self.theme_dark
    }
}

impl Prefs {
    pub(crate) fn save(&self) {
        let path = prefs_path();
        match serde_json::to_string_pretty(self) {
            Ok(s) => {
                if let Err(err) = std::fs::write(&path, s) {
                    super::log::write(format!("settings save failed {path:?}: {err}"));
                }
            }
            Err(err) => super::log::write(format!("settings encode failed: {err}")),
        }
    }
}
